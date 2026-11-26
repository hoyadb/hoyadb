// SPEC-0039 / TC-0043 GW15 and the GW16 managed-pass rules, scripted NativeCalling only.
// Mirrors GuardSDKTests.swift. Written while JVM/Gradle/Android execution is held: UNEXECUTED.
package dev.hoyadb

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executor

private class GuardRuntime(private val guardedError: String? = null, private val reconcileErrors: Map<String, String> = emptyMap(),
    pending: List<String> = emptyList(), private val pages: List<Boolean> = listOf(false)) : NativeCalling {
    val frames = CopyOnWriteArrayList<Map<String, ProtocolValue>>()
    private val open = CopyOnWriteArrayList(pending)
    @Volatile private var page = 0
    private fun ok(value: ProtocolValue) = WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to value))
    private fun failure(kind: String) = WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(false), "error" to obj("kind" to str(kind))))
    override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = reply.onSuccess(ok(str("1")))
    override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
        val fields = WireCodec.decode(frame).fields(); frames.add(fields)
        val value: ProtocolValue = when (fields.getValue("op").string()) {
            "reconcile_resolving" -> {
                if (guardedError != null) return reply.onSuccess(failure(guardedError))
                val key = fields.getValue("args").fields().getValue("key").string()
                reconcileErrors[key]?.let { return reply.onSuccess(failure(it)) }
                open.remove(key); obj("kind" to str("completed"))
            }
            "pending_reconciliations" -> ProtocolValue.ArrayValue(open.map { obj("key" to str(it), "outcome" to obj("status" to str("applied"), "result" to obj("kind" to str("null")))) })
            "prepare_push" -> { val more = pages[minOf(page, pages.size - 1)]; page++
                obj("envelopes" to ProtocolValue.ArrayValue(emptyList()), "next_due_ms" to ProtocolValue.Null, "has_more" to ProtocolValue.BooleanValue(more)) }
            else -> ProtocolValue.Null
        }
        reply.onSuccess(ok(value))
    }
    fun args(name: String) = frames.filter { it.getValue("op").string() == name }.map { it.getValue("args").fields() }
    fun firstPassReconciles(pagesInPass: Int): List<String> {
        var prepares = 0; val keys = ArrayList<String>()
        for (frame in frames) {
            val op = frame.getValue("op").string()
            if (op == "prepare_push" && ++prepares > pagesInPass) break
            if (op == "reconcile_resolving") keys.add(frame.getValue("args").fields().getValue("key").string())
        }
        return keys
    }
}

private val guardPlan = ReconcilePlan(listOf(PlanWrite.Put("notes", "s", mapOf("text" to Cell.StringValue("server")))))

object GuardSdkTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        suspend fun open(runtime: GuardRuntime) = Database(runtime, Executor { it.run() }, null, DatabaseOptions()).also { it.openMemory(notesSchema(), TEST_NAMESPACE) }
        fun awaitUntil(condition: () -> Boolean) { val end = System.nanoTime() + 2_000_000_000L; while (!condition() && System.nanoTime() < end) Thread.sleep(5) }

        test("GW15_one_call_carries_reconcile_plan_encoding_and_lossless_guards_in_order") {
            val runtime = GuardRuntime(); val db = open(runtime)
            try {
                val seen = mutableMapOf<String, Cell>("text" to Cell.StringValue("seen"), "note" to Cell.Null)
                val guards = listOf(CorrectionGuard("notes", "s", seen), CorrectionGuard("notes", "u", mapOf("text" to Cell.StringValue("omitted note"))), CorrectionGuard("notes", "t", null))
                seen["text"] = Cell.StringValue("mutated after construction") // the guard keeps its own copy
                db.reconcile("A", guardPlan); db.reconcileGuarded("A", guardPlan, guards)
                val sent = runtime.args("reconcile_resolving"); check(sent.size == 2)
                check(sent.last().getValue("key") == str("A"))
                check(sent.last().getValue("plan") == sent.first().getValue("plan"))
                check(sent.first().getValue("guards").items().isEmpty())
                val encoded = sent.last().getValue("guards").items(); check(encoded.map { it.fields().getValue("id") } == listOf(str("s"), str("u"), str("t")))
                check(encoded[0].fields().getValue("expected") == fieldsWire(mapOf("text" to Cell.StringValue("seen"), "note" to Cell.Null)))
                check("note" !in encoded[1].fields().getValue("expected").fields())
                check(encoded[2].fields().getValue("expected") === ProtocolValue.Null)
                check(encoded.all { it.fields().keys == setOf("table", "id", "expected") })
            } finally { db.close() }
        }
        test("GW15_core_errors_are_returned_unchanged_without_retry_or_fallback") {
            for (kind in listOf("RollbackConflict", "InvalidCommand", "Protocol", "Busy")) {
                val runtime = GuardRuntime(guardedError = kind); val db = open(runtime)
                try {
                    try { db.reconcileGuarded("A", guardPlan, emptyList()); error("$kind was lost") } catch (error: HoyaException) { check(error.kind == kind) }
                    check(runtime.args("reconcile_resolving").size == 1)
                } finally { db.close() }
            }
        }
        test("GW16_managed_pass_reports_RollbackConflict_and_continues_to_later_entries") {
            val runtime = GuardRuntime(reconcileErrors = mapOf("P1" to "RollbackConflict"), pending = listOf("P1", "P2")); val db = open(runtime)
            val errors = CopyOnWriteArrayList<String>()
            try {
                val driver = db.startSync(transport = { "[]" }, reconciler = { guardPlan }, now = { 10 }, jitter = { 0 }, onError = { errors.add(it.kind) })
                awaitUntil { runtime.args("reconcile_resolving").size >= 2 }; Thread.sleep(100)
                check(runtime.firstPassReconciles(1) == listOf("P1", "P2")); check(errors.firstOrNull() == "RollbackConflict")
                driver.close()
                check(db.pendingReconciliations().map { it.key } == listOf("P1"))
            } finally { db.close() }
        }
        test("GW16_conflicted_key_is_not_retried_on_later_pages_of_the_pass") {
            val runtime = GuardRuntime(reconcileErrors = mapOf("P1" to "RollbackConflict"), pending = listOf("P1", "P2"), pages = listOf(true, false)); val db = open(runtime)
            try {
                val driver = db.startSync(transport = { "[]" }, reconciler = { guardPlan }, now = { 10 }, jitter = { 0 })
                awaitUntil { runtime.args("prepare_push").size >= 2 && runtime.args("reconcile_resolving").size >= 2 }; Thread.sleep(100)
                check(runtime.firstPassReconciles(2) == listOf("P1", "P2"))
                driver.close()
            } finally { db.close() }
        }
        test("GW16_callback_failures_and_other_core_errors_keep_the_existing_pass_failure") {
            val throwing = GuardRuntime(pending = listOf("P1", "P2")); val first = open(throwing); val firstErrors = CopyOnWriteArrayList<String>()
            try {
                val driver = first.startSync(transport = { "[]" }, reconciler = { throw IllegalStateException("application mapper") }, now = { 10 }, jitter = { 0 }, onError = { firstErrors.add(it.kind) })
                awaitUntil { firstErrors.isNotEmpty() }; Thread.sleep(100)
                check(throwing.args("reconcile_resolving").isEmpty() && firstErrors.toList() == listOf("NativeFailure")); driver.close()
            } finally { first.close() }
            val core = GuardRuntime(reconcileErrors = mapOf("P1" to "Protocol"), pending = listOf("P1", "P2")); val second = open(core); val secondErrors = CopyOnWriteArrayList<String>()
            try {
                val driver = second.startSync(transport = { "[]" }, reconciler = { guardPlan }, now = { 10 }, jitter = { 0 }, onError = { secondErrors.add(it.kind) })
                awaitUntil { secondErrors.isNotEmpty() }; Thread.sleep(100)
                check(core.firstPassReconciles(1) == listOf("P1") && secondErrors.toList() == listOf("Protocol")); driver.close()
            } finally { second.close() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}

// TC-0043 GW16b (a379e8b): deterministic scripted no-self-loop controls. UNEXECUTED while held.
object GuardGw16bTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        suspend fun run(pending: List<String>): Triple<GuardRuntime, Int, Int> {
            val runtime = GuardRuntime(reconcileErrors = mapOf("X" to "RollbackConflict"), pending = pending)
            val db = Database(runtime, Executor { it.run() }, null, DatabaseOptions()).also { it.openMemory(notesSchema(), TEST_NAMESPACE) }
            val callbacks = java.util.concurrent.atomic.AtomicInteger(); val errors = java.util.concurrent.atomic.AtomicInteger()
            try {
                val driver = db.startSync(transport = { "[]" }, reconciler = { if (it.key == "X") callbacks.incrementAndGet(); guardPlan }, now = { 10 }, jitter = { 0 }, onError = { errors.incrementAndGet() })
                Thread.sleep(400); val prepares = runtime.args("prepare_push").size
                Thread.sleep(200); check(runtime.args("prepare_push").size == prepares) { "later activity" }
                driver.close()
                check(runtime.args("reconcile_resolving").count { it.getValue("key") == str("X") } == callbacks.get())
                check(errors.get() == callbacks.get())
                return Triple(runtime, callbacks.get(), prepares)
            } finally { db.close() }
        }
        test("GW16b_only_conflicting_X_is_tried_once_then_idle") {
            val (_, callbacks, prepares) = run(listOf("X")); check(callbacks == 1 && prepares == 1)
        }
        test("GW16b_conflicting_X_with_successful_Y_gets_exactly_one_further_pass") {
            val (runtime, callbacks, prepares) = run(listOf("X", "Y"))
            check(callbacks == 2 && prepares == 2)
            check(runtime.args("reconcile_resolving").count { it.getValue("key") == str("Y") } == 1)
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
