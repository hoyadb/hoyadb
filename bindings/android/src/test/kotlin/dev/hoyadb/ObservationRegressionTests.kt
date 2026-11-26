package dev.hoyadb

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

// API-0011 §7 managed observation: registration/drain ordering and the count deadline timer.
// Controlled native replies prove host delivery and scheduling only, never Rust behavior.
object ObservationRegressionTests {
    private fun eventually(body: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!body()) { check(System.nanoTime() < deadline) { "condition not reached" }; Thread.sleep(1) }
    }
    private class Fixture : NativeCalling {
        val events = AtomicReference<List<ProtocolValue>>(emptyList())
        val heldDrain = AtomicReference<HoyaNativeTransport.Reply>()
        val heldObserve = AtomicReference<HoyaNativeTransport.Reply>()
        val drains = AtomicInteger()
        private val nextSubscription = AtomicInteger(6)
        @Volatile var holdDrain = false
        @Volatile var holdObserve = false
        val tasks = CopyOnWriteArrayList<Runnable>()
        val delays = CopyOnWriteArrayList<Long>()
        val timer = object : HoyaTimer {
            override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable { delays.add(delayMilliseconds); tasks.add(work); return AutoCloseable { tasks.remove(work) } }
        }
        val db = Database(this, Executor { it.run() }, null, DatabaseOptions(), { 0 }, timer)
        fun success(reply: HoyaNativeTransport.Reply, value: ProtocolValue) {
            reply.onSuccess(WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to value)))
        }
        fun page() = obj("events" to ProtocolValue.ArrayValue(events.getAndSet(emptyList())), "has_more" to ProtocolValue.BooleanValue(false))
        fun releaseDrain() { success(checkNotNull(heldDrain.getAndSet(null)), page()) }
        fun releaseObserve() { success(checkNotNull(heldObserve.getAndSet(null)), str(nextSubscription.incrementAndGet().toString())) }
        override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) { success(reply, str("1")) }
        override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
            when (WireCodec.decode(frame).fields().getValue("op").string()) {
                "observe" -> if (holdObserve) { holdObserve = false; check(heldObserve.compareAndSet(null, reply)) } else success(reply, str(nextSubscription.incrementAndGet().toString()))
                "drain" -> { drains.incrementAndGet(); if (holdDrain) { holdDrain = false; check(heldDrain.compareAndSet(null, reply)) } else success(reply, page()) }
                "insert" -> success(reply, Row("committed", emptyMap()).toProtocolValue())
                else -> success(reply, ProtocolValue.Null)
            }
        }
        suspend fun open() { db.openMemory(notesSchema(), TEST_NAMESPACE) }
        suspend fun write() { db.insert("notes", mapOf("text" to Cell.StringValue("x"))) }
        fun idle() = eventually { synchronized(db.stateLock) { db.drainGuard } == listeners }
        @Volatile var listeners = 0
    }
    private fun rows(token: String) = obj("kind" to str("data"), "subscription" to str(token), "value" to obj("kind" to str("rows"), "rows" to ProtocolValue.ArrayValue(emptyList())))
    private fun count(token: String, value: String) = obj("kind" to str("data"), "subscription" to str(token), "value" to obj("kind" to str("count"), "count" to str(value)))

    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun test(name: String, body: suspend () -> Unit) {
            if (args.isNotEmpty() && name !in args) return
            try { blocking(body); println("PASS $name") }
            catch (error: Throwable) {
                failures++
                println("FAIL $name ${error.javaClass.simpleName} line=" + error.stackTrace.firstOrNull { it.className.contains("ObservationRegressionTests") }?.lineNumber)
            }
        }
        test("drain_reply_during_registration_is_buffered_for_the_new_listener") {
            val f = Fixture(); f.open()
            try {
                val first = AtomicInteger(); val second = AtomicInteger()
                f.db.observe(Query("notes")) { first.incrementAndGet() }
                f.listeners = 1; f.idle()
                // A managed drain is in flight when the second registration starts.
                f.holdDrain = true; f.write()
                eventually { f.heldDrain.get() != null }
                f.holdObserve = true
                val registered = AtomicReference<Any>()
                launchManaged({ registered.set(f.db.observe(Query("notes")) { second.incrementAndGet() }) }, { registered.set(it) })
                eventually { f.heldObserve.get() != null }
                // Core already queued the new subscription's initial snapshot (token 8) and the
                // in-flight drain returns it before the registration reply is handled.
                f.events.set(listOf(rows("8"), rows("7")))
                f.releaseDrain()
                Thread.sleep(50)
                check(second.get() == 0 && first.get() == 0) { "delivered while a registration was pending" }
                f.releaseObserve()
                eventually { registered.get() != null }
                check(registered.get() is Subscription)
                eventually { second.get() == 1 }
                check(first.get() == 1)
            } finally { f.db.close() }
        }
        test("no_managed_or_explicit_drain_starts_while_registration_is_pending") {
            val f = Fixture(); f.open()
            try {
                f.db.observe(Query("notes")) { }
                f.listeners = 1; f.idle()
                val before = f.drains.get()
                f.holdObserve = true
                val registered = AtomicReference<Any>()
                launchManaged({ registered.set(f.db.observe(Query("notes")) { }) }, { registered.set(it) })
                eventually { f.heldObserve.get() != null }
                f.write()
                try { f.db.drainNotifications(); error("explicit drain during registration") } catch (error: HoyaException) { check(error.kind == "Busy") }
                Thread.sleep(50)
                check(f.drains.get() == before)
                f.releaseObserve()
                eventually { registered.get() is Subscription }
                eventually { f.drains.get() > before }
            } finally { f.db.close() }
        }
        test("deadline_before_core_window_arms_one_follow_up_timer_then_stops") {
            val f = Fixture(); f.open()
            try {
                val values = CopyOnWriteArrayList<String>()
                f.db.observe(Query("notes"), ObservationMode.Count) { if (it is ObservedValue.Count) values.add(it.count.value) }
                f.listeners = 1; f.idle()
                // The write's drain is sampled after the timer was armed: it is still in flight.
                f.holdDrain = true; f.write()
                eventually { f.heldDrain.get() != null }
                f.write()
                check(f.delays.toList() == listOf(16L))
                f.releaseDrain(); f.idle()
                // The deadline drain returns no count: the core's window has not ended yet.
                f.tasks.removeAt(0).run(); f.idle()
                eventually { f.delays.size == 2 }
                check(f.delays.toList() == listOf(16L, 16L))
                f.events.set(listOf(count("7", "2")))
                f.tasks.removeAt(0).run(); f.idle()
                eventually { values.toList() == listOf("2") }
                Thread.sleep(50)
                check(f.delays.toList() == listOf(16L, 16L) && f.tasks.isEmpty())
            } finally { f.db.close() }
        }
        test("deadline_without_delivery_or_later_drain_does_not_poll") {
            val f = Fixture(); f.open()
            try {
                f.db.observe(Query("notes"), ObservationMode.Count) { }
                f.listeners = 1; f.idle()
                f.holdDrain = true; f.write()
                eventually { f.heldDrain.get() != null }
                f.write(); f.releaseDrain(); f.idle()
                f.tasks.removeAt(0).run(); f.idle()
                eventually { f.delays.size == 2 }
                // No drain was sampled during the follow-up window: the sequence ends.
                f.tasks.removeAt(0).run(); f.idle()
                Thread.sleep(50)
                check(f.delays.toList() == listOf(16L, 16L) && f.tasks.isEmpty())
            } finally { f.db.close() }
        }
        test("deadline_drain_that_delivers_the_count_arms_no_follow_up") {
            val f = Fixture(); f.open()
            try {
                val values = CopyOnWriteArrayList<String>()
                f.db.observe(Query("notes"), ObservationMode.Count) { if (it is ObservedValue.Count) values.add(it.count.value) }
                f.listeners = 1; f.idle()
                f.holdDrain = true; f.write()
                eventually { f.heldDrain.get() != null }
                f.write(); f.releaseDrain(); f.idle()
                f.events.set(listOf(count("7", "2")))
                f.tasks.removeAt(0).run(); f.idle()
                eventually { values.toList() == listOf("2") }
                Thread.sleep(50)
                check(f.delays.toList() == listOf(16L))
            } finally { f.db.close() }
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
