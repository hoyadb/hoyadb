// Independent local-Maven AAR consumer: no SDK implementation/source imports.
package dev.hoyadb.consumer

import android.app.Activity
import android.content.ContextWrapper
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.system.Os
import android.system.OsConstants
import dev.hoyadb.*
import dev.hoyadb.testing.FakeTransport
import dev.hoyadb.testing.PreloadedRuntime
import dev.hoyadb.testing.openMemoryDatabase
import dev.hoyadb.testing.preloadRuntime
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.coroutines.*
import org.json.JSONArray
import org.json.JSONObject

private const val NS = "00112233445566778899aabbccddeeff"
private const val CANARY = "hoyadb-owned-kotlin-private-canary-81974cfd"
private fun key() = ByteArray(32) { (it + 1).toByte() }
private fun articlesSchema() = Schema(1, mapOf(
    "authors" to Table(mapOf("name" to Column(ColumnKind.String))),
    "articles" to Table(mapOf("title" to Column(ColumnKind.String), "score" to Column(ColumnKind.Integer), "ratio" to Column(ColumnKind.Float), "active" to Column(ColumnKind.Boolean), "at" to Column(ColumnKind.Timestamp), "note" to Column(ColumnKind.String, true)),
        mapOf("author" to Relation.ToOne("authors", "author_id"), "tags" to Relation.ManyToMany("tags", "article_tags", "article", "tag"))),
    "tags" to Table(mapOf("label" to Column(ColumnKind.String))),
    "article_tags" to Table(emptyMap(), mapOf("article" to Relation.ToOne("articles", "article_id"), "tag" to Relation.ToOne("tags", "tag_id")))))
private fun sensorSchema() = Schema(1, mapOf(
    "sensors" to Table(mapOf("label" to Column(ColumnKind.String)), mapOf("readings" to Relation.HasMany("readings", "sensor_id"))),
    "readings" to Table(mapOf("value" to Column(ColumnKind.Float), "at" to Column(ColumnKind.Timestamp)), mapOf("sensor" to Relation.ToOne("sensors", "sensor_id")))))
private fun articleFields(title: String = CANARY, at: Long = Long.MIN_VALUE) = mapOf(
    "title" to Cell.StringValue(title), "score" to Cell.IntegerValue(Long.MAX_VALUE), "ratio" to Cell.FloatValue(42.0),
    "active" to Cell.BooleanValue(true), "at" to Cell.TimestampValue(at), "author_id" to Cell.ReferenceValue("author"))
private fun schemaNotes(version: Long = 1) = Schema(version, mapOf("notes" to Table(mapOf("text" to Column(ColumnKind.String)))))

private class OnExecutor(private val executor: Executor) : AbstractCoroutineContextElement(ContinuationInterceptor), ContinuationInterceptor {
    override fun <T> interceptContinuation(continuation: Continuation<T>) = object : Continuation<T> {
        override val context = continuation.context
        override fun resumeWith(result: Result<T>) { executor.execute { continuation.resumeWith(result) } }
    }
}
private fun await(latch: CountDownLatch) { check(latch.await(30, TimeUnit.SECONDS)) }
private fun hash(file: File) = MessageDigest.getInstance("SHA-256").digest(file.readBytes()).joinToString("") { "%02x".format(it.toInt() and 255) }
private fun hasBytes(haystack: ByteArray, needle: ByteArray): Boolean = haystack.indices.any { offset -> offset + needle.size <= haystack.size && needle.indices.all { haystack[offset + it] == needle[it] } }

