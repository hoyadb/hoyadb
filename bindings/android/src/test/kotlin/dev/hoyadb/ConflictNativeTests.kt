package dev.hoyadb

import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executor

// SPEC-0040 / TC-0044: current JNI/core integration, separate from controlled host replies.
object ConflictNativeTests {
    private fun fields(text: String) = mapOf("text" to Cell.StringValue(text))
    private suspend fun create(db: Database, id: String): CommandResult = db.command(CommandDefinition("create-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 0)) {
        it.insert("notes", fields("optimistic"), id); JSONValue.Null
    }
    private suspend fun rejected(db: Database, command: CommandResult) {
        db.preparePush(0, 0); db.receive(command.key, RemoteOutcome.Rejected("synthetic rule")); db.reconcile(command.key, ReconcilePlan(emptyList()))
    }
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) {
            try { blocking(body); println("PASS $name") }
            catch (error: Throwable) { failed++; println("FAIL $name " + (if (error is HoyaException) error.kind else error.javaClass.simpleName) + " line=" + error.stackTrace.firstOrNull { it.className.contains("ConflictNativeTests") }?.lineNumber) }
        }
        test("CP06_native_manual_discard_preserves_rows_and_truthful_queue") {
            val callbacks = CopyOnWriteArrayList<String>()
            val db = Database(Executor { it.run() }, onResolution = { key, _ -> callbacks.add(key) })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val command = create(db, "local:kept"); val before = db.fetch(Query("notes")).map { it.toProtocolValue() }
                val result = db.discard(command.key) as Resolution.KeptCurrent
                check(result.discardedKeys == listOf(command.key) && result.unmapped == listOf("local:kept") && result.skippedWrites.value == "0")
                check(db.fetch(Query("notes")).map { it.toProtocolValue() } == before && db.preparePush(0, 0).envelopes.isEmpty())
                val entry = db.queueStatus().commands.single()
                check(entry.state == QueueState.Discarded && !entry.rolledBack && callbacks == listOf(command.key))
            } finally { db.close() }
        }
        test("CP04_native_default_stop_then_explicit_policy_keeps_direct_edit") {
            val callbacks = CopyOnWriteArrayList<String>()
            val db = Database(Executor { it.run() }, onResolution = { key, _ -> callbacks.add(key) })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val command = create(db, "local:one"); rejected(db, command); db.replace("notes", "local:one", fields("direct"))
                try { db.rollbackResolving(command.key); error("default Stop lost") } catch (e: HoyaException) { check(e.kind == "RollbackConflict") }
                check(callbacks.isEmpty())
                db.setConflictPolicy(ConflictPolicy(ConflictReaction.Discard))
                val result = db.rollbackResolving(command.key) as Resolution.KeptCurrent
                check(result.discardedKeys == listOf(command.key) && result.unmapped == listOf("local:one"))
                check(db.fetch(Query("notes")).single().fields == fields("direct") && callbacks == listOf(command.key))
            } finally { db.close() }
        }
        test("CP01_native_constructor_policy_reapplies_to_retained_store") {
            val store = MemoryStore.create(); val callbacks = CopyOnWriteArrayList<String>()
            val db = Database(Executor { it.run() }, conflict = ConflictPolicy(ConflictReaction.Discard), onResolution = { key, _ -> callbacks.add(key) })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE, store)
                val command = create(db, "local:persisted"); rejected(db, command); db.replace("notes", "local:persisted", fields("direct"))
                db.setConflictPolicy(ConflictPolicy()); db.close(); db.openMemory(notesSchema(), TEST_NAMESPACE, store)
                db.rollback(command.key)
                check(db.fetch(Query("notes")).single().fields == fields("direct") && callbacks == listOf(command.key))
            } finally { db.close(); store.drop() }
        }
        test("CP05_native_guard_validation_precedes_discard_and_exact_report") {
            val callbacks = CopyOnWriteArrayList<String>()
            val db = Database(Executor { it.run() }, conflict = ConflictPolicy(ConflictReaction.Discard), onResolution = { key, _ -> callbacks.add(key) })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val command = create(db, "local:one"); db.insert("notes", fields("outside"), "outside")
                db.preparePush(0, 0); db.receive(command.key, RemoteOutcome.Corrected(JSONValue.Null)); db.replace("notes", "local:one", fields("direct"))
                val plan = ReconcilePlan(listOf(PlanWrite.Put("notes", "remote", fields("server")), PlanWrite.Remove("notes", "outside")), mapOf("local:one" to "remote"))
                try { db.reconcileResolving(command.key, plan, listOf(CorrectionGuard("notes", "outside", fields("stale")))); error("stale guard accepted") }
                catch (e: HoyaException) { check(e.kind == "RollbackConflict") }
                check(callbacks.isEmpty())
                val result = db.reconcileResolving(command.key, plan, listOf(CorrectionGuard("notes", "outside", fields("outside")))) as Resolution.KeptCurrent
                check(result.discardedKeys.isEmpty() && result.unmapped.isEmpty() && result.skippedWrites.value == "2")
                val rows = db.fetch(Query("notes")); check(rows.single { it.id == "remote" }.fields == fields("direct") && rows.single { it.id == "outside" }.fields == fields("outside"))
                check(db.queueStatus().commands.single().state == QueueState.Corrected && callbacks == listOf(command.key))
            } finally { db.close() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
