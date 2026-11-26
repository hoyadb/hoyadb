package dev.hoyadb

import java.util.concurrent.Executor

object EntityTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, run: suspend () -> Unit) { try { blocking(run); println("PASS $name") } catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        test("K11_entity_context_autoqueue_patch_pull_bootstrap_and_reset") {
            val db = Database(Executor { it.run() })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                var sampled = 0
                db.configureEntities(EntityConfig("synthetic scope", listOf("notes"))) { sampled++; CapturedContext(JSONValue.StringValue("synthetic identity"), 10L + sampled) }
                val changes = db.entityBatch(listOf(Write.Insert("notes", mapOf("text" to Cell.StringValue("initial")))))
                val id = changes.single().id; check(id.startsWith("entity:")); check(sampled == 1)
                check(db.patch("notes", id, mapOf("text" to Cell.StringValue("patched"))).fields.getValue("text") == Cell.StringValue("patched")); check(sampled == 2)
                val pending = db.execute("queue_status").fields().getValue("commands").items()
                check(pending.size == 2); check(pending.all { it.fields().getValue("kind").string() == "entity" })
                try { db.reset(); error("live reset accepted") } catch (error: HoyaException) { check(error.kind == "Protocol") }
                db.reset(ResetMode.DiscardRecovery); check(db.fetch(Query("notes")).isEmpty())
                db.configureEntities(EntityConfig("synthetic scope", listOf("notes"))) { CapturedContext(JSONValue.Null, 20) }
                val request = db.beginPull(); check(request.since == null)
                db.applyPull(request, PullPage("checkpoint-one", true, listOf(EntityChange("notes", "server-row", EntityOperation.Create, mapOf("text" to Cell.StringValue("remote")), timestampMs = 20, fieldTimestamps = mapOf("text" to 20)))))
                check(db.syncProgress().ready); check(db.fetch(Query("notes")).single().id == "server-row")
            } finally { db.close() }
        }
        test("K12_passive_hook_sequential_owned_parts_and_atomic_bad_identity") {
            val db = Database(Executor { it.run() })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                db.configureEntities(EntityConfig("hook scope", listOf("notes"))) { CapturedContext(JSONValue.Null, 1) }
                var calls = 0
                db.setMergeHooks(mapOf("notes" to { input ->
                    calls++; check(input.schema.tables.keys == setOf("notes")); check(input.incoming.recordId == "remote")
                    if (calls == 2) check(input.current!!.fields.getValue("text") == Cell.StringValue("first"))
                    input.defaultResolution.row
                }))
                val progress = db.pullOnce { since, scope ->
                    check(since == null && scope == "hook scope")
                    PullPage("hook-one", true, listOf(
                        EntityChange("notes", "remote", EntityOperation.Create, mapOf("text" to Cell.StringValue("first")), timestampMs = 10, fieldTimestamps = mapOf("text" to 10)),
                        EntityChange("notes", "remote", EntityOperation.Update, mapOf("text" to Cell.StringValue("second")), timestampMs = 11, fieldTimestamps = mapOf("text" to 11))))
                }
                check(progress.ready && calls == 2); check(db.fetch(Query("notes")).single().fields.getValue("text") == Cell.StringValue("second"))
                db.setMergeHooks(mapOf("notes" to { input -> Row("unrelated", input.defaultResolution.row!!.fields) }))
                try {
                    db.pullOnce { _, _ -> PullPage("bad", true, listOf(EntityChange("notes", "remote", EntityOperation.Update, mapOf("text" to Cell.StringValue("bad")), timestampMs = 12, fieldTimestamps = mapOf("text" to 12)))) }
                    error("bad hook accepted")
                } catch (error: HoyaException) { check(error.kind == "InvalidCommand") }
                check(db.beginPull().since == "hook-one")
            } finally { db.close() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
