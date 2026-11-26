package dev.hoyadb

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.Executor
import kotlin.coroutines.*

internal fun <T> blocking(body: suspend () -> T): T {
    val done = CountDownLatch(1); var outcome: Result<T>? = null
    body.startCoroutine(object : Continuation<T> { override val context = EmptyCoroutineContext; override fun resumeWith(result: Result<T>) { outcome = result; done.countDown() } })
    check(done.await(30, TimeUnit.SECONDS)); return outcome!!.getOrThrow()
}
internal fun notesSchema(version: Long = 1) = Schema(version, mapOf("notes" to Table(mapOf("text" to Column(ColumnKind.String)))))
internal const val TEST_NAMESPACE = "00112233445566778899aabbccddeeff"
object OwnerTests {
    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun test(name: String, body: suspend () -> Unit) { try { blocking(body); println("PASS $name") } catch (error: Throwable) { failures++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) } }
        test("K03_real_hermetic_memory_owner_injected_exact_ID") {
            val db = Database(Executor { it.run() }, { "test-fixed-id-001" })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE)
                val protection = db.protection(); check(!protection.encrypted && protection.backup == "not_applicable")
                check(db.insert("notes", mapOf("text" to Cell.StringValue("東京🌿"))).id == "test-fixed-id-001")
                check(db.fetch(Query("notes")).single().fields.getValue("text") == Cell.StringValue("東京🌿"))
                try { db.insert("notes", mapOf("text" to Cell.StringValue("duplicate"))); error("duplicate accepted") } catch (error: HoyaException) { check(error.kind == "RowAlreadyExists") }
            } finally { db.close() }
        }
        test("K04_real_retained_memory_closed_drop_and_failed_reopen_preserves") {
            val store = MemoryStore.create(); val db = Database(Executor { it.run() }, { "one" })
            try {
                db.openMemory(notesSchema(), TEST_NAMESPACE, store)
                db.insert("notes", mapOf("text" to Cell.StringValue("saved")))
                try { store.drop(); error("active drop") } catch (error: HoyaException) { check(error.kind == "Busy") }
                db.close()
                val invalid = Schema(1, mapOf("other" to Table(emptyMap())))
                try { db.openMemory(invalid, TEST_NAMESPACE, store); error("schema drift accepted") } catch (error: HoyaException) { check(error.kind == "SchemaConflict") }
                db.openMemory(notesSchema(), TEST_NAMESPACE, store)
                check(db.fetch(Query("notes")).single().id == "one"); db.close()
                val v2 = Schema(2, mapOf("notes" to Table(mapOf("text" to Column(ColumnKind.String), "extra" to Column(ColumnKind.String, true)))))
                db.openMemory(v2, TEST_NAMESPACE, store, listOf(Migration(1, v2, listOf(MigrationStep.AddColumn("notes", "extra", Column(ColumnKind.String, true))))))
                check(db.fetch(Query("notes")).single().fields.getValue("text") == Cell.StringValue("saved"))
            } finally { db.close(); store.drop() }
        }
        test("K05_close_during_delayed_open_disposes_before_replacement") {
            var pending: HoyaNativeTransport.Reply? = null
            var closeCalls = 0
            val fake = object : NativeCalling {
                override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) { check(pending == null); pending = reply }
                override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) {
                    check(WireCodec.decode(frame).fields().getValue("op").string() == "close"); closeCalls++
                    reply.onSuccess(WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to ProtocolValue.Null)))
                }
            }
            val db = Database(fake, Executor { it.run() }, null, DatabaseOptions())
            var openError: String? = null; var closed = false; val disposed = CountDownLatch(1); val openReturned = CountDownLatch(1)
            val first: suspend () -> Unit = { try { db.openMemory(notesSchema(), TEST_NAMESPACE) } catch (error: HoyaException) { openError = error.kind } }
            first.startCoroutine(object : Continuation<Unit> { override val context = EmptyCoroutineContext; override fun resumeWith(result: Result<Unit>) { result.getOrThrow(); openReturned.countDown() } })
            check(pending != null)
            val closing: suspend () -> Unit = { db.close(); closed = true; disposed.countDown() }
            closing.startCoroutine(object : Continuation<Unit> { override val context = EmptyCoroutineContext; override fun resumeWith(result: Result<Unit>) { result.getOrThrow() } })
            check(!closed)
            try { db.openMemory(notesSchema(), TEST_NAMESPACE); error("racing open") } catch (error: HoyaException) { check(error.kind == "Busy") }
            pending!!.onSuccess(WireCodec.encode(obj("abi" to str("1"), "ok" to ProtocolValue.BooleanValue(true), "value" to str("1"))))
            check(disposed.await(30, TimeUnit.SECONDS) && openReturned.await(30, TimeUnit.SECONDS))
            check(closed && closeCalls == 1 && openError == "Closed")
        }
        test("K13_invalid_key_precedes_even_context_path_getters") {
            var touched = false
            val context = object : android.content.ContextWrapper(null) {
                override fun getNoBackupFilesDir(): java.io.File { touched = true; error("path touched") }
                override fun getFilesDir(): java.io.File { touched = true; error("path touched") }
            }
            val db = Database(Executor { it.run() })
            try { db.openProtected(context, "owned.sqlite", ByteArray(31), notesSchema(), TEST_NAMESPACE); error("bad key accepted") }
            catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
            check(!touched)
        }
        test("K02_injected_empty_exhausted_throwing_generator_never_falls_back") {
            for (generator in listOf<() -> String?>({ null }, { "" }, { throw IllegalStateException("synthetic private canary") })) {
                val db = Database(Executor { it.run() }, generator)
                try {
                    db.openMemory(notesSchema(), TEST_NAMESPACE)
                    try { db.insert("notes", mapOf("text" to Cell.StringValue("rejected"))); error("generator fallback") }
                    catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
                    check(db.fetch(Query("notes")).isEmpty())
                } finally { db.close() }
            }
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