class MainActivity : Activity() {
    private val tests = Executors.newCachedThreadPool { Thread(it, "consumer-check") }
    private val ui = Executor { work -> Handler(Looper.getMainLooper()).post(work) }
    private val report = JSONArray()
    private var failures = 0
    private var heldDatabase: Database? = null
    private var heldTask: CommandTask? = null
    private var memoryRuntime: PreloadedRuntime? = null
    @Volatile private var step = 0
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val mode = intent.getStringExtra("mode") ?: "full"
        val body: suspend () -> Unit = {
            try {
                when (mode) { "full" -> full(); "seed" -> seed(); "restart" -> restart(); else -> error("unknown fixture mode") }
            } catch (error: Throwable) { record("harness", false, error) }
            val output = JSONObject().put("mode", mode).put("failed", failures).put("cases", report)
                .put("page_size", Os.sysconf(OsConstants._SC_PAGESIZE)).put("pid", android.os.Process.myPid())
            File(noBackupFilesDir, "results-$mode.json").writeText(output.toString())
        }
        body.startCoroutine(object : Continuation<Unit> {
            override val context = OnExecutor(tests)
            override fun resumeWith(result: Result<Unit>) { /* full harness records its static result above */ }
        })
    }
    private fun record(name: String, pass: Boolean, error: Throwable? = null) {
        if (!pass) failures++
        val value = JSONObject().put("id", name).put("ok", pass)
        if (error != null) {
            value.put("step", step)
            value.put("error", if (error is HoyaException) error.kind else error.javaClass.simpleName)
            // Static throw site (class:line) only; never a message, path or payload.
            error.stackTrace.firstOrNull { it.className.startsWith("dev.hoyadb.") && !it.className.startsWith("dev.hoyadb.consumer") }
                ?.let { value.put("at", it.className + ":" + it.lineNumber) }
            error.stackTrace.firstOrNull { it.className.startsWith("dev.hoyadb.consumer") }
                ?.let { value.put("fixture_at", it.lineNumber) }
        }
        report.put(value)
    }
    private suspend fun case(name: String, body: suspend () -> Unit) {
        step = 0
        try { body(); record(name, true) } catch (error: Throwable) { record(name, false, error) }
    }
    private suspend fun database(name: String, schema: Schema, persistent: Boolean, id: (() -> String?)? = null): Database {
        if (!persistent) return openMemoryDatabase(checkNotNull(memoryRuntime), schema, NS, ui, idGenerator = id)
        val db = Database(ui, id)
        db.openProtected(this, "$name.sqlite", key(), schema, NS)
        return db
    }
    private suspend fun full() {
        case("K14.actual_16KiB_native_abi") {
            check(Os.sysconf(OsConstants._SC_PAGESIZE) == 16384L)
            check(HoyaNativeTransport.abiVersion() == 1)
            memoryRuntime = preloadRuntime()
        }
        for (persistent in listOf(false, true)) {
            val suffix = if (persistent) "protected" else "memory"
            case("K01.$suffix.literal_schema_A_all_values_relations_queries_batch") {
                val db = database("schema-a-$suffix", articlesSchema(), persistent)
                try {
                    db.insert("authors", mapOf("name" to Cell.StringValue("東京 author")), "author")
                    db.insert("tags", mapOf("label" to Cell.StringValue("offline")), "tag")
                    val row = db.insert("articles", articleFields(), "article")
                    check(row.fields.getValue("ratio") is Cell.FloatValue); check(row.fields.getValue("at") == Cell.TimestampValue(Long.MIN_VALUE))
                    check(!row.fields.containsKey("note")); check(db.schema().tables.getValue("articles").columns.getValue("author_id").kind == ColumnKind.Reference)
                    db.link("articles", "article", "tags", "tag", proposedId = "managed-link")
                    check(db.fetch(Query("tags", source = Source("articles", "article", "tags"))).single().id == "tag")
                    check(db.fetch(Query("authors", source = Source("articles", "article", "author"))).single().id == "author")
                    for (operator in Operator.values()) {
                        val value = when (operator) { Operator.Ne, Operator.Lt -> Long.MAX_VALUE - 1; else -> Long.MAX_VALUE }
                        val rows = db.fetch(Query("articles", listOf(Predicate("score", operator, listOf(Cell.IntegerValue(value)))), Order("author.name", Direction.Desc), 1))
                        val expected = when (operator) { Operator.Lt, Operator.Gt -> 0; else -> 1 }
                        check(rows.size == expected)
                    }
                    val changes = db.batch(listOf(Write.Replace("articles", "article", articleFields("replacement") + ("note" to Cell.Null))))
                    check(changes.single().before!!.fields.getValue("title") == Cell.StringValue(CANARY))
                    check(changes.single().after!!.fields.getValue("note") == Cell.Null)
                    check(db.unlink("articles", "article", "tags", "tag").value == "1")
                    check(db.fetch(Query("tags", source = Source("articles", "article", "tags"))).isEmpty())
                } finally { db.close() }
            }
            case("K01.$suffix.literal_schema_B_HasMany") {
                val db = database("schema-b-$suffix", sensorSchema(), persistent)
                try {
                    db.batch(listOf(Write.Insert("sensors", mapOf("label" to Cell.StringValue("sensor")), "sensor"), Write.Insert("readings", mapOf("value" to Cell.FloatValue(1.25), "at" to Cell.TimestampValue(Long.MAX_VALUE), "sensor_id" to Cell.ReferenceValue("sensor")), "reading")))
                    val row = db.fetch(Query("readings", source = Source("sensors", "sensor", "readings"))).single()
                    check(row.fields.getValue("value") == Cell.FloatValue(1.25)); check(row.fields.getValue("at") == Cell.TimestampValue(Long.MAX_VALUE))
                } finally { db.close() }
            }
            case("K06.$suffix.initial_UI_executor_reentrant_drain_cancel") {
                val db = database("observe-$suffix", schemaNotes(), persistent, { "one" })
                try {
                    val initial = CountDownLatch(1); val changed = CountDownLatch(1); val callbackError = AtomicReference<Throwable>()
                    var calls = 0; var subscription: Subscription? = null
                    subscription = db.observe(Query("notes")) { value ->
                        try {
                            check(Looper.myLooper() == Looper.getMainLooper()); check(value is ObservedValue.Rows); calls++
                            val action: suspend () -> Unit = {
                                try { db.execute("drain", mapOf("now_ms" to ProtocolValue.StringValue("0"))); error("reentrant drain") }
                                catch (failure: HoyaException) { check(failure.kind == "Busy") }
                            }
                            action.startCoroutine(object : Continuation<Unit> { override val context = EmptyCoroutineContext; override fun resumeWith(result: Result<Unit>) { result.exceptionOrNull()?.let(callbackError::set) } })
                            if (calls == 1) initial.countDown() else { subscription!!.close(); changed.countDown() }
                        } catch (failure: Throwable) { callbackError.set(failure); initial.countDown(); changed.countDown() }
                    }
                    await(initial); callbackError.get()?.let { throw it }
                    db.insert("notes", mapOf("text" to Cell.StringValue("changed")))
                    await(changed); callbackError.get()?.let { throw it }; subscription.close(); check(calls == 2)
                } finally { db.close() }
            }
            case("K07.$suffix.awaited_command_context_injected_ID") {
                val db = database("command-$suffix", schemaNotes(), persistent, { "local:test-fixed-id-001" })
                try {
                    val started = CountDownLatch(1); val suspended = AtomicReference<Continuation<Unit>>()
                    val task = db.startCommand(CommandDefinition("create-note-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.StringValue("synthetic identity"), 123)) { scope ->
                        check(scope.context().nowMilliseconds == 123L); scope.insert("notes", mapOf("text" to Cell.StringValue(CANARY)))
                        suspendCoroutine<Unit> { suspended.set(it); started.countDown() }; JSONValue.IntegerValue(Long.MAX_VALUE)
                    }
                    await(started); check(db.fetch(Query("notes")).isEmpty()); suspended.get().resume(Unit)
                    check(task.await().value == JSONValue.IntegerValue(Long.MAX_VALUE)); check(db.fetch(Query("notes")).single().id == "local:test-fixed-id-001")
                    check(db.queueStatus().commands.single().state == QueueState.Queued)
                } finally { db.close() }
            }
            case("K08.$suffix.noncooperative_cancel_late_resume_releases_scope") {
                val db = database("cancel-$suffix", schemaNotes(), persistent, { "local:test-fixed-id-001" })
                try {
                    val started = CountDownLatch(1); val late = AtomicReference<Continuation<Unit>>()
                    val task = db.startCommand(CommandDefinition("cancel-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 1)) { scope ->
                        scope.insert("notes", mapOf("text" to Cell.StringValue("discard")))
                        suspendCoroutine<Unit> { late.set(it); started.countDown() }; JSONValue.Null
                    }
                    await(started); task.cancel()
                    try { task.await(); error("cancelled result") } catch (error: HoyaException) { check(error.kind == "Closed") }
                    db.command(CommandDefinition("after-cancel-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 2)) { scope -> scope.insert("notes", mapOf("text" to Cell.StringValue("retained"))); JSONValue.Null }
                    late.get().resume(Unit)
                    check(db.fetch(Query("notes")).single().fields.getValue("text") == Cell.StringValue("retained"))
                } finally { db.close() }
            }
            case("K09.$suffix.packaged_fake_exact_wire_and_i64") {
                val db = database("fake-$suffix", schemaNotes(), persistent)
                try {
                    // Core rejects initial entity configuration (Protocol) once live command
                    // recovery exists (entity_owner.rs), so configure before commands.
                    step = 3
                    db.configureEntities(EntityConfig("fake scope", listOf("notes"))) { CapturedContext(JSONValue.Null, 20) }
                    step = 1
                    val command = db.command(CommandDefinition("echo-v1", mapOf("value" to CommandDefinition.Argument(CommandDefinition.Kind.Integer))), JSONValue.ObjectValue(mapOf("value" to JSONValue.IntegerValue(Long.MAX_VALUE))), CapturedContext(JSONValue.Null, 1)) { JSONValue.Null }
                    step = 2
                    val rejected = db.command(CommandDefinition("reject-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 2)) { JSONValue.Null }
                    step = 4
                    val entityId = db.entityBatch(listOf(Write.Insert("notes", mapOf("text" to Cell.StringValue("optimistic"))))).single().id
                    step = 5
                    val fake = FakeTransport(mapOf(
                        "echo-v1" to { envelope -> RemoteOutcome.Applied(((envelope as JSONValue.ObjectValue).value.getValue("args") as JSONValue.ObjectValue).value.getValue("value")) },
                        "reject-v1" to { RemoteOutcome.Rejected("synthetic rejection") }),
                        entityHandler = { RemoteOutcome.Corrected(JSONValue.Null) }, reorder = true, duplicate = true)
                    step = 6
                    val batch = db.preparePush(100, 0); check(batch.envelopes.size == 3)
                    step = 7
                    val response = fake.push("[" + batch.envelopes.joinToString(",") + "]")
                    step = 8
                    check((JSONValue.decodeJSON(response) as JSONValue.ArrayValue).value.size == 6)
                    step = 9
                    db.receiveWire(response)
                    step = 10
                    check(db.pendingReconciliations().size == 3)
                    step = 11
                    db.reconcile(command.key, ReconcilePlan(emptyList()))
                    step = 12
                    // Core inverse requires Rejected && reconciled (tests/conformance/command_sync.rs).
                    db.reconcile(rejected.key, ReconcilePlan(emptyList())); db.rollback(rejected.key)
                    step = 13
                    val entityKey = db.queueStatus().commands.single { it.kind == QueueKind.Entity }.key
                    step = 14
                    db.reconcile(entityKey, ReconcilePlan(listOf(PlanWrite.Entity(EntityChange("notes", entityId,
                        EntityOperation.Update, mapOf("text" to Cell.StringValue("authoritative")), timestampMs = 25,
                        fieldTimestamps = mapOf("text" to 25)))), mapOf(entityId to entityId)))
                    step = 15
                    val states = db.queueStatus().commands.associate { it.key to it.state }
                    step = 16
                    check(states[command.key] == QueueState.Applied && states[rejected.key] == QueueState.Rejected && states[entityKey] == QueueState.Corrected)
                    step = 17
                    check(db.fetch(Query("notes")).single().fields.getValue("text") == Cell.StringValue("authoritative"))
                    step = 18
                    val pull = db.beginPull()
                    step = 19
                    db.applyPull(pull, PullPage("fake-clock-24", true, listOf(EntityChange("notes", entityId,
                        EntityOperation.Update, mapOf("text" to Cell.StringValue("older")), timestampMs = 24,
                        fieldTimestamps = mapOf("text" to 24)))))
                    step = 20
                    check(db.fetch(Query("notes")).single().fields.getValue("text") == Cell.StringValue("authoritative"))
                    step = 21
                    check(db.diagnostics().live.value == "0")
                    step = 22
                    val dropped = db.command(CommandDefinition("dropped-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 30)) { JSONValue.Null }
                    step = 23
                    val first = db.preparePush(200, 0)
                    step = 24
                    try { FakeTransport(dropPush = { true }).push("[" + first.envelopes.joinToString(",") + "]"); error("ACK drop returned") }
                    catch (error: HoyaException) { check(error.kind == "NativeFailure") }
                    step = 25
                    check(db.preparePush(1199, 0).envelopes.isEmpty())
                    step = 26
                    val retry = db.preparePush(1200, 0)
                    step = 27
                    check(retry.envelopes == first.envelopes && retry.envelopes.size == 1)
                    step = 28
                    val sent = db.queueStatus().commands.single { it.key == dropped.key }
                    step = 29
                    check(sent.state == QueueState.Sent && sent.attempts == 2L && sent.nextDueMs == 3200L)
                } finally { db.close() }
            }
        }
        retainedMigration()
        protection()
        passiveMerge()
        localOffline()
        privacyAndBounds()
        ownedPrefixes()
        lateDriverResponse()
        conflictPolicy()
    }
    private suspend fun retainedMigration() = case("K04.retained_memory_all_seven_authored_steps") {
        val store = MemoryStore.create(); val db = Database(ui)
        try {
            db.openMemory(sensorSchema(), NS, store); db.insert("sensors", mapOf("label" to Cell.StringValue("saved")), "sensor"); db.close()
            val target = Schema(2, mapOf("sensors" to Table(mapOf("name" to Column(ColumnKind.String)), mapOf("readings" to Relation.HasMany("readings", "sensor_id"))), "readings" to sensorSchema().tables.getValue("readings")))
            val steps = listOf(MigrationStep.AddTable("scratch", Table(emptyMap())), MigrationStep.DropTable("scratch"), MigrationStep.AddColumn("sensors", "flag", Column(ColumnKind.Boolean), ColumnDefault.Some(Cell.BooleanValue(true))),
                MigrationStep.Backfill("sensors", "flag", Cell.BooleanValue(false)), MigrationStep.RenameColumn("sensors", "label", "name"), MigrationStep.DropColumn("sensors", "flag"), MigrationStep.SetRelations("sensors", mapOf("readings" to Relation.HasMany("readings", "sensor_id"))))
            db.openMemory(target, NS, store, listOf(Migration(1, target, steps))); check(db.fetch(Query("sensors")).single().fields.getValue("name") == Cell.StringValue("saved"))
        } finally { db.close(); store.drop() }
    }
    private suspend fun protection() = case("K13.actual_noBackup_optin_key_cipher_foreign_corruption_preservation") {
        val db = Database(ui); val supplied = key(); val original = supplied.copyOf()
        try {
            val spy = object : ContextWrapper(this) {
                override fun getNoBackupFilesDir(): File = error("unexpected path getter")
                override fun getFilesDir(): File = error("unexpected path getter")
            }
            try { db.openProtected(spy, "bad.sqlite", ByteArray(31), schemaNotes(), NS); error("bad key accepted") } catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
            db.openProtected(this, "secure-canary.sqlite", supplied, schemaNotes(), NS)
            db.insert("notes", mapOf("text" to Cell.StringValue(CANARY)), "one")
            check(db.protection().encrypted && db.protection().backup == "host_managed"); db.close(); check(supplied.contentEquals(original))
            val file = File(noBackupFilesDir, "hoyadb/secure-canary.sqlite"); check(file.exists() && file.canonicalFile.parentFile == File(noBackupFilesDir, "hoyadb").canonicalFile)
            check(!hasBytes(file.readBytes(), CANARY.toByteArray(Charsets.UTF_8)))
            check(!String(file.readBytes().take(16).toByteArray(), Charsets.US_ASCII).startsWith("SQLite format 3"))
            val before = hash(file); val wrong = supplied.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }
            try { db.openProtected(this, "secure-canary.sqlite", wrong, schemaNotes(), NS); error("wrong key") } catch (error: HoyaException) { check(error.kind == "BackendFailure") }
            check(hash(file) == before)
            val corrupt = File(file.parentFile, "corrupt.sqlite"); corrupt.writeBytes(file.readBytes().also { it[100] = (it[100].toInt() xor 1).toByte() }); val corruptedHash = hash(corrupt)
            try { db.openProtected(this, "corrupt.sqlite", supplied, schemaNotes(), NS); error("corrupt page") } catch (error: HoyaException) { check(error.kind == "BackendFailure") }
            check(hash(corrupt) == corruptedHash)
            db.openProtected(this, "opt-in.sqlite", supplied, schemaNotes(), NS, backupExcluded = false); db.close()
            check(File(filesDir, "hoyadb/opt-in.sqlite").exists() && !File(noBackupFilesDir, "hoyadb/opt-in.sqlite").exists())
            val foreign = File(file.parentFile, "foreign.sqlite")
            android.database.sqlite.SQLiteDatabase.openOrCreateDatabase(foreign, null).use { it.execSQL("CREATE TABLE fixture(value TEXT)") }
            val foreignHash = hash(foreign)
            try { db.openProtected(this, "foreign.sqlite", supplied, schemaNotes(), NS); error("foreign plaintext") } catch (error: HoyaException) { check(error.kind == "BackendFailure") }
            check(hash(foreign) == foreignHash)
        } finally { db.close(); supplied.fill(0); original.fill(0) }
    }
    private suspend fun passiveMerge() = case("K11_K12.actual_protected_passive_hooks_pull_reset") {
        val db = database("passive", schemaNotes(), true)
        try {
            db.configureEntities(EntityConfig("consumer scope", listOf("notes"))) { CapturedContext(JSONValue.Null, 10) }
            var calls = 0
            db.setMergeHooks(mapOf("notes" to { input -> calls++; check(input.incoming.recordId == "remote"); input.defaultResolution.row }))
            check(db.pullOnce { _, scope -> check(scope == "consumer scope"); PullPage("one", true, listOf(EntityChange("notes", "remote", EntityOperation.Create, mapOf("text" to Cell.StringValue("remote")), timestampMs = 10, fieldTimestamps = mapOf("text" to 10)))) }.ready)
            check(calls == 1); db.patch("notes", "remote", mapOf("text" to Cell.StringValue("optimistic")))
            check(db.queueStatus().commands.single().kind == QueueKind.Entity)
            try { db.reset(); error("live recovery reset") } catch (error: HoyaException) { check(error.kind == "Protocol") }
            db.reset(ResetMode.DiscardRecovery); check(db.fetch(Query("notes")).isEmpty())
        } finally { db.close() }
    }
    private suspend fun localOffline() = case("K10_K22.all_local_operations_without_successful_transport") {
        val db = database("offline", schemaNotes(), true, { "local:offline" })
        val attempted = CountDownLatch(1)
        try {
            val driver = db.startSync(transport = { attempted.countDown(); throw java.io.IOException("synthetic offline") }, now = { 0 }, jitter = { 0 })
            db.command(CommandDefinition("offline-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 1)) { scope -> scope.insert("notes", mapOf("text" to Cell.StringValue("offline"))); JSONValue.Null }
            check(db.fetch(Query("notes")).single().id == "local:offline"); check(db.queueStatus().live.value == "1")
            await(attempted); check(db.queueStatus().commands.single().state == QueueState.Sent); driver.close()
        } finally { db.close() }
    }
    private suspend fun privacyAndBounds() = case("K02_K24.public_opaque_formatting_and_static_boundaries") {
        check(!Source("notes", CANARY, "relation").toString().contains(CANARY)); check(!Row(CANARY, mapOf("text" to Cell.StringValue(CANARY))).toString().contains(CANARY))
        check(UnsignedDecimal.parse("18446744073709551615").value == "18446744073709551615")
        val value = JSONValue.decodeJSON("9223372036854775807"); check(value == JSONValue.IntegerValue(Long.MAX_VALUE))
        val db = database("bounds", schemaNotes(), false)
        try {
            try { db.insert("notes", mapOf("text" to Cell.StringValue("x".repeat(1_048_577))), "oversized"); error("bound accepted") } catch (error: HoyaException) { check(error.kind == "LimitExceeded") }
            check(db.fetch(Query("notes")).isEmpty())
        } finally { db.close() }
    }
    private suspend fun ownedPrefixes() {
        case("K02_K11.large_owned_event_prefixes") {
            val db = database("event-prefix", schemaNotes(), false)
            try {
                db.batch((0 until 120).map { Write.Insert("notes", mapOf("text" to Cell.StringValue("x".repeat(4500))), "n" + it.toString().padStart(3, '0')) })
                val args = mapOf("query" to Query("notes").toProtocolValue(), "mode" to ProtocolValue.StringValue("membership"), "columns" to ProtocolValue.ArrayValue(emptyList()))
                val first = db.execute("observe", args) as ProtocolValue.StringValue
                val second = db.execute("observe", args) as ProtocolValue.StringValue
                val subscriptions = ArrayList<String>(); var pages = 0
                var more: Boolean
                do {
                    val page = (db.execute("drain", mapOf("now_ms" to ProtocolValue.StringValue("0"))) as ProtocolValue.ObjectValue).value
                    val events = (page.getValue("events") as ProtocolValue.ArrayValue).value
                    for (event in events) {
                        val fields = (event as ProtocolValue.ObjectValue).value
                        subscriptions.add((fields.getValue("subscription") as ProtocolValue.StringValue).value)
                        val rows = (((fields.getValue("value") as ProtocolValue.ObjectValue).value.getValue("rows")) as ProtocolValue.ArrayValue).value
                        check(rows.size == 120)
                        check(Row.fromProtocolValue(rows.first()).fields.getValue("text") == Cell.StringValue("x".repeat(4500)))
                    }
                    more = (page.getValue("has_more") as ProtocolValue.BooleanValue).value; pages++
                } while (more)
                check(pages == 2 && subscriptions == listOf(first.value, second.value))
                db.execute("unsubscribe", mapOf("subscription" to first)); db.execute("unsubscribe", mapOf("subscription" to second))
            } finally { db.close() }
        }
        case("K02_K11.aggregate_push_prefixes_retain_exact_core_batch") {
            val db = database("push-prefix", schemaNotes(), false)
            try {
                val definition = CommandDefinition("large-v1", mapOf("text" to CommandDefinition.Argument(CommandDefinition.Kind.String)))
                val argument = JSONValue.ObjectValue(mapOf("text" to JSONValue.StringValue("x".repeat(20000))))
                repeat(60) { db.command(definition, argument, CapturedContext(JSONValue.Null, 1)) { JSONValue.Null } }
                val keys = HashSet<String>(); var pages = 0; var more: Boolean
                do {
                    val page = db.preparePush(100, 0); pages++; more = page.hasMore
                    for (body in page.envelopes) {
                        val envelope = JSONValue.decodeJSON(body) as JSONValue.ObjectValue
                        check(keys.add((envelope.value.getValue("idempotencyKey") as JSONValue.StringValue).value))
                        check(((envelope.value.getValue("args") as JSONValue.ObjectValue).value.getValue("text") as JSONValue.StringValue).value.length == 20000)
                    }
                } while (more)
                check(keys.size == 60 && pages > 1); check(db.queueStatus().commands.all { it.attempts == 1L && it.nextDueMs == 1100L })
                check(db.preparePush(1099, 0).envelopes.isEmpty())
            } finally { db.close() }
        }
    }
    private suspend fun lateDriverResponse() = case("K15.late_sync_response_cannot_reach_replacement_owner_or_mapper") {
        val db = database("late-driver", schemaNotes(), false)
        try {
            val old = db.command(CommandDefinition("old-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 1)) { JSONValue.Null }
            val entered = CountDownLatch(1); val reply = AtomicReference<Continuation<String>>()
            val oldMaps = AtomicInteger(); val newMaps = AtomicInteger(); val lateErrors = AtomicInteger(); val finished = CountDownLatch(1)
            db.startSync(transport = { suspendCoroutine<String> { reply.set(it); entered.countDown() } }, reconciler = { oldMaps.incrementAndGet(); ReconcilePlan(emptyList()) }, now = { 0 }, jitter = { 0 }, onError = { lateErrors.incrementAndGet() })
            await(entered); db.close(); db.openMemory(schemaNotes(), "ffeeddccbbaa99887766554433221100")
            val current = db.command(CommandDefinition("new-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 2)) { JSONValue.Null }
            val observation = db.observeOutcome(current.key) { if (it is CommandOutcome.Applied) finished.countDown() }
            val fake = FakeTransport(mapOf("new-v1" to { RemoteOutcome.Applied(JSONValue.Null) }))
            db.startSync(fake::push, reconciler = { newMaps.incrementAndGet(); ReconcilePlan(emptyList()) }, now = { 0 }, jitter = { 0 })
            await(finished)
            val late = JSONValue.ArrayValue(listOf(JSONValue.ObjectValue(mapOf("idempotencyKey" to JSONValue.StringValue(old.key), "status" to JSONValue.StringValue("applied"), "result" to JSONValue.Null)))).encodeJSON()
            reply.get().resume(late); Thread.sleep(100)
            check(oldMaps.get() == 0 && newMaps.get() == 1 && lateErrors.get() == 0); check(db.queueStatus().commands.single().key == current.key); observation.close()
        } finally { db.close() }
    }
    private suspend fun conflictPolicy() {
        for (persistent in listOf(false, true)) {
            val suffix = if (persistent) "protected" else "memory"
            case("CP13.$suffix.packaged_policy_reports_and_callback_containment") {
                val callbacks = CopyOnWriteArrayList<String>(); val facts = CopyOnWriteArrayList<BackgroundError>(); val reported = CountDownLatch(2)
                val db = Database(ui, conflict = ConflictPolicy(ConflictReaction.Discard),
                    onResolution = { key, result ->
                        check(Looper.myLooper() == Looper.getMainLooper() && result is Resolution.KeptCurrent)
                        callbacks.add(key); throw AssertionError(CANARY)
                    }, onBackgroundError = { fact ->
                        check(Looper.myLooper() == Looper.getMainLooper()); facts.add(fact); reported.countDown(); throw AssertionError(CANARY)
                    })
                try {
                    if (persistent) db.openProtected(this, "conflict-$suffix.sqlite", key(), schemaNotes(), NS) else db.openMemory(schemaNotes(), NS)
                    suspend fun create(id: String) = db.command(CommandDefinition("create-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 0)) {
                        it.insert("notes", mapOf("text" to Cell.StringValue("optimistic")), id); JSONValue.Null
                    }
                    val first = create("local:manual")
                    val report = db.discard(first.key) as Resolution.KeptCurrent
                    check(report.discardedKeys == listOf(first.key) && report.unmapped == listOf("local:manual") && report.skippedWrites.value == "0")
                    val entry = db.queueStatus().commands.single(); check(entry.state == QueueState.Discarded && !entry.rolledBack)
                    val second = create("local:automatic")
                    db.preparePush(0, 0); db.receive(second.key, RemoteOutcome.Rejected("synthetic rule")); db.reconcile(second.key, ReconcilePlan(emptyList()))
                    db.replace("notes", "local:automatic", mapOf("text" to Cell.StringValue("direct"))); db.rollback(second.key)
                    await(reported)
                    check(callbacks == listOf(first.key, second.key) && facts.size == 2)
                    check(facts.all { it.source == BackgroundError.Source.Callback && it.kind == "CallbackFailure" && it.token == null && !it.toString().contains(CANARY) })
                    check(db.fetch(Query("notes")).single { it.id == "local:automatic" }.fields.getValue("text") == Cell.StringValue("direct"))
                    check(db.fetch(Query("notes")).any { it.id == "local:manual" })
                } finally { db.close() }
            }
        }
    }
    private suspend fun seed() = case("K15.seed_real_protected_queue_checkpoint_and_abandoned_scope") {
        val db = database("restart", schemaNotes(), true, { "local:restart" })
        try {
            db.configureEntities(EntityConfig("restart scope", listOf("notes"))) { CapturedContext(JSONValue.Null, 10) }
            val pull = db.beginPull(); db.applyPull(pull, PullPage("saved-checkpoint", true, listOf(EntityChange("notes", "server", EntityOperation.Create, mapOf("text" to Cell.StringValue("saved")), timestampMs = 10, fieldTimestamps = mapOf("text" to 10)))))
            db.command(CommandDefinition("restart-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 11)) { JSONValue.Null }
            db.preparePush(100, 0)
            val staged = CountDownLatch(1)
            heldDatabase = db
            heldTask = db.startCommand(CommandDefinition("abandoned-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, 12)) { scope ->
                scope.insert("notes", mapOf("text" to Cell.StringValue("must-not-survive")))
                suspendCoroutine<Unit> { staged.countDown() }
                JSONValue.Null
            }
            await(staged)
            check(db.fetch(Query("notes")).single().id == "server")
            // Intentionally retain actual owner and unfinished scope. The host
            // kills only this owned app after reading the durable seed report.
        } catch (failure: Throwable) { db.close(); heldDatabase = null; heldTask = null; throw failure }
    }
    private suspend fun restart() = case("K15.fresh_process_reopens_real_rows_frozen_retry_and_checkpoint") {
        val db = database("restart", schemaNotes(), true)
        try {
            check(db.fetch(Query("notes")).single().id == "server")
            val entry = db.queueStatus().commands.single(); check(entry.state == QueueState.Sent && entry.attempts == 1L && entry.nextDueMs == 1100L)
            check(db.syncProgress().ready)
            db.configureEntities(EntityConfig("restart scope", listOf("notes"))) { CapturedContext(JSONValue.Null, 20) }
            val pull = db.beginPull(); check(pull.since == "saved-checkpoint"); db.abortPull(pull)
            check(db.preparePush(1099, 0).envelopes.isEmpty()); check(db.preparePush(1100, 0).envelopes.size == 1)
        } finally { db.close() }
    }
}
