package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.coroutines.*
import dev.hoyadb.testing.FakeTransport

object SyncDriverTests {
    // DEF-0055 / ADR-0037: same-owner Busy and independent-owner progress are
    // separate native contracts. The Busy reply is the observation barrier.
    private suspend fun verifyAdmission(independent: Boolean) {
        val busyReply = CountDownLatch(1); val busyReplies = AtomicInteger()
        val runtime = object : NativeCalling {
            override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = NativeRuntime.open(frame, key, reply)
            override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
                val operation = WireCodec.decode(frame).fields().getValue("op").string()
                NativeRuntime.call(frame, object : HoyaNativeTransport.Reply {
                    override fun onSuccess(bytes: ByteArray) {
                        if (operation == "receive_wire") {
                            val response = WireCodec.decode(bytes).fields()
                            if (response.getValue("ok").boolean() && response.getValue("value").items().any {
                                val entry = it.fields()
                                !entry.getValue("ok").boolean() && entry.getValue("error").fields().getValue("kind").string() == "Busy"
                            }) { busyReplies.incrementAndGet(); busyReply.countDown() }
                        }
                        reply.onSuccess(bytes)
                    }
                    override fun onFailure(kind: String) = reply.onFailure(kind)
                })
            }
        }
        val ui = Executors.newSingleThreadExecutor { Thread(it, "sync-selected-ui") }
        val db = Database(runtime, ui, null, DatabaseOptions())
        val blocker = if (independent) Database(Executor { it.run() }) else db
        val late = AtomicReference<Continuation<Unit>>()
        val handlerResumed = AtomicInteger()
        try {
            db.openMemory(notesSchema(), TEST_NAMESPACE)
            if (independent) blocker.openMemory(notesSchema(), "ffeeddccbbaa99887766554433221100")
            val command = db.command(CommandDefinition("sync-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 10)) { JSONValue.Null }
            val network = CountDownLatch(1); val response = AtomicReference<Continuation<String>>()
            val sends = AtomicInteger(); val maps = AtomicInteger(); val applied = CountDownLatch(1)
            val observer = db.observeOutcome(command.key) { if (it is CommandOutcome.Applied) applied.countDown() }
            val driver = db.startSync(transport = { body ->
                check(body.startsWith("[")); sends.incrementAndGet()
                suspendCoroutine<String> { response.set(it); network.countDown() }
            }, reconciler = { maps.incrementAndGet(); ReconcilePlan(emptyList()) }, now = { 0 }, jitter = { 0 })
            check(network.await(30, TimeUnit.SECONDS))
            val held = CountDownLatch(1)
            val blocking = blocker.startCommand(CommandDefinition("hold-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 11)) {
                suspendCoroutine<Unit> { late.set(it); held.countDown() }
                handlerResumed.incrementAndGet(); JSONValue.Null
            }
            check(held.await(30, TimeUnit.SECONDS))
            val wire = JSONValue.ArrayValue(listOf(JSONValue.ObjectValue(mapOf("idempotencyKey" to JSONValue.StringValue(command.key), "status" to JSONValue.StringValue("applied"), "result" to JSONValue.IntegerValue(Long.MAX_VALUE))))).encodeJSON()
            response.get().resume(wire)
            if (independent) {
                check(applied.await(30, TimeUnit.SECONDS))
                check(sends.get() == 1 && maps.get() == 1 && busyReplies.get() == 0)
                check(handlerResumed.get() == 0)
                blocking.cancel()
            } else {
                check(busyReply.await(30, TimeUnit.SECONDS))
                check(sends.get() == 1 && maps.get() == 0 && handlerResumed.get() == 0)
                blocking.cancel()
                check(applied.await(30, TimeUnit.SECONDS))
            }
            check(sends.get() == 1 && maps.get() == 1)
            driver.close(); observer.close()
        } finally {
            db.close(); if (independent) blocker.close()
            late.getAndSet(null)?.resume(Unit); ui.shutdownNow()
        }
    }
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        test("K09_packaged_fake_auto_correlations_mixed_i64_reorder_duplicate_and_drop") {
            val command = JSONValue.ObjectValue(mapOf("command" to JSONValue.StringValue("fake-v1"), "idempotencyKey" to JSONValue.StringValue("synthetic-key"), "args" to JSONValue.ObjectValue(mapOf("large" to JSONValue.IntegerValue(Long.MAX_VALUE)))))
            val entity = JSONValue.ObjectValue(mapOf("table" to JSONValue.StringValue("notes"), "recordId" to JSONValue.StringValue("remote"), "writeId" to JSONValue.StringValue("synthetic-write"), "changes" to JSONValue.ObjectValue(emptyMap())))
            val fake = FakeTransport(mapOf("fake-v1" to { envelope ->
                val fields = (envelope as JSONValue.ObjectValue).value
                check(fields.getValue("idempotencyKey") == JSONValue.StringValue("synthetic-key"))
                RemoteOutcome.Applied((fields.getValue("args") as JSONValue.ObjectValue).value.getValue("large"))
            }), entityHandler = { envelope ->
                check((envelope as JSONValue.ObjectValue).value.getValue("recordId") == JSONValue.StringValue("remote"))
                RemoteOutcome.Rejected("synthetic rejection")
            }, reorder = true, duplicate = true)
            val reply = JSONValue.decodeJSON(fake.push(JSONValue.ArrayValue(listOf(command, entity)).encodeJSON())) as JSONValue.ArrayValue
            check(reply.value.size == 4)
            val entityReply = reply.value[0] as JSONValue.ObjectValue
            check(entityReply.value.getValue("writeId") == JSONValue.StringValue("synthetic-write")); check(entityReply.value.getValue("recordId") == JSONValue.StringValue("remote"))
            val commandReply = reply.value[1] as JSONValue.ObjectValue
            check(commandReply.value.getValue("idempotencyKey") == JSONValue.StringValue("synthetic-key")); check(commandReply.value.getValue("result") == JSONValue.IntegerValue(Long.MAX_VALUE))
            try { FakeTransport(dropPush = { true }).push("[]"); error("drop did not reject") } catch (error: HoyaException) { check(error.kind == "NativeFailure") }
        }
        test("K10_sync_retains_actual_response_and_mapper_across_same_database_Busy") { verifyAdmission(false) }
        test("K10_independent_database_sync_progresses_while_other_handler_is_held") { verifyAdmission(true) }
        System.exit(if (failed == 0) 0 else 1)
    }
}
