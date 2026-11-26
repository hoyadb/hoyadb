// TC-0036 K01/K02: run on actual Android platform, no mock database or JSON engine.
package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.Executor

object SdkTests {
    private fun expect(kind: String, body: () -> Unit) {
        try { body() } catch (error: HoyaException) { check(error.kind == kind); return }
        error("expected static failure")
    }

    private fun nativeCall(frame: ProtocolValue): ByteArray {
        val complete = CountDownLatch(1)
        val value = AtomicReference<ByteArray>()
        val failure = AtomicReference<String>()
        HoyaNativeTransport.call(WireCodec.encode(frame), object : HoyaNativeTransport.Reply {
            override fun onSuccess(bytes: ByteArray) { value.set(bytes); complete.countDown() }
            override fun onFailure(kind: String) { failure.set(kind); complete.countDown() }
        })
        check(complete.await(30, TimeUnit.SECONDS))
        failure.get()?.let { throw HoyaException(it) }
        return value.get() ?: error("missing owned native reply")
    }

    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun run(name: String, body: () -> Unit) {
            try { body(); println("PASS $name") }
            catch (error: Throwable) { failures++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) }
        }
        run("K01_i64_and_integral_float_wire_kinds") {
            check(Cell.IntegerValue(Long.MIN_VALUE).toProtocolValue() == ProtocolValue.ObjectValue(mapOf("kind" to ProtocolValue.StringValue("integer"), "value" to ProtocolValue.StringValue("-9223372036854775808"))))
            check(Cell.TimestampValue(Long.MAX_VALUE).toProtocolValue() == ProtocolValue.ObjectValue(mapOf("kind" to ProtocolValue.StringValue("timestamp"), "value" to ProtocolValue.StringValue("9223372036854775807"))))
            val value = Cell.FloatValue(42.0).toProtocolValue() as ProtocolValue.ObjectValue
            check(value.value["kind"] == ProtocolValue.StringValue("float"))
            check((value.value["value"] as ProtocolValue.StringValue).value.toDouble() == 42.0)
            expect("InvalidPayload") { Cell.FloatValue(Double.NaN).toProtocolValue() }
        }
        run("K01_owned_nested_immutable_business_snapshot") {
            val fields = mutableMapOf<String, JSONValue>("captured" to JSONValue.StringValue("synthetic original"))
            val list = mutableListOf<JSONValue>(JSONValue.ObjectValue(fields))
            val captured = JSONValue.ArrayValue(list)
            fields["captured"] = JSONValue.StringValue("later mutation")
            list.clear()
            check(captured.value.size == 1)
            val nested = captured.value[0] as JSONValue.ObjectValue
            check(nested.value["captured"] == JSONValue.StringValue("synthetic original"))
            var rejected = false
            try { (nested.value as MutableMap)["new"] = JSONValue.Null } catch (_: UnsupportedOperationException) { rejected = true }
            check(rejected)
        }
        run("K02_unsigned_full_width_is_not_signed_Java_long") {
            check(UnsignedDecimal.parse("18446744073709551615").value == "18446744073709551615")
            for (invalid in listOf("-1", "+1", "00", "01", "18446744073709551616", " 1", "1 ")) {
                expect("InvalidRequest") { UnsignedDecimal.parse(invalid) }
            }
        }
        run("K02_strict_UTF16_UTF8_and_frame_budgets") {
            val value = ProtocolValue.StringValue("東京🌿")
            check(WireCodec.decode(WireCodec.encode(value)) == value)
            expect("InvalidPayload") { WireCodec.encode(ProtocolValue.StringValue("\uD800")) }
            expect("InvalidPayload") { WireCodec.decode(byteArrayOf(0xC3.toByte(), 0x28)) }
            expect("LimitExceeded") { WireCodec.encode(ProtocolValue.StringValue("x".repeat(1_048_577))) }
            var deep: ProtocolValue = ProtocolValue.Null
            repeat(66) { deep = ProtocolValue.ArrayValue(listOf(deep)) }
            expect("LimitExceeded") { WireCodec.encode(deep) }
            expect("LimitExceeded") { WireCodec.encode(ProtocolValue.ArrayValue(List(100_000) { ProtocolValue.Null })) }
        }
        run("K03_actual_preloaded_ABI_lossless_codec") {
            check(HoyaNativeTransport.abiVersion() == 1)
            val frame = obj("abi" to str("1"), "op" to str("json_decode"), "owner" to ProtocolValue.Null,
                "args" to obj("body" to str("9223372036854775807")))
            val output = nativeCall(frame)
            val reply = WireCodec.decode(output).exact("abi", "ok", "value")
            check(reply.getValue("ok").boolean())
            check(JSONValue.fromProtocolValue(reply.getValue("value")) == JSONValue.IntegerValue(Long.MAX_VALUE))
        }
        run("K08_terminal_failure_releases_after_native_guard_without_begin_spin") {
            check(HoyaNativeTransport.abiVersion() == 1)
            val releases = AtomicInteger()
            val signalled = CountDownLatch(1)
            val subscription = HoyaNativeTransport.subscribeRelease(Executor { it.run() }, Runnable {
                releases.incrementAndGet(); signalled.countDown()
            })
            val ordinary = obj("abi" to str("1"), "op" to str("json_decode"), "owner" to ProtocolValue.Null,
                "args" to obj("body" to str("null")))
            nativeCall(ordinary)
            check(releases.get() == 0)
            val terminal = obj("abi" to str("1"), "op" to str("close"), "owner" to str("18446744073709551615"), "args" to obj())
            val reply = WireCodec.decode(nativeCall(terminal)).exact("abi", "ok", "error")
            check(!reply.getValue("ok").boolean())
            check(signalled.await(30, TimeUnit.SECONDS))
            check(releases.get() == 1)
            subscription.close(); subscription.close()
            nativeCall(terminal)
            check(releases.get() == 1)
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
