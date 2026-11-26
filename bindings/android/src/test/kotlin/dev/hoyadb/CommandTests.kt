package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.Executor
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.coroutines.*

object CommandTests {
    // DEF-0055 / ADR-0037: observe actual core admission without changing replies.
    private class AdmissionProbe(private val name: String) : NativeCalling {
        val begins = AtomicInteger()
        val waiting = CountDownLatch(1)
        override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = NativeRuntime.open(frame, key, reply)
        override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
            val request = WireCodec.decode(frame).fields()
            val watched = request.getValue("op").string() == "begin" && request.getValue("args").fields().getValue("definition").fields().getValue("name").string() == name
            if (!watched) { NativeRuntime.call(frame, reply); return }
            begins.incrementAndGet()
            NativeRuntime.call(frame, object : HoyaNativeTransport.Reply {
                override fun onSuccess(bytes: ByteArray) {
                    val response = WireCodec.decode(bytes).fields()
                    val queued = response.getValue("ok").boolean() && !response.getValue("value").boolean()
                    reply.onSuccess(bytes)
                    if (queued) waiting.countDown()
                }
                override fun onFailure(kind: String) = reply.onFailure(kind)
            })
        }
    }
    private suspend fun verifyAdmission(independent: Boolean) {
        val probe = AdmissionProbe("note-second-v1"); val ids = AtomicInteger()
        val a = Database(probe, Executor { it.run() }, { "local:test-fixed-id-00" + ids.incrementAndGet() }, DatabaseOptions())
        val b = if (independent) Database(probe, Executor { it.run() }, { "local:test-fixed-id-002" }, DatabaseOptions()) else a
        val resume = AtomicReference<Continuation<Unit>>()
        try {
            a.openMemory(notesSchema(), TEST_NAMESPACE)
            if (independent) b.openMemory(notesSchema(), "ffeeddccbbaa99887766554433221100")
            val started = CountDownLatch(1); val resumed = AtomicInteger()
            val mutable = mutableMapOf<String, JSONValue>("text" to JSONValue.StringValue("captured"))
            val argument = JSONValue.ObjectValue(mutable)
            val first = a.startCommand(CommandDefinition("note-create-v1", mapOf("text" to CommandDefinition.Argument(CommandDefinition.Kind.String))), argument, CapturedContext(JSONValue.StringValue("synthetic identity"), 123)) { scope ->
                check(Thread.currentThread().name != "main")
                val context = scope.context(); check(context.nowMilliseconds == 123L && context.identity == JSONValue.StringValue("synthetic identity"))
                val captured = scope.arguments() as JSONValue.ObjectValue; check(captured.value.getValue("text") == JSONValue.StringValue("captured"))
                check(scope.insert("notes", mapOf("text" to Cell.StringValue("first"))).id == "local:test-fixed-id-001")
                suspendCoroutine<Unit> { resume.set(it); started.countDown() }
                resumed.incrementAndGet(); JSONValue.IntegerValue(Long.MAX_VALUE)
            }
            mutable.clear(); check(started.await(30, TimeUnit.SECONDS)); check(a.fetch(Query("notes")).isEmpty())
            val secondStarted = CountDownLatch(1)
            val second = b.startCommand(CommandDefinition("note-second-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 124)) { scope ->
                secondStarted.countDown(); scope.insert("notes", mapOf("text" to Cell.StringValue("second"))); JSONValue.Null
            }
            if (independent) {
                second.await(); check(secondStarted.count == 0L && resumed.get() == 0 && probe.begins.get() == 1)
                check(a.fetch(Query("notes")).isEmpty())
            } else {
                check(probe.waiting.await(30, TimeUnit.SECONDS))
                check(secondStarted.count == 1L && resumed.get() == 0 && probe.begins.get() == 1)
            }
            resume.getAndSet(null).resume(Unit)
            check(first.await().value == JSONValue.IntegerValue(Long.MAX_VALUE)); second.await()
            if (independent) {
                check(a.fetch(Query("notes")).single().id == "local:test-fixed-id-001")
                check(b.fetch(Query("notes")).single().id == "local:test-fixed-id-002")
            } else {
                check(a.fetch(Query("notes")).map { it.id }.toSet() == setOf("local:test-fixed-id-001", "local:test-fixed-id-002"))
                check(probe.begins.get() == 2)
            }
        } finally { a.close(); if (independent) b.close(); resume.getAndSet(null)?.resume(Unit) }
    }
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, run: suspend () -> Unit) { try { blocking(run); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        test("K07_real_suspended_same_owner_FIFO_and_immutable_captured_context") { verifyAdmission(false) }
        test("K07_independent_owner_progresses_while_other_handler_is_suspended") { verifyAdmission(true) }
        test("K08_noncooperative_cancel_releases_same_owner_and_late_resume_cannot_commit") {
            val db = Database(Executor { it.run() }, { "local:cancelled" }); val resume = AtomicReference<Continuation<Unit>>()
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val started = CountDownLatch(1); val resumed = CountDownLatch(1)
                val first = db.startCommand(CommandDefinition("note-cancel-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 1)) { scope ->
                    scope.insert("notes", mapOf("text" to Cell.StringValue("aborted")))
                    suspendCoroutine<Unit> { resume.set(it); started.countDown() }; resumed.countDown(); JSONValue.Null
                }
                check(started.await(30, TimeUnit.SECONDS)); first.cancel()
                try { first.await(); error("cancelled task succeeded") } catch (error: HoyaException) { check(error.kind == "Closed") }
                db.command(CommandDefinition("note-after-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 2)) { it.insert("notes", mapOf("text" to Cell.StringValue("survived")), "local:survivor"); JSONValue.Null }
                resume.getAndSet(null).resume(Unit); check(resumed.await(30, TimeUnit.SECONDS))
                check(db.fetch(Query("notes")).single().id == "local:survivor")
            } finally { db.close(); resume.getAndSet(null)?.resume(Unit) }
        }
        test("K08_raw_RN_style_terminal_wakes_same_typed_owner_without_busy_spin") {
            val probe = AdmissionProbe("typed-after-raw-v1")
            val db = Database(probe, Executor { it.run() }, { "local:raw-wake" }, DatabaseOptions())
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val request = db.execute("request").unsigned().value
                check(db.execute("begin", mapOf("request" to str(request), "definition" to CommandDefinition("raw-owner-v1").wire(), "value" to JSONValue.ObjectValue(emptyMap()).toProtocolValue(), "context" to CapturedContext(JSONValue.Null, 10).wire())).boolean())
                val handlers = AtomicInteger()
                val task = db.startCommand(CommandDefinition("typed-after-raw-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 11)) { scope -> handlers.incrementAndGet(); scope.insert("notes", mapOf("text" to Cell.StringValue("awake"))); JSONValue.Null }
                check(probe.waiting.await(30, TimeUnit.SECONDS))
                // A real read crosses the native channel while the command remains queued.
                check(db.fetch(Query("notes")).isEmpty()); check(probe.begins.get() == 1 && handlers.get() == 0)
                db.execute("abort", mapOf("request" to str(request)))
                task.await(); check(probe.begins.get() == 2 && handlers.get() == 1); check(db.fetch(Query("notes")).single().id == "local:raw-wake")
            } finally { db.close() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
