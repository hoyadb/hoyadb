package dev.hoyadb

import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

// DEF-0022 / c40c746: controlled custom capability failure, no default failure claim.
object TimerFailureTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        try {
            blocking {
                val calls = AtomicInteger()
                val tasks = CopyOnWriteArrayList<Runnable>()
                val timer = object : HoyaTimer {
                    override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable {
                        if (calls.incrementAndGet() == 1) throw IOException("synthetic timer private canary")
                        check(delayMilliseconds == 16L)
                        tasks.add(work)
                        return AutoCloseable { tasks.remove(work) }
                    }
                }
                var now = 0L
                val ui = Executors.newSingleThreadExecutor { Thread(it, "timer-failure-ui") }
                val db = Database(ui, uptime = { now }, timer = timer)
                try {
                    db.openMemory(notesSchema(), TEST_NAMESPACE)
                    val initial = CountDownLatch(1)
                    val old = db.observe(Query("notes"), ObservationMode.Count) { initial.countDown() }
                    check(initial.await(10, TimeUnit.SECONDS))
                    // Native insert succeeds before the injected scheduler throws.
                    val committed = db.insert("notes", mapOf("text" to Cell.StringValue("committed")), "one")
                    check(committed.id == "one" && db.fetch(Query("notes")).single().id == "one")
                    old.close()
                    val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
                    while (true) {
                        val guard = synchronized(db.stateLock) { db.drainGuard }
                        val observers = db.execute("observation_stats").fields().getValue("observers").unsigned().value
                        if (guard == 0 && observers == "0") break
                        check(System.nanoTime() < deadline); Thread.sleep(1)
                    }
                    db.execute("drain", mapOf("now_ms" to str("0")))
                    val ready = CountDownLatch(1); val delivered = CountDownLatch(1)
                    val values = CopyOnWriteArrayList<String>()
                    val replacement = db.observe(Query("notes"), ObservationMode.Count) { value ->
                        check(Thread.currentThread().name == "timer-failure-ui")
                        check(value is ObservedValue.Count)
                        values.add(value.count.value)
                        if (values.size == 1) ready.countDown() else delivered.countDown()
                    }
                    check(ready.await(10, TimeUnit.SECONDS))
                    db.insert("notes", mapOf("text" to Cell.StringValue("second")), "two")
                    check(calls.get() == 2 && tasks.size == 1)
                    now = 16; tasks.removeAt(0).run()
                    check(delivered.await(10, TimeUnit.SECONDS) && values.toList() == listOf("1", "2"))
                    replacement.close()
                    check(!db.diagnostics().toString().contains("synthetic timer private canary"))
                } finally { db.close(); ui.shutdownNow() }
            }
            println("PASS DEF22_custom_timer_exception_preserves_committed_Row_and_normal_replacement_deadline")
        } catch (error: Throwable) {
            failed++
            println("FAIL DEF22_custom_timer_exception_preserves_committed_Row_and_normal_replacement_deadline " +
                if (error is HoyaException) error.kind else error.javaClass.simpleName + " line=" + error.stackTrace.firstOrNull { it.className.contains("TimerFailureTests") }?.lineNumber)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
