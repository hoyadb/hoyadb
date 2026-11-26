// DEF-0013 / TC-0036 G10: never install a global uncaught-exception handler.
package dev.hoyadb

import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

object ReleaseFailureTests {
    private val terminal = obj("abi" to str("1"), "op" to str("close"), "owner" to str("18446744073709551615"), "args" to obj())
    private fun call(frame: ProtocolValue, timeout: Long = 10): ProtocolValue {
        val done = CountDownLatch(1); val reply = AtomicReference<ByteArray>(); val failed = AtomicReference<String>()
        HoyaNativeTransport.call(WireCodec.encode(frame), object : HoyaNativeTransport.Reply {
            override fun onSuccess(bytes: ByteArray) { reply.set(bytes); done.countDown() }
            override fun onFailure(kind: String) { failed.set(kind); done.countDown() }
        })
        check(done.await(timeout, TimeUnit.SECONDS)); failed.get()?.let { throw HoyaException(it) }
        return WireCodec.decode(reply.get())
    }
    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun test(name: String, body: () -> Unit) { try { body(); println("PASS $name") } catch (error: Throwable) { failures++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        val ordinary = obj("abi" to str("1"), "op" to str("json_decode"), "owner" to ProtocolValue.Null, "args" to obj("body" to str("null")))
        test("DEF0013_inline_checked_exception_preserves_original_reply_and_later_listener") {
            call(ordinary)
            val nativeThread = Thread.getAllStackTraces().keys.single { it.name == "hoyadb-native" }
            val previous = nativeThread.uncaughtExceptionHandler
            // Test-only, exact owned worker. Avoid Android's process-wide crash
            // policy obscuring the expected assertion; never change a default.
            nativeThread.uncaughtExceptionHandler = Thread.UncaughtExceptionHandler { _, _ -> }
            val bad = AtomicInteger(); val later = AtomicInteger()
            val badSub = HoyaNativeTransport.subscribeRelease(Executor { it.run() }, Runnable { bad.incrementAndGet(); throw IOException("synthetic checked callback failure") })
            val goodSub = HoyaNativeTransport.subscribeRelease(Executor { it.run() }, Runnable { later.incrementAndGet() })
            try {
                val value = call(terminal, 2).exact("abi", "ok", "error")
                check(!value.getValue("ok").boolean()); check(value.getValue("error").exact("kind").getValue("kind").string() == "InvalidHandle")
                call(terminal); check(bad.get() == 1 && later.get() == 2)
                check(call(ordinary).fields().getValue("ok").boolean())
            } finally { badSub.close(); goodSub.close(); nativeThread.uncaughtExceptionHandler = previous }
        }
        test("DEF0013_async_checked_exception_retires_only_bad_listener") {
            val uncaught = AtomicInteger(); val calls = AtomicInteger(); val first = CountDownLatch(1)
            val executor = Executors.newSingleThreadExecutor { work -> Thread(work, "test-owned-release").apply { uncaughtExceptionHandler = Thread.UncaughtExceptionHandler { _, _ -> uncaught.incrementAndGet(); first.countDown() } } }
            val subscription = HoyaNativeTransport.subscribeRelease(executor, Runnable { calls.incrementAndGet(); first.countDown(); throw IOException("synthetic asynchronous callback failure") })
            try {
                call(terminal); check(first.await(10, TimeUnit.SECONDS))
                call(terminal)
                val barrier = CountDownLatch(1); executor.execute { barrier.countDown() }; check(barrier.await(10, TimeUnit.SECONDS))
                check(calls.get() == 1 && uncaught.get() == 0)
                check(call(ordinary).fields().getValue("ok").boolean())
            } finally { subscription.close(); executor.shutdownNow() }
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
