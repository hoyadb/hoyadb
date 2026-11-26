package dev.hoyadb

import java.io.IOException
import java.util.concurrent.Executor

object LifetimeTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) {
            try { blocking(body); println("PASS $name") }
            catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) }
        }
        test("K15_scoped_owner_closes_after_success_and_preserves_return_value") {
            val db = Database(Executor { it.run() })
            try {
                val result = useDatabase(db) {
                    it.openMemory(notesSchema(), TEST_NAMESPACE)
                    it.insert("notes", mapOf("text" to Cell.StringValue("owned")), "one")
                    42
                }
                check(result == 42)
                try { db.fetch(Query("notes")); error("scope leaked") }
                catch (error: HoyaException) { check(error.kind == "Closed") }
            } finally { db.close() }
        }
        test("K15_scoped_owner_closes_after_application_failure") {
            val db = Database(Executor { it.run() })
            val original = IOException("synthetic scoped failure")
            try {
                try {
                    useDatabase<Unit>(db) { it.openMemory(notesSchema(), TEST_NAMESPACE); throw original }
                    error("application failure disappeared")
                } catch (error: IOException) { check(error === original) }
                try { db.protection(); error("scope leaked") }
                catch (error: HoyaException) { check(error.kind == "Closed") }
            } finally { db.close() }
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
