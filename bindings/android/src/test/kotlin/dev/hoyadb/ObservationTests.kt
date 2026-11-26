package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicReference

object ObservationTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName + " line=" + error.stackTrace.firstOrNull { it.className.contains("ObservationTests") }?.lineNumber) } }
        test("K06_initial_callback_selected_executor_cancel_and_reentrant_drain") {
            val ui = Executors.newSingleThreadExecutor { Thread(it, "test-selected-ui") }
            val runtime = object : NativeCalling {
                override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = NativeRuntime.open(frame, key, reply)
                override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
                    val operation = WireCodec.decode(frame).fields().getValue("op").string()
                    println("TRACE observation operation=" + operation)
                    NativeRuntime.call(frame, object : HoyaNativeTransport.Reply {
                        override fun onSuccess(bytes: ByteArray) {
                            val value = WireCodec.decode(bytes).fields()
                            println("TRACE observation completed=" + operation + " ok=" + value.getValue("ok").boolean())
                            reply.onSuccess(bytes)
                        }
                        override fun onFailure(kind: String) { println("TRACE observation boundary=" + kind); reply.onFailure(kind) }
                    })
                }
            }
            val db = Database(runtime, ui, { "one" }, DatabaseOptions())
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val initial = CountDownLatch(1); val update = CountDownLatch(1); val failure = AtomicReference<Throwable>()
                var count = 0; var subscription: Subscription? = null
                subscription = db.observe(Query("notes")) { value ->
                    try {
                        check(Thread.currentThread().name == "test-selected-ui")
                        check(value is ObservedValue.Rows)
                        count++
                        try { blocking { db.execute("drain", mapOf("now_ms" to str("0"))) }; error("reentrant drain") } catch (error: HoyaException) { check(error.kind == "Busy") }
                        if (count == 1) initial.countDown() else { subscription!!.close(); update.countDown() }
                    } catch (error: Throwable) { failure.set(error); initial.countDown(); update.countDown() }
                }
                check(initial.await(30, TimeUnit.SECONDS)); failure.get()?.let { throw it }
                db.insert("notes", mapOf("text" to Cell.StringValue("changed")))
                check(update.await(30, TimeUnit.SECONDS)); failure.get()?.let { throw it }
                subscription.close(); check(count == 2)
            } finally { db.close(); ui.shutdownNow() }
        }
        test("K06_count_deadline_is_single_unpostponed_injected_timer") {
            val tasks = java.util.concurrent.CopyOnWriteArrayList<Runnable>()
            val delays = java.util.concurrent.CopyOnWriteArrayList<Long>()
            val timer = object : HoyaTimer {
                override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable { delays.add(delayMilliseconds); tasks.add(work); return AutoCloseable { tasks.remove(work) } }
            }
            var now = 0L
            val ui = Executors.newSingleThreadExecutor { Thread(it, "test-count-ui") }
            val db = Database(ui, { "one" }, uptime = { now }, timer = timer)
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val initial = CountDownLatch(1); val delivered = CountDownLatch(1)
                val values = java.util.concurrent.CopyOnWriteArrayList<String>()
                val sub = db.observe(Query("notes"), ObservationMode.Count) { value ->
                    check(value is ObservedValue.Count); values.add(value.count.value)
                    if (values.size == 1) initial.countDown() else delivered.countDown()
                }
                check(initial.await(30, TimeUnit.SECONDS)); check(values.toList() == listOf("0"))
                db.insert("notes", mapOf("text" to Cell.StringValue("first"))); now = 8
                db.replace("notes", "one", mapOf("text" to Cell.StringValue("second")))
                check(tasks.size == 1 && delays.toList() == listOf(16L))
                now = 16; tasks.removeAt(0).run(); check(delivered.await(30, TimeUnit.SECONDS))
                check(values.toList() == listOf("0", "1")); sub.close()
            } finally { db.close(); ui.shutdownNow() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
