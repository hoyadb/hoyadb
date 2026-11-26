// TC-0036 K01: exact representations before real owner integration.
package dev.hoyadb

object ModelTests {
    @JvmStatic fun main(args: Array<String>) {
        var failures = 0
        fun run(name: String, body: () -> Unit) {
            try { body(); println("PASS $name") }
            catch (error: Throwable) { failures++; println("FAIL $name " + if (error is HoyaException) error.kind else error.javaClass.simpleName) }
        }
        run("K01_two_unrelated_schemas_relations_all_seven_migrations") {
            val a = Schema(1, mapOf("authors" to Table(mapOf("name" to Column(ColumnKind.String)),
                emptyMap()),
                "articles" to Table(mapOf("title" to Column(ColumnKind.String), "score" to Column(ColumnKind.Integer), "ratio" to Column(ColumnKind.Float), "active" to Column(ColumnKind.Boolean), "at" to Column(ColumnKind.Timestamp), "note" to Column(ColumnKind.String, true)),
                    mapOf("author" to Relation.ToOne("authors", "author_id"),
                        "tags" to Relation.ManyToMany("tags", "article_tags", "article", "tag"))),
                "tags" to Table(mapOf("label" to Column(ColumnKind.String))),
                "article_tags" to Table(emptyMap(), mapOf("article" to Relation.ToOne("articles", "article_id"), "tag" to Relation.ToOne("tags", "tag_id")))))
            val b = Schema(1, mapOf("sensors" to Table(mapOf("label" to Column(ColumnKind.String)),
                mapOf("readings" to Relation.HasMany("readings", "sensor_id"))),
                "readings" to Table(mapOf("value" to Column(ColumnKind.Float), "at" to Column(ColumnKind.Timestamp)),
                    mapOf("sensor" to Relation.ToOne("sensors", "sensor_id")))))
            val aWire = a.toProtocolValue().fields()
            check(aWire.getValue("version").string() == "1")
            check(aWire.getValue("tables").fields().keys == setOf("authors", "articles", "tags", "article_tags"))
            check(b.toProtocolValue().fields().getValue("tables").fields().keys == setOf("sensors", "readings"))
            val relation = aWire.getValue("tables").fields().getValue("articles").fields().getValue("relations").fields().getValue("tags").fields()
            check(relation.getValue("through").string() == "article_tags")
            val steps = listOf(MigrationStep.AddTable("scratch", Table(emptyMap())), MigrationStep.DropTable("scratch"),
                MigrationStep.AddColumn("articles", "extra", Column(ColumnKind.String, true), ColumnDefault.Some(Cell.Null)),
                MigrationStep.DropColumn("articles", "extra"), MigrationStep.RenameColumn("articles", "title", "name"),
                MigrationStep.Backfill("articles", "name", Cell.StringValue("filled")), MigrationStep.SetRelations("articles", emptyMap()))
            check(steps.map { it.toProtocolValue().fields().getValue("op").string() } == listOf("AddTable", "DropTable", "AddColumn", "DropColumn", "RenameColumn", "Backfill", "SetRelations"))
            val default = steps[2].toProtocolValue().fields().getValue("default").fields()
            check(default.getValue("kind").string() == "some")
            check(default.getValue("value") == Cell.Null.toProtocolValue())
            val migration = Migration(1, Schema(2, b.tables), steps).toProtocolValue().fields()
            check(migration.getValue("from_version").string() == "1")
            check(migration.getValue("steps").items().size == 7)
        }
        run("K01_query_all_predicates_related_source_and_atomic_write_shapes") {
            for (operator in Operator.values()) {
                val q = Query("articles", listOf(Predicate("score", operator, listOf(Cell.IntegerValue(Long.MIN_VALUE)))),
                    Order("author.name", Direction.Desc), 10, Source("authors", "a", "articles"))
                val wire = q.toProtocolValue().fields()
                check(wire.getValue("predicates").items()[0].fields().getValue("operator").string() == operator.name)
                check(wire.getValue("limit").string() == "10")
                check(wire.getValue("source").fields().getValue("id").string() == "a")
            }
            val writes = listOf(Write.Insert("notes", mapOf("title" to Cell.StringValue("new")), "fixed"),
                Write.Replace("notes", "fixed", emptyMap()), Write.Remove("notes", "fixed"))
            check(writes.map { it.toProtocolValue().fields().getValue("op").string() } == listOf("insert", "replace", "remove"))
            check(writes[0].toProtocolValue().fields().getValue("proposed_id").string() == "fixed")
        }
        run("K01_rows_and_schema_copy_mutable_inputs") {
            val fields = mutableMapOf<String, Cell>("title" to Cell.StringValue("before"))
            val row = Row("n", fields)
            fields.clear()
            check(row.fields.getValue("title") == Cell.StringValue("before"))
            val tables = mutableMapOf("notes" to Table(mapOf("title" to Column(ColumnKind.String))))
            val schema = Schema(1, tables)
            tables.clear()
            check(schema.tables.keys == setOf("notes"))
            var rejected = false
            try { (row.fields as MutableMap).clear() } catch (_: UnsupportedOperationException) { rejected = true }
            check(rejected)
        }
        fun expectKind(expected: String, body: () -> Unit) {
            try { body(); error("expected rejection") }
            catch (error: HoyaException) { check(error.kind == expected) }
        }
        run("K01_nested_snapshots_preserve_query_writes_and_migration_inputs") {
            val columns = mutableMapOf("title" to Column(ColumnKind.String))
            val relations = mutableMapOf<String, Relation>("owner" to Relation.ToOne("users", "owner_id", true))
            val table = Table(columns, relations)
            val values = mutableListOf<Cell>(Cell.IntegerValue(Long.MAX_VALUE))
            val predicates = mutableListOf(Predicate("score", Operator.In, values))
            val query = Query("notes", predicates)
            val fields = mutableMapOf<String, Cell>("title" to Cell.StringValue("before"))
            val insert = Write.Insert("notes", fields)
            val replace = Write.Replace("notes", "n", fields)
            val relationStep = MigrationStep.SetRelations("notes", relations)
            val steps = mutableListOf<MigrationStep>(relationStep)
            val migration = Migration(1, Schema(2, mapOf("notes" to table)), steps)
            columns.clear(); relations.clear(); values.clear(); predicates.clear(); fields.clear(); steps.clear()
            check(table.columns.keys == setOf("title"))
            check(table.relations.keys == setOf("owner"))
            check(query.toProtocolValue().fields().getValue("predicates").items()[0].fields().getValue("values").items()[0] == Cell.IntegerValue(Long.MAX_VALUE).toProtocolValue())
            check(insert.toProtocolValue().fields().getValue("fields").fields().getValue("title") == Cell.StringValue("before").toProtocolValue())
            check(replace.toProtocolValue().fields().getValue("fields") == insert.toProtocolValue().fields().getValue("fields"))
            check(insert.toProtocolValue().fields().getValue("proposed_id") == ProtocolValue.Null)
            check(migration.toProtocolValue().fields().getValue("steps").items()[0].fields().getValue("relations").fields().keys == setOf("owner"))
            var rejected = 0
            try { (query.predicates as MutableList).clear() } catch (_: UnsupportedOperationException) { rejected++ }
            try { (table.columns as MutableMap).clear() } catch (_: UnsupportedOperationException) { rejected++ }
            try { (migration.steps as MutableList).clear() } catch (_: UnsupportedOperationException) { rejected++ }
            check(rejected == 3)
        }
        run("K01_row_and_change_decoders_preserve_null_absence_and_exact_shapes") {
            val rowWire = obj("id" to str("n"), "fields" to obj(
                "score" to obj("kind" to str("integer"), "value" to str("-9223372036854775808")),
                "note" to obj("kind" to str("null"))))
            val row = Row.fromProtocolValue(rowWire)
            check(row.id == "n" && row.fields.getValue("score") == Cell.IntegerValue(Long.MIN_VALUE))
            check(row.fields["note"] == Cell.Null && !row.fields.containsKey("omitted"))
            check(row.toProtocolValue() == rowWire)
            val change = Change.fromProtocolValue(obj("table" to str("notes"), "id" to str("n"), "before" to rowWire, "after" to ProtocolValue.Null))
            check(change.table == "notes" && change.id == "n" && change.before?.fields == row.fields && change.after == null)
            expectKind("InvalidResponse") { Row.fromProtocolValue(obj("id" to str("n"), "fields" to obj(), "extra" to ProtocolValue.Null)) }
            expectKind("InvalidResponse") { Row.fromProtocolValue(obj("id" to str("n"))) }
            expectKind("InvalidResponse") { Change.fromProtocolValue(obj("table" to str("notes"), "id" to str("n"), "before" to ProtocolValue.Null)) }
        }
        run("K02_schema_widths_exact_enums_and_deferred_core_validation") {
            val schema = Schema(4294967295L, mapOf("notes" to Table(mapOf("title" to Column(ColumnKind.String, true)))))
            val decoded = Schema.fromProtocolValue(schema.toProtocolValue())
            check(decoded.version == 4294967295L && decoded.tables.getValue("notes").columns.getValue("title") == Column(ColumnKind.String, true))
            expectKind("InvalidRequest") { Schema(-1, emptyMap()).toProtocolValue() }
            expectKind("InvalidRequest") { Schema(4294967296L, emptyMap()).toProtocolValue() }
            expectKind("InvalidRequest") { Query("notes", limit = -1).toProtocolValue() }
            expectKind("InvalidResponse") { Schema.fromProtocolValue(obj("version" to str("01"), "tables" to obj())) }
            expectKind("InvalidResponse") { Schema.fromProtocolValue(obj("version" to str("4294967296"), "tables" to obj())) }
            expectKind("InvalidResponse") { Schema.fromProtocolValue(obj("version" to str("1"), "tables" to obj("notes" to obj("columns" to obj("title" to obj("kind" to str("Bogus"), "optional" to ProtocolValue.BooleanValue(false))), "relations" to obj())))) }
            // Core, not the DTO encoder, owns reserved-name and field-path validation.
            check(Schema(1, mapOf("__hoya_bad" to Table(emptyMap()))).toProtocolValue().fields().getValue("tables").fields().containsKey("__hoya_bad"))
            check(Query("notes", listOf(Predicate("bad..path", Operator.Eq, emptyList()))).toProtocolValue().fields().getValue("predicates").items().size == 1)
        }
        run("G24_source_implicit_formatting_redacts_identity_without_changing_data") {
            val canary = "synthetic-private-row-DEF0016"
            val source = Source("authors", canary, "articles")
            check(!source.toString().contains(canary))
            check(!listOf(source).toString().contains(canary))
            check(source.id == canary)
            val same = source.copy()
            check(same == source && same.hashCode() == source.hashCode())
            check(source.copy(id = "other").id == "other" && source.id == canary)
            val wire = Query("articles", source = source).toProtocolValue().fields()
            check(wire.getValue("source").fields().getValue("id").string() == canary)
        }
        System.exit(if (failures == 0) 0 else 1)
    }
}
