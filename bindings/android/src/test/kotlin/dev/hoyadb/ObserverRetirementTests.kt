package dev.hoyadb

import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

// DEF-0021 / 9a17b9c. Source only until the quiet window ends and RED is observed.
object ObserverRetirementTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        try {
            blocking {
                val ui = Executors.newSingleThreadExecutor { Thread(it, "observer-retirement-ui") }
                val db = Database(ui)
                try {
                    db.openMemory(notesSchema(), TEST_NAMESPACE)
                    val attempted = CountDownLatch(1)
                    val subscription = db.observe(Query("notes")) {
                        check(Thread.currentThread().name == "observer-retirement-ui")
                        attempted.countDown()
                        throw IOException("synthetic callback canary")
                    }
                    check(attempted.await(10, TimeUnit.SECONDS))
                    // A marker behind delivery ensures the exception was caught
                    // before explicit cancellation tests its cleanup path.
                    ui.submit {}.get(10, TimeUnit.SECONDS)
                    subscription.close()
                    subscription.close()
                    val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
                    while (true) {
                        val guard = synchronized(db.stateLock) { db.drainGuard }
                        val observers = db.execute("observation_stats").fields().getValue("observers").unsigned().value
                        if (guard == 0 && observers == "0") break
                        check(System.nanoTime() < deadline) { "throwing listener retained registration" }
                        Thread.sleep(1)
                    }
                    check(db.execute("drain", mapOf("now_ms" to str("0"))).fields().getValue("events").items().isEmpty())
                    val control = CountDownLatch(1)
                    val normal = db.observe(Query("notes")) { control.countDown() }
                    check(control.await(10, TimeUnit.SECONDS)); normal.close()
                    db.close()
                    db.openMemory(notesSchema(), "ffeeddccbbaa99887766554433221100")
                    check(db.execute("drain", mapOf("now_ms" to str("0"))).fields().getValue("events").items().isEmpty())
                    check(db.execute("observation_stats").fields().getValue("observers").unsigned().value == "0")
                } finally { db.close(); ui.shutdownNow() }
            }
            println("PASS DEF21_throwing_observer_explicit_close_releases_token_guard_and_reopen")
        } catch (error: Throwable) {
            failed++
            println("FAIL DEF21_throwing_observer_explicit_close_releases_token_guard_and_reopen " +
                if (error is HoyaException) error.kind else error.javaClass.simpleName + " line=" + error.stackTrace.firstOrNull { it.className.contains("ObserverRetirementTests") }?.lineNumber)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
