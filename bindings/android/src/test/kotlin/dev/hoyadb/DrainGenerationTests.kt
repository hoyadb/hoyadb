package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

// DEF-0019 / a9080bd. Source-prepared during quiet; actual RED must precede fix.
object DrainGenerationTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        try {
            blocking {
                val clockEntered = CountDownLatch(1)
                val releaseClock = CountDownLatch(1)
                val firstClock = AtomicBoolean(true)
                val oldCallbacks = AtomicInteger()
                val db = Database(Executor { it.run() }, uptime = {
                    if (firstClock.getAndSet(false)) {
                        clockEntered.countDown()
                        check(releaseClock.await(20, TimeUnit.SECONDS))
                    }
                    0L
                })
                try {
                    db.openMemory(notesSchema(), TEST_NAMESPACE)
                    val old = db.observe(Query("notes")) { oldCallbacks.incrementAndGet() }
                    check(clockEntered.await(10, TimeUnit.SECONDS))
                    // Old drain already captured its owner; no native drain was
                    // submitted yet. Replace that owner while its clock is held.
                    db.close()
                    db.openMemory(notesSchema(), "ffeeddccbbaa99887766554433221100")
                    val replacement = db.execute("observe", mapOf(
                        "query" to Query("notes").toProtocolValue(),
                        "mode" to str("membership"),
                        "columns" to ProtocolValue.ArrayValue(emptyList()))).unsigned()
                    releaseClock.countDown()
                    val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
                    while (synchronized(db.stateLock) { db.drainGuard != 0 } && System.nanoTime() < deadline) Thread.sleep(1)
                    check(synchronized(db.stateLock) { db.drainGuard == 0 })
                    val reply = db.execute("drain", mapOf("now_ms" to str("0"))).exact("events", "has_more")
                    val events = reply.getValue("events").items()
                    check(events.size == 1) { "replacement initial event was consumed by retired drain" }
                    check(events.single().fields().getValue("subscription").unsigned() == replacement)
                    check(!reply.getValue("has_more").boolean())
                    check(oldCallbacks.get() == 0)
                    old.close()
                    db.execute("unsubscribe", mapOf("subscription" to str(replacement.value)))
                } finally { releaseClock.countDown(); db.close() }
            }
            println("PASS DEF19_retired_clock_gated_drain_cannot_consume_replacement_initial_event")
        } catch (error: Throwable) {
            failed++
            println("FAIL DEF19_retired_clock_gated_drain_cannot_consume_replacement_initial_event " +
                if (error is HoyaException) error.kind else error.javaClass.simpleName + " line=" + error.stackTrace.firstOrNull { it.className.contains("DrainGenerationTests") }?.lineNumber)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
