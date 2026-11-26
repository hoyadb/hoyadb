package dev.hoyadb

import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

// DEF-0040: actual Kotlin/Android host behavior with controlled native replies.
// No database algorithm, JNI or native persistence is certified by this fixture.
object BackgroundErrorTests {
    private const val CANARY = "private-background-message-command-key"
    private fun eventually(body: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!body()) { check(System.nanoTime() < deadline) { "missing expected background fact" }; Thread.sleep(1) }
    }
    private fun await(latch: CountDownLatch) { check(latch.await(5, TimeUnit.SECONDS)) }
    private class QueuedExecutor : Executor {
        val work = LinkedBlockingQueue<Runnable>()
        override fun execute(command: Runnable) { work.add(command) }
        fun drain() { while (true) (work.poll() ?: return).run() }
    }
    private class Fixture(executor: Executor = Executor { it.run() }, timer: HoyaTimer = SystemHoyaTimer,
        throwSink: Boolean = false, sinkAction: ((BackgroundError) -> Unit)? = null) : NativeCalling {
        val facts = CopyOnWriteArrayList<BackgroundError>()
        val counts = ConcurrentHashMap<String, AtomicInteger>()
        val events = AtomicReference<List<ProtocolValue>>(emptyList())
        val heldDrain = AtomicReference<HoyaNativeTransport.Reply>()
        private val nextSubscription = AtomicInteger(6)
        @Volatile var holdDrain = false
        @Volatile var failDrain = false
        @Volatile var failUnsubscribe = false
        @Volatile var envelopes = emptyList<String>()
        @Volatile var due: Long? = null
        var replyOverride: ((String, Map<String, ProtocolValue>) -> ProtocolValue?)? = null
        val db = Database(this, executor, null, DatabaseOptions(), { 0 }, timer) {
            facts.add(it)
            sinkAction?.invoke(it)
            if (throwSink) throw AssertionError(CANARY)
        }
        fun count(op: String) = counts[op]?.get() ?: 0
        fun success(reply: HoyaNativeTransport.Reply, value: ProtocolValue) {
            reply.onSuccess(WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to value)))
        }
        override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) { success(reply, str("1")) }
        override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
            val fields = WireCodec.decode(frame).fields()
            val op = fields.getValue("op").string()
            counts.computeIfAbsent(op) { AtomicInteger() }.incrementAndGet()
            if (op == "drain" && holdDrain) { check(heldDrain.compareAndSet(null, reply)); return }
            if ((op == "drain" && failDrain) || (op.startsWith("unsubscribe") && failUnsubscribe)) {
                reply.onFailure(CANARY); return
            }
            val value = replyOverride?.invoke(op, fields.getValue("args").fields()) ?: when (op) {
                "observe", "observe_status", "observe_outcome", "observe_progress" -> str(nextSubscription.incrementAndGet().toString())
                "drain" -> obj("events" to ProtocolValue.ArrayValue(events.getAndSet(emptyList())), "has_more" to ProtocolValue.BooleanValue(false))
                "insert" -> Row("committed", emptyMap()).toProtocolValue()
                "prepare_push" -> obj("envelopes" to ProtocolValue.ArrayValue(envelopes.map(::str)), "next_due_ms" to (due?.let(::decimal) ?: ProtocolValue.Null), "has_more" to ProtocolValue.BooleanValue(false))
                "pending_reconciliations" -> ProtocolValue.ArrayValue(emptyList())
                "begin_pull" -> obj("request" to str("4"), "since" to ProtocolValue.Null, "scope" to str("scope"), "generation" to str("1"))
                else -> ProtocolValue.Null
            }
            success(reply, value)
        }
        suspend fun open() { db.openMemory(notesSchema(), TEST_NAMESPACE) }
        fun checkFacts(vararg expected: Triple<BackgroundError.Source, String, String?>) {
            eventually { facts.size >= expected.size }
            check(facts.map { Triple(it.source, it.kind, it.token) }.toSet() == expected.toSet())
            check(facts.size == expected.size)
            for (fact in facts) check(!fact.toString().contains(CANARY) && !fact.kind.contains(CANARY))
        }
    }
    private fun rows(token: String = "7") = obj("kind" to str("data"), "subscription" to str(token), "value" to obj("kind" to str("rows"), "rows" to ProtocolValue.ArrayValue(emptyList())))
    private fun terminal(kind: String = "LimitExceeded") = obj("kind" to str("error"), "subscription" to str("7"), "value" to obj("kind" to str(kind)))

    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun test(name: String, body: suspend () -> Unit) {
            if (args.isNotEmpty() && name !in args) return
            try { blocking(body); println("PASS $name") }
            catch (error: Throwable) {
                failures++
                println("FAIL $name ${error.javaClass.simpleName} line=" + error.stackTrace.firstOrNull { it.className.contains("BackgroundErrorTests") }?.lineNumber)
            }
        }
        test("callback_and_failed_cleanup_are_distinct_redacted_facts_once") {
            val f = Fixture(throwSink = true); f.open(); f.failUnsubscribe = true
            val calls = AtomicInteger(); f.events.set(listOf(rows(), rows()))
            try {
                f.db.observe(Query("notes")) { calls.incrementAndGet(); throw IOException(CANARY) }
                f.checkFacts(Triple(BackgroundError.Source.Callback, "CallbackFailure", "7"), Triple(BackgroundError.Source.Observation, "BackendFailure", "7"))
                eventually { synchronized(f.db.stateLock) { f.db.drainGuard == 0 } }
                check(calls.get() == 1 && f.count("unsubscribe") == 1)
                check(f.facts.size == 2)
            } finally { f.db.close() }
        }
        test("terminal_delivers_once_and_throwing_terminal_callback_reports_both") {
            val f = Fixture(); f.open(); val calls = AtomicInteger()
            f.events.set(listOf(terminal(), rows(), terminal()))
            try {
                f.db.observe(Query("notes")) { value ->
                    check(value is ObservedValue.Terminated && value.kind == "LimitExceeded")
                    calls.incrementAndGet(); throw IOException(CANARY)
                }
                f.checkFacts(Triple(BackgroundError.Source.Observation, "LimitExceeded", "7"), Triple(BackgroundError.Source.Callback, "CallbackFailure", "7"))
                check(f.facts.map { it.source } == listOf(BackgroundError.Source.Observation, BackgroundError.Source.Callback))
                eventually { synchronized(f.db.stateLock) { f.db.drainGuard == 0 } }
                check(calls.get() == 1 && f.count("unsubscribe") == 0)
                check(f.facts.size == 2)
            } finally { f.db.close() }
        }
        test("managed_drain_failure_coalesces_wakes_into_one_fact") {
            val f = Fixture(); f.open(); f.holdDrain = true
            try {
                f.db.observe(Query("notes")) { }
                eventually { f.heldDrain.get() != null }
                repeat(5) { f.db.mutated() }
                f.holdDrain = false; f.failDrain = true
                f.heldDrain.getAndSet(null).onFailure(CANARY)
                f.checkFacts(Triple(BackgroundError.Source.Observation, "BackendFailure", null))
                eventually { synchronized(f.db.stateLock) { f.db.drainGuard == 1 } }
                check(f.count("drain") == 1)
                f.failDrain = false
                f.db.mutated()
                eventually { f.count("drain") == 2 && synchronized(f.db.stateLock) { f.db.drainGuard == 1 } }
                check(f.facts.size == 1)
            } finally { f.db.close() }
        }
        test("all_queue_listener_kinds_route_terminal_before_value_decoding") {
            for (kind in listOf("status", "outcome", "progress")) {
                val f = Fixture(); f.open(); f.events.set(listOf(terminal(CANARY), terminal()))
                val ended = CopyOnWriteArrayList<String>()
                try {
                    when (kind) {
                        "status" -> f.db.observeStatus({ error("unexpected status") }, { ended.add(it) })
                        "outcome" -> f.db.observeOutcome("key", { error("unexpected outcome") }, { ended.add(it) })
                        else -> f.db.observeProgress({ error("unexpected progress") }, { ended.add(it) })
                    }
                    f.checkFacts(Triple(BackgroundError.Source.Observation, "BackendFailure", "7"))
                    eventually { ended.size == 1 }
                    check(ended.single() == "BackendFailure" && f.count("unsubscribe_sync") == 0)
                } finally { f.db.close() }
            }
        }
        test("recoverable_query_error_keeps_listener_and_is_not_background_fact") {
            val f = Fixture(); f.open(); val values = CopyOnWriteArrayList<ObservedValue>()
            val recoverable = obj("kind" to str("data"), "subscription" to str("7"), "value" to obj("kind" to str("error"), "error" to obj("kind" to str("InvalidQuery"))))
            f.events.set(listOf(recoverable, rows()))
            try {
                val subscription = f.db.observe(Query("notes")) { values.add(it) }
                eventually { values.size == 2 }
                check(values[0] is ObservedValue.Failure && values[1] is ObservedValue.Rows)
                check(f.facts.isEmpty() && f.count("unsubscribe") == 0)
                subscription.close()
            } finally { f.db.close() }
        }
        test("awaited_drain_and_pull_failures_are_returned_without_background_fact") {
            val f = Fixture(); f.open()
            try {
                f.failDrain = true
                try { f.db.drainNotifications(); error("drain accepted") } catch (error: HoyaException) { check(error.kind == "NativeFailure") }
                try { f.db.pullOnce { _, _ -> throw IOException(CANARY) }; error("pull accepted") } catch (_: IOException) { }
                check(f.count("abort_pull") == 1 && f.facts.isEmpty())
            } finally { f.db.close() }
        }
        test("explicit_drain_callback_failure_reports_without_failing_the_awaited_drain") {
            val f = Fixture(); f.open(); f.failUnsubscribe = true
            try {
                f.db.observe(Query("notes")) { throw IOException(CANARY) }
                eventually { f.count("drain") == 1 && synchronized(f.db.stateLock) { f.db.drainGuard == 1 } }
                f.events.set(listOf(rows(), rows()))
                f.db.drainNotifications()
                f.checkFacts(Triple(BackgroundError.Source.Callback, "CallbackFailure", "7"), Triple(BackgroundError.Source.Observation, "BackendFailure", "7"))
                check(f.count("unsubscribe") == 1)
            } finally { f.db.close() }
        }
        test("manual_subscription_close_preserves_prior_silent_cleanup_failure") {
            val unexpected = CountDownLatch(1)
            val f = Fixture(sinkAction = { unexpected.countDown() }); f.open()
            try {
                val subscription = f.db.observe(Query("notes")) { }
                eventually { f.count("drain") == 1 && synchronized(f.db.stateLock) { f.db.drainGuard == 1 } }
                f.failUnsubscribe = true
                subscription.close()
                eventually { f.count("unsubscribe") == 1 }
                check(!unexpected.await(1, TimeUnit.SECONDS)) { "manual cleanup invented a background fact" }
                check(f.facts.isEmpty() && synchronized(f.db.stateLock) { f.db.drainGuard == 0 })
            } finally { f.db.close() }
        }
        test("timer_failure_and_throwing_sink_preserve_committed_mutation") {
            val calls = AtomicInteger()
            val timer = object : HoyaTimer { override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable { calls.incrementAndGet(); throw IOException(CANARY) } }
            val f = Fixture(timer = timer, throwSink = true); f.open()
            try {
                f.db.observe(Query("notes"), ObservationMode.Count) { }
                val result = f.db.insert("notes", emptyMap(), "committed")
                check(result.id == "committed" && f.count("insert") == 1)
                f.checkFacts(Triple(BackgroundError.Source.Timer, "BackendFailure", null))
                check(calls.get() == 1)
            } finally { f.db.close() }
        }
        test("awaited_merge_hook_failure_is_returned_without_background_fact") {
            val f = Fixture(); f.open()
            val metadata = obj("fields" to obj(), "tombstone" to ProtocolValue.Null)
            val row = Row("remote", emptyMap()).toProtocolValue()
            f.replyOverride = { op, arguments -> when (op) {
                "schema" -> notesSchema().toProtocolValue()
                "merge_prepare" -> obj("session" to str("8"), "step" to str("1"), "kind" to str("offer"), "table" to str("notes"), "record_id" to str("remote"))
                "merge_input" -> when (arguments.getValue("part").string()) {
                    "current" -> ProtocolValue.Null
                    "metadata" -> metadata
                    "incoming" -> EntityChange("notes", "remote", EntityOperation.Create, emptyMap(), timestampMs = 1).toProtocolValue()
                    else -> obj("row" to row, "metadata" to metadata)
                }
                else -> null
            } }
            try {
                f.db.setMergeHooks(mapOf("notes" to { throw IOException(CANARY) }))
                try { f.db.pullOnce { _, _ -> PullPage("next", true, emptyList()) }; error("merge accepted") }
                catch (error: HoyaException) { check(error.kind == "InvalidCommand") }
                check(f.count("merge_input") == 4 && f.count("abort_pull") == 1 && f.facts.isEmpty())
            } finally { f.db.close() }
        }
        test("automatic_sync_failure_preserves_onError_and_contains_both_sinks") {
            val f = Fixture(throwSink = true); f.open(); f.envelopes = listOf("{}")
            val adapter = AtomicInteger()
            try {
                f.db.startSync({ throw IOException(CANARY) }, now = { 0 }, jitter = { 0 }, onError = { adapter.incrementAndGet(); throw AssertionError(CANARY) })
                f.checkFacts(Triple(BackgroundError.Source.Sync, "BackendFailure", null))
                eventually { adapter.get() == 1 }
                check(f.facts.size == 1)
            } finally { f.db.close() }
        }
        test("sync_timer_failure_uses_timer_source_and_preserves_error_adapter") {
            val timer = object : HoyaTimer { override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable { throw IOException(CANARY) } }
            val f = Fixture(timer = timer); f.open(); f.due = 1000
            val adapter = AtomicInteger()
            try {
                f.db.startSync({ error("unexpected transport") }, now = { 0 }, jitter = { 0 }, onError = { adapter.incrementAndGet() })
                f.checkFacts(Triple(BackgroundError.Source.Timer, "BackendFailure", null))
                eventually { adapter.get() == 1 }
                check(f.facts.size == 1)
            } finally { f.db.close() }
        }
        test("queued_old_epoch_facts_are_suppressed_after_close_and_reset") {
            for (reset in listOf(false, true)) {
                val queue = QueuedExecutor(); val f = Fixture(executor = queue); f.open(); f.envelopes = listOf("{}")
                val attempted = CountDownLatch(1)
                f.db.startSync({ throw IOException(CANARY) }, now = { 0 }, jitter = { 0 }, onError = { attempted.countDown() })
                await(attempted); eventually { queue.work.isNotEmpty() }
                if (reset) f.db.reset(ResetMode.DiscardRecovery) else { f.db.close(); f.open() }
                queue.drain(); check(f.facts.isEmpty())
                val next = CountDownLatch(1)
                f.db.startSync({ throw IOException(CANARY) }, now = { 0 }, jitter = { 0 }, onError = { next.countDown() })
                await(next); eventually { queue.work.isNotEmpty() }; queue.drain()
                f.checkFacts(Triple(BackgroundError.Source.Sync, "BackendFailure", null))
                f.db.close()
            }
        }
        test("inline_sink_retirement_suppresses_later_sync_error_adapter") {
            for (reset in listOf(false, true)) {
                lateinit var database: Database
                val retired = CountDownLatch(1); val adapter = CountDownLatch(1)
                val f = Fixture(sinkAction = {
                    blocking { if (reset) database.reset(ResetMode.DiscardRecovery) else database.close() }
                    retired.countDown()
                })
                database = f.db; f.open(); f.envelopes = listOf("{}")
                try {
                    f.db.startSync({ throw IOException(CANARY) }, now = { 0 }, jitter = { 0 }, onError = { adapter.countDown() })
                    await(retired)
                    check(!adapter.await(1, TimeUnit.SECONDS)) { "retired epoch reached sync error adapter" }
                    check(f.facts.size == 1)
                } finally { f.db.close() }
            }
        }
        test("sync_timer_cancel_failure_is_redacted_without_masking_mutation") {
            val scheduled = CountDownLatch(1)
            val timer = object : HoyaTimer {
                override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable {
                    scheduled.countDown(); return AutoCloseable { throw IOException(CANARY) }
                }
            }
            val f = Fixture(timer = timer); f.open(); f.due = 1000
            try {
                f.db.startSync({ error("unexpected transport") }, now = { 0 }, jitter = { 0 })
                await(scheduled)
                // A committed mutation wakes sync and cancels its previous due timer.
                eventually { synchronized(f.db.stateLock) { f.db.syncDriver != null } }
                val row = f.db.insert("notes", emptyMap(), "committed")
                check(row.id == "committed")
                f.checkFacts(Triple(BackgroundError.Source.Timer, "BackendFailure", null))
            } finally { f.db.close() }
        }
        test("observation_timer_cancel_failure_keeps_other_buffered_listener_and_releases_count") {
            val scheduled = CountDownLatch(1); val cancels = AtomicInteger()
            val timer = object : HoyaTimer {
                override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable {
                    scheduled.countDown(); return AutoCloseable { cancels.incrementAndGet(); throw IOException(CANARY) }
                }
            }
            val f = Fixture(timer = timer); f.open(); val healthy = AtomicInteger()
            try {
                f.db.observe(Query("notes"), ObservationMode.Count) { check(it is ObservedValue.Terminated) }
                f.db.observe(Query("notes")) { healthy.incrementAndGet() }
                eventually { synchronized(f.db.stateLock) { f.db.drainGuard == 2 } }
                f.db.insert("notes", emptyMap(), "committed"); await(scheduled)
                eventually { synchronized(f.db.stateLock) { f.db.drainGuard == 2 } }
                f.events.set(listOf(terminal(), rows("8")))
                f.db.mutated()
                f.checkFacts(Triple(BackgroundError.Source.Observation, "LimitExceeded", "7"), Triple(BackgroundError.Source.Timer, "BackendFailure", null))
                eventually { healthy.get() == 1 && synchronized(f.db.stateLock) { f.db.drainGuard == 1 } }
                check(cancels.get() == 1 && f.count("unsubscribe") == 0)
                check(f.facts.size == 2)
            } finally { f.db.close() }
        }
        test("late_managed_drain_failure_after_close_is_suppressed") {
            val f = Fixture(); f.open(); f.holdDrain = true
            f.db.observe(Query("notes")) { error("unexpected callback") }
            eventually { f.heldDrain.get() != null }
            f.db.close(); f.heldDrain.getAndSet(null).onFailure(CANARY)
            eventually { synchronized(f.db.stateLock) { f.db.drainGuard == 0 } }
            check(f.facts.isEmpty())
        }
        test("five_source_model_and_dormant_merge_helper_redact_unknown_kind") {
            val f = Fixture(); f.open()
            try {
                check(BackgroundError.Source.entries.size == 5)
                f.db.reportBackground(BackgroundError.Source.Merge, CANARY, null, f.db.current().second)
                f.checkFacts(Triple(BackgroundError.Source.Merge, "BackendFailure", null))
            } finally { f.db.close() }
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}

// Independent entry points keep fatal containment regressions from hiding later cases.
object BackgroundTimerErrorTests {
    @JvmStatic fun main(args: Array<String>) = BackgroundErrorTests.main(arrayOf(
        "observation_timer_cancel_failure_keeps_other_buffered_listener_and_releases_count"))
}
object BackgroundSinkThrowableTests {
    @JvmStatic fun main(args: Array<String>) = BackgroundErrorTests.main(arrayOf(
        "automatic_sync_failure_preserves_onError_and_contains_both_sinks"))
}
object BackgroundSinkRetirementTests {
    @JvmStatic fun main(args: Array<String>) = BackgroundErrorTests.main(arrayOf(
        "inline_sink_retirement_suppresses_later_sync_error_adapter"))
}
object BackgroundManualCleanupTests {
    @JvmStatic fun main(args: Array<String>) = BackgroundErrorTests.main(arrayOf(
        "manual_subscription_close_preserves_prior_silent_cleanup_failure"))
}
