package dev.hoyadb

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

object ExplicitDrainTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        try {
            blocking {
                val ui = Executors.newSingleThreadExecutor { Thread(it, "explicit-drain-ui") }
                val tasks = CopyOnWriteArrayList<Runnable>()
                val timer = object : HoyaTimer {
                    override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable {
                        check(delayMilliseconds == 16L); tasks.add(work)
                        return AutoCloseable { tasks.remove(work) }
                    }
                }
                var now = 0L
                val db = Database(ui, uptime = { now }, timer = timer)
                try {
                    db.openMemory(notesSchema(), TEST_NAMESPACE)
                    val initial = CountDownLatch(1)
                    val values = CopyOnWriteArrayList<String>()
                    val subscription = db.observe(Query("notes"), ObservationMode.Count) { value ->
                        check(Thread.currentThread().name == "explicit-drain-ui")
                        check(value is ObservedValue.Count); values.add(value.count.value)
                        if (values.size == 1) initial.countDown()
                        else {
                            try { blocking { db.drainNotifications() }; error("reentrant managed drain") }
                            catch (error: HoyaException) { check(error.kind == "Busy") }
                        }
                    }
                    check(initial.await(10, TimeUnit.SECONDS))
                    db.insert("notes", mapOf("text" to Cell.StringValue("one")), "one")
                    val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
                    while (synchronized(db.stateLock) { db.drainGuard != 1 } && System.nanoTime() < deadline) Thread.sleep(1)
                    check(synchronized(db.stateLock) { db.drainGuard == 1 })
                    check(tasks.size == 1 && values.toList() == listOf("0"))
                    now = 16
                    db.drainNotifications()
                    check(values.toList() == listOf("0", "1"))
                    try { db.execute("drain", mapOf("now_ms" to str("16"))); error("raw consumed managed stream") }
                    catch (error: HoyaException) { check(error.kind == "Busy") }
                    subscription.close()
                    val end = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
                    while (synchronized(db.stateLock) { db.drainGuard != 0 } && System.nanoTime() < end) Thread.sleep(1)
                    db.execute("drain", mapOf("now_ms" to str("16")))
                } finally { db.close(); ui.shutdownNow() }
            }
            println("PASS K06a_managed_explicit_drain_dispatches_due_count_and_reentrant_Busy")
        } catch (error: Throwable) {
            failed++
            println("FAIL K06a_managed_explicit_drain_dispatches_due_count_and_reentrant_Busy " + if (error is HoyaException) error.kind else error.javaClass.simpleName)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
