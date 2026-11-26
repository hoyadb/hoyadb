package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

object TransportTests {
    private fun exchange(action: (HoyaNativeTransport.Reply) -> Unit): ByteArray {
        val done = CountDownLatch(1); val result = AtomicReference<ByteArray>(); val failure = AtomicReference<String>()
        action(object : HoyaNativeTransport.Reply {
            override fun onSuccess(bytes: ByteArray) { check(Thread.currentThread().name != "main"); result.set(bytes); done.countDown() }
            override fun onFailure(kind: String) { failure.set(kind); done.countDown() }
        })
        check(done.await(30, TimeUnit.SECONDS)); failure.get()?.let { throw HoyaException(it) }
        return result.get() ?: error("missing reply")
    }
    private fun value(bytes: ByteArray): ProtocolValue {
        val reply = WireCodec.decode(bytes).fields()
        if (!reply.getValue("ok").boolean()) throw HoyaException(reply.getValue("error").fields().getValue("kind").string())
        return reply.getValue("value")
    }
    private fun frame(): ByteArray = WireCodec.encode(obj("abi" to str("1"), "op" to str("open"), "owner" to ProtocolValue.Null,
        "args" to obj("engine" to obj("kind" to str("memory"), "store" to ProtocolValue.Null),
            "schema" to obj("version" to str("1"), "tables" to obj("notes" to obj("columns" to obj("text" to obj("kind" to str("String"), "optional" to ProtocolValue.BooleanValue(false))), "relations" to obj()))),
            "migrations" to ProtocolValue.ArrayValue(emptyList()), "namespace" to str("1234567890abcdef1234567890abcdef"),
            "options" to obj("capacity" to str("1000"), "history" to str("100"), "initial_delay_ms" to str("1000"), "multiplier" to str("2"), "max_delay_ms" to str("60000")))))
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, run: () -> Unit) { try { run(); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        test("K02_actual_readonly_original_frame_validation") {
            val valid = frame(); value(exchange { HoyaNativeTransport.validateOpen(valid, 0, it) })
            for (invalid in listOf(WireCodec.utf8(WireCodec.utf8(valid).replace("\"abi\":\"1\"", "\"abi\":\"1\",\"abi\":\"1\"")),
                    WireCodec.utf8(WireCodec.utf8(valid).replace("\"version\":\"1\"", "\"version\":1")), byteArrayOf(0xC3.toByte(), 0x28))) {
                try {
                    val reply = WireCodec.decode(exchange { HoyaNativeTransport.validateOpen(invalid, 0, it) }).fields()
                    check(!reply.getValue("ok").boolean())
                } catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
            }
        }
        test("K03_actual_owned_open_call_close_binary_boundary") {
            val input = frame(); val key = byteArrayOf(); val done = CountDownLatch(1); val result = AtomicReference<ByteArray>(); val failure = AtomicReference<String>()
            HoyaNativeTransport.open(input, key, object : HoyaNativeTransport.Reply {
                override fun onSuccess(bytes: ByteArray) { check(Thread.currentThread().name != "main"); result.set(bytes); done.countDown() }
                override fun onFailure(kind: String) { failure.set(kind); done.countDown() }
            }); input.fill(0)
            check(done.await(30, TimeUnit.SECONDS)); failure.get()?.let { throw HoyaException(it) }
            val owner = value(result.get()).unsigned()
            val protected = value(exchange { HoyaNativeTransport.call(WireCodec.encode(obj("abi" to str("1"), "op" to str("protection"), "owner" to str(owner.value), "args" to obj())), it) }).fields()
            check(!protected.getValue("encrypted").boolean()); check(protected.getValue("backup").string() == "not_applicable")
            value(exchange { HoyaNativeTransport.call(WireCodec.encode(obj("abi" to str("1"), "op" to str("close"), "owner" to str(owner.value), "args" to obj())), it) })
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
