package dev.hoyadb

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executor
import java.util.concurrent.atomic.AtomicInteger

// API-0011 §4 paged reads: the unpaged call stays the fast path. Controlled native replies prove
// the SDK loop only, never the core's ordering or bounds.
object PagedReadTests {
    private enum class Mode { Paged, Older, Small }
    private class Runtime(private val mode: Mode, private val vanish: Int = 0) : NativeCalling {
        val calls = CopyOnWriteArrayList<String>()
        private val protocols = AtomicInteger()
        private fun ok(value: ProtocolValue) = WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to value))
        private fun error(kind: String) = WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(false), "error" to obj("kind" to str(kind))))
        private fun entry(key: String) = obj("key" to str(key), "outcome" to obj("status" to str("rejected"), "reason" to str("no")))
        private fun command(key: String) = obj("kind" to str("command"), "table" to ProtocolValue.Null, "record_id" to ProtocolValue.Null, "key" to str(key),
            "state" to str("queued"), "blocked_on" to ProtocolValue.ArrayValue(emptyList()), "awaiting_local" to ProtocolValue.BooleanValue(false),
            "awaiting_key" to ProtocolValue.Null, "rolled_back" to ProtocolValue.BooleanValue(false), "attempts" to str("0"), "next_due_ms" to ProtocolValue.Null)
        private fun status(keys: List<String>, more: Boolean?): ProtocolValue {
            val fields = linkedMapOf<String, ProtocolValue>("pending" to str("3"), "sending" to str("0"), "awaiting_outcome" to str("0"), "live" to str("3"),
                "blocked" to ProtocolValue.BooleanValue(false), "commands" to ProtocolValue.ArrayValue(keys.map(::command)))
            if (more != null) fields["has_more"] = ProtocolValue.BooleanValue(more)
            return ProtocolValue.ObjectValue(fields)
        }
        override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = reply.onSuccess(ok(str("1")))
        override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
            val fields = WireCodec.decode(frame).fields()
            val op = fields.getValue("op").string(); val args = fields.getValue("args").fields()
            val after = args["after"]
            calls.add(if (after == null) op else op + ":" + ((after as? ProtocolValue.StringValue)?.value ?: "null"))
            reply.onSuccess(when (op) {
                "pending_reconciliations", "queue_status" -> if (mode != Mode.Small) error("LimitExceeded")
                    else ok(if (op == "queue_status") status(listOf("a"), null) else ProtocolValue.ArrayValue(listOf(entry("a"))))
                "pending_reconciliations_page", "queue_status_page" -> when {
                    mode == Mode.Older || args.keys != setOf("after") -> error("InvalidRequest")
                    after === ProtocolValue.Null -> page(op, listOf("a", "b"), true)
                    protocols.get() < vanish -> { protocols.incrementAndGet(); error("Protocol") }
                    after == str("b") -> page(op, listOf("c"), false)
                    else -> error("Protocol")
                }
                else -> ok(ProtocolValue.Null)
            })
        }
        private fun page(op: String, keys: List<String>, more: Boolean) = ok(if (op == "queue_status_page") status(keys, more)
            else obj("entries" to ProtocolValue.ArrayValue(keys.map(::entry)), "has_more" to ProtocolValue.BooleanValue(more)))
    }
    private suspend fun open(runtime: Runtime): Database =
        Database(runtime, Executor { it.run() }, null, DatabaseOptions(), { 0 }, SystemHoyaTimer).also { it.openMemory(notesSchema(), TEST_NAMESPACE) }

    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun test(name: String, body: suspend () -> Unit) {
            try { blocking(body); println("PASS $name") }
            catch (error: Throwable) {
                failures++
                println("FAIL $name " + (if (error is HoyaException) error.kind else error.javaClass.simpleName) + " line=" + error.stackTrace.firstOrNull { it.className.contains("PagedReadTests") }?.lineNumber)
            }
        }
        test("oversized_pending_reconciliations_follow_core_pages_by_key_cursor") {
            val runtime = Runtime(Mode.Paged); val db = open(runtime)
            try {
                check(db.pendingReconciliations().map { it.key } == listOf("a", "b", "c"))
                check(runtime.calls.toList() == listOf("pending_reconciliations", "pending_reconciliations_page:null", "pending_reconciliations_page:b"))
            } finally { db.close() }
        }
        test("oversized_queue_status_keeps_first_page_counters_and_all_entries") {
            val runtime = Runtime(Mode.Paged); val db = open(runtime)
            try {
                val status = db.queueStatus()
                check(status.commands.map { it.key } == listOf("a", "b", "c") && status.pending.value == "3")
                check(runtime.calls.toList() == listOf("queue_status", "queue_status_page:null", "queue_status_page:b"))
            } finally { db.close() }
        }
        test("unpaged_reply_that_fits_never_pages") {
            val runtime = Runtime(Mode.Small); val db = open(runtime)
            try {
                check(db.pendingReconciliations().single().key == "a" && db.queueStatus().commands.single().key == "a")
                check(runtime.calls.toList() == listOf("pending_reconciliations", "queue_status"))
            } finally { db.close() }
        }
        test("vanished_cursor_restarts_from_the_first_page_at_most_twice") {
            val runtime = Runtime(Mode.Paged, vanish = 2); val db = open(runtime)
            try {
                check(db.pendingReconciliations().map { it.key } == listOf("a", "b", "c"))
                check(runtime.calls.count { it == "pending_reconciliations_page:null" } == 3)
            } finally { db.close() }
            val stuck = open(Runtime(Mode.Paged, vanish = 3))
            try { stuck.pendingReconciliations(); error("third vanished cursor accepted") }
            catch (error: HoyaException) { check(error.kind == "Protocol") }
            finally { stuck.close() }
        }
        test("older_native_library_without_pages_surfaces_the_original_limit") {
            val db = open(Runtime(Mode.Older))
            try { db.queueStatus(); error("older runtime paged") }
            catch (error: HoyaException) { check(error.kind == "LimitExceeded") }
            finally { db.close() }
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
