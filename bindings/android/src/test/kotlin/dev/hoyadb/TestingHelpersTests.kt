package dev.hoyadb

import dev.hoyadb.testing.*
import java.util.concurrent.Executor

object TestingHelpersTests {
    @JvmStatic fun main(args: Array<String>) {
        var failed = 0
        fun test(name: String, body: suspend () -> Unit) {
            try { blocking(body); println("PASS $name") }
            catch (error: Throwable) { failed++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) }
        }
        test("KT01_preloaded_real_memory_factory_accepts_two_caller_schemas") {
            val runtime = preloadRuntime()
            check(!runtime.toString().contains(TEST_NAMESPACE))
            val ui = Executor { it.run() }
            val firstSchema = Schema(1, mapOf(
                "authors" to Table(mapOf("name" to Column(ColumnKind.String))),
                "articles" to Table(mapOf("title" to Column(ColumnKind.String), "score" to Column(ColumnKind.Integer), "ratio" to Column(ColumnKind.Float), "active" to Column(ColumnKind.Boolean), "at" to Column(ColumnKind.Timestamp), "note" to Column(ColumnKind.String, true)),
                    mapOf("author" to Relation.ToOne("authors", "author_id"), "tags" to Relation.ManyToMany("tags", "article_tags", "article", "tag"))),
                "tags" to Table(mapOf("label" to Column(ColumnKind.String))),
                "article_tags" to Table(emptyMap(), mapOf("article" to Relation.ToOne("articles", "article_id"), "tag" to Relation.ToOne("tags", "tag_id")))))
            val first = openMemoryDatabase(runtime, firstSchema, TEST_NAMESPACE, ui)
            try {
                first.insert("authors", mapOf("name" to Cell.StringValue("東京")), "author")
                val row = first.insert("articles", mapOf("title" to Cell.StringValue("caller A"), "score" to Cell.IntegerValue(Long.MIN_VALUE), "ratio" to Cell.FloatValue(42.0), "active" to Cell.BooleanValue(true), "at" to Cell.TimestampValue(Long.MIN_VALUE), "note" to Cell.Null, "author_id" to Cell.ReferenceValue("author")), "one")
                check(row.id == "one" && row.fields.getValue("note") == Cell.Null)
                check(first.fetch(Query("authors", source = Source("articles", "one", "author"))).single().id == "author")
            }
            finally { first.close() }
            val secondSchema = Schema(1, mapOf(
                "sensors" to Table(mapOf("label" to Column(ColumnKind.String)), mapOf("readings" to Relation.HasMany("readings", "sensor_id"))),
                "readings" to Table(mapOf("value" to Column(ColumnKind.Float), "at" to Column(ColumnKind.Timestamp)), mapOf("sensor" to Relation.ToOne("sensors", "sensor_id")))))
            val second = openMemoryDatabase(runtime, secondSchema, TEST_NAMESPACE, ui)
            try {
                second.insert("sensors", mapOf("label" to Cell.StringValue("caller B")), "sensor")
                second.insert("readings", mapOf("value" to Cell.FloatValue(42.0), "at" to Cell.TimestampValue(Long.MIN_VALUE), "sensor_id" to Cell.ReferenceValue("sensor")), "reading")
                check(second.fetch(Query("readings", source = Source("sensors", "sensor", "readings"))).single().fields.getValue("at") == Cell.TimestampValue(Long.MIN_VALUE))
                check(!second.protection().encrypted && second.protection().backup == "not_applicable")
            } finally { second.close() }
        }
        test("KT02_explicit_namespace_and_exact_injected_clock_IDs_no_fallback") {
            val runtime = preloadRuntime(); val ui = Executor { it.run() }
            try { openMemoryDatabase(runtime, notesSchema(), "invalid", ui); error("namespace fallback") }
            catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
            val clock = ManualClock(123)
            var proposed = "test-fixed-id-001"
            val db = openMemoryDatabase(runtime, notesSchema(), TEST_NAMESPACE, ui, idGenerator = { proposed })
            try {
                check(db.insert("notes", mapOf("text" to Cell.StringValue("ordinary"))).id == "test-fixed-id-001")
                proposed = "local:test-fixed-id-001"
                db.command(CommandDefinition("fixed-v1"), JSONValue.ObjectValue(emptyMap()), CapturedContext(JSONValue.Null, clock.now())) { scope ->
                    check(scope.context().nowMilliseconds == 123L)
                    check(scope.insert("notes", mapOf("text" to Cell.StringValue("scoped"))).id == proposed)
                    JSONValue.Null
                }
                check(db.fetch(Query("notes")).map { it.id }.toSet() == setOf("test-fixed-id-001", "local:test-fixed-id-001"))
            } finally { db.close() }
        }
        test("KT03_manual_clock_checked_overflow_is_atomic_and_formatting_opaque") {
            val clock = ManualClock(Long.MAX_VALUE)
            check(clock.now() == Long.MAX_VALUE)
            try { clock.advance(1); error("overflow accepted") }
            catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
            check(clock.now() == Long.MAX_VALUE)
            clock.set(Long.MIN_VALUE)
            try { clock.advance(-1); error("underflow accepted") }
            catch (error: HoyaException) { check(error.kind == "InvalidRequest") }
            check(clock.now() == Long.MIN_VALUE)
            clock.set(10); clock.advance(5); check(clock.now() == 15L)
            check(!clock.toString().contains("15"))
        }
        System.exit(if (failed == 0) 0 else 1)
    }
}
