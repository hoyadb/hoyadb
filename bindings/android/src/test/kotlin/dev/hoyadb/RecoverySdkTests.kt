// SPEC-0038 / TC-0042 SDK1-SDK2: a scripted NativeCalling proves Kotlin SDK translation only.
// Mirrors SDKRecoveryTests.swift. Written while JVM/Gradle/Android execution is held: UNEXECUTED.
package dev.hoyadb

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

private const val FROZEN = "{\"idempotencyKey\":\"A\",\"args\":{\"value\":42.0}}"

private fun recoveryEntry(awaiting: ProtocolValue = str("A")) = obj(
    "kind" to str("command"), "table" to ProtocolValue.Null, "record_id" to ProtocolValue.Null, "key" to str("B"),
    "state" to str("queued"), "blocked_on" to ProtocolValue.ArrayValue(emptyList()), "awaiting_local" to ProtocolValue.BooleanValue(false),
    "awaiting_key" to awaiting, "rolled_back" to ProtocolValue.BooleanValue(false), "attempts" to str("0"), "next_due_ms" to ProtocolValue.Null)

private fun recoveryStatus() = obj(
    "pending" to str("1"), "sending" to str("0"), "awaiting_outcome" to str("0"), "live" to str("1"),
    "blocked" to ProtocolValue.BooleanValue(false), "commands" to ProtocolValue.ArrayValue(listOf(recoveryEntry())))

private class RecoveryRuntime(private val failure: String? = null) : NativeCalling {
    val frames = CopyOnWriteArrayList<Map<String, ProtocolValue>>()
    private val ready = AtomicBoolean(false)
    private val statusDelivered = AtomicBoolean(false)
    private fun ok(value: ProtocolValue) = WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to value))
    private fun error(kind: String) = WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(false), "error" to obj("kind" to str(kind))))
    override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = reply.onSuccess(ok(str("1")))
    override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
        val fields = WireCodec.decode(frame).fields()
        frames.add(fields)
        val value: ProtocolValue = when (fields.getValue("op").string()) {
            "queue_status" -> recoveryStatus()
            "observe_status" -> str("11")
            "drain" -> obj("events" to ProtocolValue.ArrayValue(
                if (statusDelivered.getAndSet(true)) emptyList()
                else listOf(obj("kind" to str("status"), "subscription" to str("11"), "value" to recoveryStatus()))),
                "has_more" to ProtocolValue.BooleanValue(false))
            "request_redelivery" -> {
                if (failure != null) return reply.onSuccess(error(failure))
                ready.set(true); ProtocolValue.Null
            }
            "prepare_push" -> obj("envelopes" to ProtocolValue.ArrayValue(if (ready.getAndSet(false)) listOf(str(FROZEN)) else emptyList()),
                "next_due_ms" to ProtocolValue.Null, "has_more" to ProtocolValue.BooleanValue(false))
            "receive_wire", "pending_reconciliations" -> ProtocolValue.ArrayValue(emptyList())
            "unsubscribe_sync" -> ProtocolValue.BooleanValue(true)
            else -> ProtocolValue.Null
        }
        reply.onSuccess(ok(value))
    }
    fun operations(name: String) = frames.filter { it.getValue("op").string() == name }
}

object RecoverySdkTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        suspend fun open(runtime: RecoveryRuntime): Database =
            Database(runtime, Executor { it.run() }, null, DatabaseOptions()).also { it.openMemory(notesSchema(), TEST_NAMESPACE) }

        test("SDK1_queue_fetch_exposes_the_row_causal_blocker") {
            val db = open(RecoveryRuntime())
            try { check(db.queueStatus().commands.single().awaitingKey == "A") } finally { db.close() }
        }
        test("SDK1_status_observation_uses_the_eleven_field_decoder") {
            val db = open(RecoveryRuntime())
            try {
                val seen = AtomicReference<String?>(); val delivered = CountDownLatch(1)
                // A rejected entry would retire the listener without a callback.
                val subscription = db.observeStatus { status -> seen.set(status.commands.single().awaitingKey); delivered.countDown() }
                check(delivered.await(10, TimeUnit.SECONDS)); check(seen.get() == "A")
                subscription.close()
            } finally { db.close() }
        }
        test("SDK2_explicit_redelivery_encodes_one_lossless_call_without_transport") {
            val runtime = RecoveryRuntime(); val db = open(runtime)
            try {
                db.requestRedelivery("A", 253402300799999L)
                val requests = runtime.operations("request_redelivery")
                check(requests.size == 1)
                check(requests.single().getValue("args") == obj("key" to str("A"), "now_ms" to str("253402300799999")))
                check(requests.single().getValue("owner") == str("1"))
                check(runtime.operations("prepare_push").isEmpty())
            } finally { db.close() }
        }
        test("SDK2_core_failures_are_returned_unchanged_without_retry") {
            for (kind in listOf("Protocol", "Busy", "InvalidCommand")) {
                val runtime = RecoveryRuntime(kind); val db = open(runtime)
                try {
                    try { db.requestRedelivery("A", Long.MIN_VALUE); error("rearm error was lost") }
                    catch (error: HoyaException) { check(error.kind == kind) }
                    val requests = runtime.operations("request_redelivery")
                    check(requests.size == 1 && requests.single().getValue("args") == obj("key" to str("A"), "now_ms" to str("-9223372036854775808")))
                    check(runtime.operations("prepare_push").isEmpty())
                } finally { db.close() }
            }
        }
        test("SDK2_successful_rearm_wakes_the_started_driver_which_sends_the_frozen_bytes_once") {
            val runtime = RecoveryRuntime(); val db = open(runtime)
            try {
                val sent = CopyOnWriteArrayList<String>(); val first = CountDownLatch(1)
                val driver = db.startSync(transport = { body -> sent.add(body); first.countDown(); "[]" }, now = { 0 }, jitter = { 0 })
                db.requestRedelivery("A", 0)
                check(first.await(10, TimeUnit.SECONDS))
                Thread.sleep(200)
                check(sent.toList() == listOf("[$FROZEN]"))
                driver.close()
            } finally { db.close() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
