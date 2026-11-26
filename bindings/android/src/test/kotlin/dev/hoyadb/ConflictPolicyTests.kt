package dev.hoyadb

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

// SPEC-0040 / TC-0044 CP13: controlled native replies test the real Kotlin host.
// Core policy eligibility, persistence and JNI integration have separate tests.
object ConflictPolicyTests {
    private const val CANARY = "private-resolution-command-key"
    private fun eventually(test: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!test()) { check(System.nanoTime() < deadline) { "expected policy/report behavior missing" }; Thread.sleep(1) }
    }
    private fun await(latch: CountDownLatch) { check(latch.await(5, TimeUnit.SECONDS)) }
    private fun async(work: suspend () -> Unit): Completion<Unit> = Completion<Unit>().also { result ->
        launchManaged({ try { work(); result.succeed(Unit) } catch (error: Throwable) { result.fail(error) } })
    }
    private class QueuedExecutor : Executor {
        val work = LinkedBlockingQueue<Runnable>()
        override fun execute(command: Runnable) { work.add(command) }
        fun drain() { while (true) (work.poll() ?: return).run() }
    }
    private fun kept() = obj("kind" to str("kept_current"), "discarded_keys" to ProtocolValue.ArrayValue(listOf(str("retired"))), "unmapped" to ProtocolValue.ArrayValue(listOf(str("local-id"))), "skipped_writes" to str("0"))
    private class Pending(val owner: String, val args: Map<String, ProtocolValue>, val reply: HoyaNativeTransport.Reply)
    private class Fixture(conflict: ConflictPolicy? = null, executor: Executor = Executor { it.run() },
        resolutionAction: ((String, Resolution) -> Unit)? = null, backgroundAction: ((BackgroundError) -> Unit)? = null) : NativeCalling {
        val frames = CopyOnWriteArrayList<Pair<String, Map<String, ProtocolValue>>>()
        val callbacks = CopyOnWriteArrayList<Pair<String, Resolution>>()
        val facts = CopyOnWriteArrayList<BackgroundError>()
        val owners = ConcurrentHashMap.newKeySet<String>()
        private val handles = AtomicInteger()
        val commits = AtomicInteger()
        val configEntered = CountDownLatch(1)
        val resolutionEntered = CountDownLatch(1)
        val pendingConfig = AtomicReference<Pending>()
        val pendingResolution = AtomicReference<Pending>()
        @Volatile var holdConfig = false
        @Volatile var holdResolution = false
        @Volatile var configFailure: String? = null
        @Volatile var closeFailure: String? = null
        @Volatile var resolutionFailure: String? = null
        @Volatile var report: ProtocolValue = kept()
        @Volatile var installedPolicy: ProtocolValue? = null
        @Volatile var managedPending = false
        val db = Database(this, executor, null, DatabaseOptions(), { 0 }, SystemHoyaTimer,
            { facts.add(it); backgroundAction?.invoke(it) }, conflict,
            { key, result -> callbacks.add(key to result); resolutionAction?.invoke(key, result) })
        fun count(operation: String) = frames.count { it.first == operation }
        fun success(reply: HoyaNativeTransport.Reply, value: ProtocolValue) = reply.onSuccess(WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to value)))
        fun fail(reply: HoyaNativeTransport.Reply, kind: String) = reply.onSuccess(WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(false), "error" to obj("kind" to str(kind)))))
        override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) {
            frames.add("open" to WireCodec.decode(frame).fields().getValue("args").fields())
            val handle = handles.incrementAndGet().toString(); owners.add(handle); success(reply, str(handle))
        }
        override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
            val wire = WireCodec.decode(frame).fields(); val op = wire.getValue("op").string()
            val args = wire.getValue("args").fields(); val owner = wire.getValue("owner").string()
            frames.add(op to args)
            if (op == "close") closeFailure?.let { fail(reply, it); return }
            if (op == "reconcile" && args.keys != setOf("key", "plan")) { fail(reply, "InvalidRequest"); return }
            if (op == "set_conflict_policy") {
                val pending = Pending(owner, args, reply)
                if (holdConfig) { check(pendingConfig.compareAndSet(null, pending)); configEntered.countDown() }
                else finishConfig(pending)
                return
            }
            if (op in setOf("reconcile", "reconcile_guarded", "rollback", "reconcile_resolving", "rollback_resolving", "discard")) {
                resolutionFailure?.let { fail(reply, it); return }
                commits.incrementAndGet(); managedPending = false
                if (holdResolution) { check(pendingResolution.compareAndSet(null, Pending(owner, args, reply))); resolutionEntered.countDown(); return }
                success(reply, if (op in setOf("reconcile", "reconcile_guarded", "rollback")) ProtocolValue.Null else report)
                return
            }
            val value = when (op) {
                "close" -> { check(owners.remove(owner)); ProtocolValue.Null }
                "fetch" -> ProtocolValue.ArrayValue(emptyList())
                "prepare_push" -> obj("envelopes" to ProtocolValue.ArrayValue(emptyList()), "next_due_ms" to ProtocolValue.Null, "has_more" to ProtocolValue.BooleanValue(false))
                "pending_reconciliations" -> ProtocolValue.ArrayValue(if (managedPending) listOf(obj("key" to str("managed"), "outcome" to RemoteOutcome.Applied(JSONValue.Null).toProtocolValue())) else emptyList())
                else -> ProtocolValue.Null
            }
            success(reply, value)
        }
        private fun finishConfig(pending: Pending) {
            val failure = configFailure
            if (failure == null) { installedPolicy = pending.args.getValue("policy"); success(pending.reply, ProtocolValue.Null) }
            else fail(pending.reply, failure)
        }
        fun releaseConfig() { pendingConfig.getAndSet(null)?.let(::finishConfig) }
        fun releaseResolution() { pendingResolution.getAndSet(null)?.let { success(it.reply, report) } }
        suspend fun open() = db.openMemory(notesSchema(), TEST_NAMESPACE)
    }
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, work: suspend () -> Unit) {
            try { blocking(work); println("PASS $name") }
            catch (error: Throwable) { failed++; println("FAIL $name ${error.javaClass.simpleName} line=" + error.stackTrace.firstOrNull { it.className.contains("ConflictPolicyTests") }?.lineNumber) }
        }
        test("CP13_open_installs_policy_before_publication_and_reapplies_on_reopen") {
            val overrides = linkedMapOf("edit-v1" to ConflictReaction.Stop)
            val f = Fixture(ConflictPolicy(ConflictReaction.Discard, overrides)); overrides["new-v1"] = ConflictReaction.Discard
            f.holdConfig = true
            try {
                val opening = async { f.open() }; await(f.configEntered)
                try { f.db.fetch(Query("notes")); error("owner published before policy") } catch (e: HoyaException) { check(e.kind == "Closed") }
                try { f.open(); error("second open admitted") } catch (e: HoyaException) { check(e.kind == "Busy") }
                f.holdConfig = false; f.releaseConfig(); opening.await()
                check(f.frames.map { it.first }.take(2) == listOf("open", "set_conflict_policy"))
                check(f.installedPolicy!!.exact("default", "commands").getValue("commands").fields().keys == setOf("edit-v1"))
                f.db.close(); f.open(); check(f.count("set_conflict_policy") == 2)
            } finally { f.releaseConfig(); f.db.close() }
        }
        test("CP13_failed_configuration_disposes_unpublished_owner_before_return") {
            val f = Fixture(ConflictPolicy(ConflictReaction.Discard)); f.configFailure = "InvalidCommand"
            try {
                try { f.open(); error("invalid policy accepted") } catch (e: HoyaException) { check(e.kind == "InvalidCommand") }
                check(f.owners.isEmpty() && f.count("close") == 1)
                try { f.db.fetch(Query("notes")); error("failed owner published") } catch (e: HoyaException) { check(e.kind == "Closed") }
                f.configFailure = null; f.open(); check(f.owners.size == 1)
            } finally { f.db.close() }
        }
        test("CP13_close_during_configuration_disposes_once_without_publication") {
            val f = Fixture(ConflictPolicy(ConflictReaction.Discard)); f.holdConfig = true
            try {
                val opening = async { f.open() }; await(f.configEntered)
                val closing = async { f.db.close() }
                eventually { synchronized(f.db.stateLock) { f.db.epoch == 2L } }
                f.holdConfig = false; f.releaseConfig()
                try { opening.await(); error("retired open returned success") } catch (e: HoyaException) { check(e.kind == "Closed") }
                closing.await(); check(f.owners.isEmpty() && f.count("close") == 1)
                f.open(); check(f.count("set_conflict_policy") == 2)
            } finally { f.releaseConfig(); f.db.close() }
        }
        test("CP13_failed_configuration_cleanup_retains_owner_for_close_retry") {
            val f = Fixture(ConflictPolicy(ConflictReaction.Discard)); f.configFailure = "InvalidCommand"; f.closeFailure = "BackendFailure"
            try {
                try { f.open(); error("invalid policy accepted") } catch (e: HoyaException) { check(e.kind == "InvalidCommand") }
                check(f.owners.size == 1 && f.count("close") == 1)
                try { f.open(); error("owner leaked into replacement") } catch (e: HoyaException) { check(e.kind == "Busy") }
                f.closeFailure = null; f.db.close(); check(f.owners.isEmpty() && f.count("close") == 2)
                f.configFailure = null; f.open()
            } finally { f.closeFailure = null; f.db.close() }
        }
        test("CP02_policy_update_forwards_exact_wire_and_preserves_core_error") {
            val f = Fixture(); f.open()
            try {
                val initial = ConflictPolicy(ConflictReaction.Discard, mapOf("unknown-v1" to ConflictReaction.Stop))
                f.db.setConflictPolicy(initial)
                check(f.count("set_conflict_policy") == 1)
                check(f.installedPolicy == initial.toProtocolValue())
                f.configFailure = "Busy"
                try { f.db.setConflictPolicy(ConflictPolicy()); error("Busy policy accepted") } catch (e: HoyaException) { check(e.kind == "Busy") }
                check(f.installedPolicy == initial.toProtocolValue())
            } finally { f.db.close() }
        }
        test("CP13_unit_and_advanced_operations_use_reporting_wire_and_notify_once") {
            val f = Fixture(); f.open(); f.frames.clear()
            try {
                val plan = ReconcilePlan(emptyList()); val guard = CorrectionGuard("notes", "one", null)
                f.db.reconcile("one", plan); f.db.reconcileGuarded("two", plan, listOf(guard)); f.db.rollback("three")
                check(f.db.execute("rollback", mapOf("key" to str("four"))) === ProtocolValue.Null)
                check(f.frames.map { it.first } == listOf("reconcile_resolving", "reconcile_resolving", "rollback_resolving", "rollback_resolving"))
                check(f.frames[0].second.getValue("guards").items().isEmpty())
                check(f.frames[1].second.getValue("guards").items().single() == guard.toProtocolValue())
                check(f.callbacks.map { it.first } == listOf("one", "two", "three", "four") && f.commits.get() == 4)
            } finally { f.db.close() }
        }
        test("CP13_explicit_resolving_and_discard_preserve_owned_reports") {
            val f = Fixture(); f.open()
            try {
                val reports = listOf(f.db.reconcileResolving("one", ReconcilePlan(emptyList())), f.db.rollbackResolving("two"), f.db.discard("three"))
                check(reports.all { it is Resolution.KeptCurrent && it.discardedKeys == listOf("retired") && it.unmapped == listOf("local-id") && it.skippedWrites.value == "0" })
                check(f.callbacks.map { it.first } == listOf("one", "two", "three"))
            } finally { f.db.close() }
        }
        test("CP13_resolution_callback_survives_Throwable_and_reports_each_failure_once") {
            val f = Fixture(resolutionAction = { _, _ -> throw AssertionError(CANARY) }, backgroundAction = { throw AssertionError(CANARY) }); f.open()
            try {
                f.db.rollback("one"); f.db.rollback("two")
                eventually { f.facts.size == 2 }
                check(f.commits.get() == 2 && f.callbacks.size == 2)
                check(f.facts.all { it.source == BackgroundError.Source.Callback && it.kind == "CallbackFailure" && it.token == null && !it.toString().contains(CANARY) })
            } finally { f.db.close() }
        }
        test("CP13_managed_resolution_callback_failure_never_reaches_sync_onError") {
            val f = Fixture(resolutionAction = { _, _ -> throw AssertionError(CANARY) }); f.open(); f.managedPending = true
            val errors = AtomicInteger()
            try {
                val driver = f.db.startSync({ error("unexpected transport") }, { ReconcilePlan(emptyList()) }, now = { 0 }, jitter = { 0 }, onError = { errors.incrementAndGet() })
                eventually { f.facts.size == 1 }
                check(f.callbacks.map { it.first } == listOf("managed") && f.commits.get() == 1 && errors.get() == 0)
                check(f.facts.single().source == BackgroundError.Source.Callback)
                driver.close()
            } finally { f.db.close() }
        }
        test("CP13_queued_resolution_callbacks_are_suppressed_after_close_or_reset") {
            for (reset in listOf(false, true)) {
                val executor = QueuedExecutor(); val f = Fixture(executor = executor); f.open()
                try {
                    f.db.rollback("old"); check(executor.work.size == 1)
                    if (reset) f.db.reset(ResetMode.DiscardRecovery) else { f.db.close(); f.open() }
                    executor.drain(); check(f.callbacks.isEmpty() && f.facts.isEmpty())
                    f.db.rollback("new"); executor.drain(); check(f.callbacks.map { it.first } == listOf("new"))
                } finally { f.db.close() }
            }
        }
        test("CP13_queued_callback_failure_facts_are_suppressed_after_close_or_reset") {
            for (reset in listOf(false, true)) {
                val executor = QueuedExecutor(); val f = Fixture(executor = executor, resolutionAction = { _, _ -> throw AssertionError(CANARY) }); f.open()
                try {
                    f.db.rollback("old"); check(executor.work.size == 1)
                    executor.work.remove().run(); check(f.callbacks.size == 1 && f.facts.isEmpty() && executor.work.size == 1)
                    if (reset) f.db.reset(ResetMode.DiscardRecovery) else { f.db.close(); f.open() }
                    executor.drain(); check(f.facts.isEmpty())
                    f.db.rollback("new"); executor.drain()
                    check(f.callbacks.map { it.first } == listOf("old", "new") && f.facts.size == 1)
                    check(f.facts.single().source == BackgroundError.Source.Callback && f.facts.single().token == null)
                } finally { f.db.close() }
            }
        }
        test("CP13_reentrant_retirement_suppresses_callback_failure_fact") {
            for (reset in listOf(false, true)) {
                lateinit var database: Database
                val f = Fixture(resolutionAction = { _, _ ->
                    blocking { if (reset) database.reset(ResetMode.DiscardRecovery) else database.close() }
                    throw AssertionError(CANARY)
                }); database = f.db; f.open()
                try {
                    f.db.rollback("one")
                    check(f.commits.get() == 1 && f.callbacks.size == 1 && f.facts.isEmpty())
                } finally { f.db.close() }
            }
        }
        test("CP13_late_native_resolution_does_not_reach_replacement_epoch") {
            val f = Fixture(); f.open(); f.holdResolution = true
            try {
                val pending = async { f.db.rollback("old") }; await(f.resolutionEntered)
                f.db.close(); f.open(); f.releaseResolution()
                try { pending.await(); error("retired result accepted") } catch (e: HoyaException) { check(e.kind == "Closed") }
                check(f.callbacks.isEmpty() && f.facts.isEmpty())
            } finally { f.releaseResolution(); f.db.close() }
        }
        test("CP03_completed_and_core_errors_do_not_fabricate_resolution_callbacks") {
            val f = Fixture(); f.open(); f.report = obj("kind" to str("completed"))
            try {
                check(f.db.rollbackResolving("completed") === Resolution.Completed)
                for (kind in listOf("RollbackConflict", "Protocol", "Busy", "LimitExceeded", "BackendFailure")) {
                    f.resolutionFailure = kind
                    try { f.db.rollback("refused"); error("refusal accepted") } catch (e: HoyaException) { check(e.kind == kind) }
                }
                check(f.callbacks.isEmpty() && f.facts.isEmpty())
            } finally { f.db.close() }
        }
        test("CP03_legacy_exact_argument_rejection_is_preserved_by_resolution_routing") {
            val f = Fixture(); f.open()
            try {
                try { f.db.execute("reconcile", mapOf("key" to str("one"), "plan" to ReconcilePlan(emptyList()).toProtocolValue(), "guards" to ProtocolValue.ArrayValue(emptyList()))); error("legacy extra argument accepted") }
                catch (e: HoyaException) { check(e.kind == "InvalidRequest") }
                check(f.commits.get() == 0 && f.callbacks.isEmpty())
            } finally { f.db.close() }
        }
        test("CP01_absent_host_policy_preserves_default_open_and_constructor_compatibility") {
            val executor = Executor { it.run() }
            Database(executor); Database(executor) { fact -> check(fact.kind.isNotEmpty()) }
            Database(executor, null, DatabaseOptions(), { 0 }, SystemHoyaTimer, null)
            Database(executor, onBackgroundError = {}, conflict = ConflictPolicy(), onResolution = { _, _ -> })
            val f = Fixture(); f.open(); f.db.close(); f.open(); f.db.close()
            check(f.count("set_conflict_policy") == 0)
        }
        test("CP13_resolution_and_discarded_models_are_strict_owned_and_redacted") {
            val keys = mutableListOf(CANARY); val unmapped = mutableListOf("local")
            val result = Resolution.KeptCurrent(keys, unmapped, UnsignedDecimal.parse("18446744073709551615"))
            keys.clear(); unmapped.clear(); check(result.discardedKeys == listOf(CANARY) && result.unmapped == listOf("local"))
            check(!result.toString().contains(CANARY) && !ConflictPolicy(commands = mapOf(CANARY to ConflictReaction.Stop)).toString().contains(CANARY))
            check((Resolution.fromProtocolValue(obj("kind" to str("kept_current"), "discarded_keys" to ProtocolValue.ArrayValue(emptyList()), "unmapped" to ProtocolValue.ArrayValue(emptyList()), "skipped_writes" to str("18446744073709551615"))) as Resolution.KeptCurrent).skippedWrites.value == "18446744073709551615")
            for (wire in listOf(obj("kind" to str("unknown")), obj("kind" to str("completed"), "extra" to ProtocolValue.Null), obj("kind" to str("kept_current"), "discarded_keys" to ProtocolValue.ArrayValue(emptyList()), "unmapped" to ProtocolValue.ArrayValue(emptyList()), "skipped_writes" to str("18446744073709551616")))) {
                try { Resolution.fromProtocolValue(wire); error("invalid report accepted") } catch (_: HoyaException) { }
            }
            val entry = QueueEntry.fromProtocolValue(obj("kind" to str("command"), "table" to ProtocolValue.Null, "record_id" to ProtocolValue.Null, "key" to str("discarded"), "state" to str("discarded"), "blocked_on" to ProtocolValue.ArrayValue(emptyList()), "awaiting_local" to ProtocolValue.BooleanValue(false), "awaiting_key" to ProtocolValue.Null, "rolled_back" to ProtocolValue.BooleanValue(false), "attempts" to str("0"), "next_due_ms" to ProtocolValue.Null))
            check(entry.state == QueueState.Discarded && !entry.rolledBack)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
