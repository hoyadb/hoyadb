// API-0011 / TC-0036: typed owned translation; schema and query semantics stay in core.
package dev.hoyadb

internal fun wireU32(value: Long): ProtocolValue {
    if (value !in 0..4294967295L) throw HoyaException("InvalidRequest")
    return decimal(value)
}

enum class ColumnKind { String, Integer, Float, Boolean, Timestamp, Reference }
data class Column(val kind: ColumnKind, val optional: Boolean = false) {
    fun toProtocolValue(): ProtocolValue = obj("kind" to str(kind.name), "optional" to ProtocolValue.BooleanValue(optional))
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Column {
            val fields = wire.exact("kind", "optional")
            val kind = ColumnKind.entries.firstOrNull { it.name == fields.getValue("kind").string() } ?: invalidResponse()
            return Column(kind, fields.getValue("optional").boolean())
        }
    }
}
sealed class Relation {
    data class ToOne(val table: String, val foreignKey: String, val optional: Boolean = false) : Relation()
    data class HasMany(val table: String, val foreignKey: String) : Relation()
    data class ManyToMany(val table: String, val through: String, val sourceRelation: String, val targetRelation: String) : Relation()
    fun toProtocolValue(): ProtocolValue = when (this) {
        is ToOne -> obj("kind" to str("ToOne"), "table" to str(table), "foreign_key" to str(foreignKey), "optional" to ProtocolValue.BooleanValue(optional))
        is HasMany -> obj("kind" to str("HasMany"), "table" to str(table), "foreign_key" to str(foreignKey))
        is ManyToMany -> obj("kind" to str("ManyToMany"), "table" to str(table), "through" to str(through), "source_relation" to str(sourceRelation), "target_relation" to str(targetRelation))
    }
    final override fun toString() = "<Relation>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Relation {
            val fields = wire.fields()
            return when (fields["kind"]?.string() ?: invalidResponse()) {
                "ToOne" -> { wire.exact("kind", "table", "foreign_key", "optional"); ToOne(fields.getValue("table").string(), fields.getValue("foreign_key").string(), fields.getValue("optional").boolean()) }
                "HasMany" -> { wire.exact("kind", "table", "foreign_key"); HasMany(fields.getValue("table").string(), fields.getValue("foreign_key").string()) }
                "ManyToMany" -> { wire.exact("kind", "table", "through", "source_relation", "target_relation"); ManyToMany(fields.getValue("table").string(), fields.getValue("through").string(), fields.getValue("source_relation").string(), fields.getValue("target_relation").string()) }
                else -> invalidResponse()
            }
        }
    }
}
class Table(columns: Map<String, Column>, relations: Map<String, Relation> = emptyMap()) {
    val columns = immutableMap(columns)
    val relations = immutableMap(relations)
    fun toProtocolValue(): ProtocolValue = obj("columns" to ProtocolValue.ObjectValue(columns.mapValues { it.value.toProtocolValue() }), "relations" to relationsWire(relations))
    override fun toString() = "<Table>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Table {
            val fields = wire.exact("columns", "relations")
            return Table(fields.getValue("columns").fields().mapValues { Column.fromProtocolValue(it.value) }, fields.getValue("relations").fields().mapValues { Relation.fromProtocolValue(it.value) })
        }
    }
}
private fun relationsWire(relations: Map<String, Relation>): ProtocolValue = ProtocolValue.ObjectValue(relations.mapValues { it.value.toProtocolValue() })
class Schema(val version: Long, tables: Map<String, Table>) {
    val tables = immutableMap(tables)
    fun toProtocolValue(): ProtocolValue = obj("version" to wireU32(version), "tables" to ProtocolValue.ObjectValue(tables.mapValues { it.value.toProtocolValue() }))
    override fun toString() = "<Schema>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Schema {
            val fields = wire.exact("version", "tables")
            return Schema(fields.getValue("version").u32(), fields.getValue("tables").fields().mapValues { Table.fromProtocolValue(it.value) })
        }
    }
}
sealed class ColumnDefault {
    object None : ColumnDefault()
    data class Some(val value: Cell) : ColumnDefault()
    internal fun wire(): ProtocolValue = when (this) {
        None -> obj("kind" to str("none"))
        is Some -> obj("kind" to str("some"), "value" to value.toProtocolValue())
    }
}
sealed class MigrationStep {
    data class AddTable(val table: String, val definition: Table) : MigrationStep()
    data class DropTable(val table: String) : MigrationStep()
    data class AddColumn(val table: String, val name: String, val column: Column, val default: ColumnDefault = ColumnDefault.None) : MigrationStep()
    data class DropColumn(val table: String, val name: String) : MigrationStep()
    data class RenameColumn(val table: String, val from: String, val to: String) : MigrationStep()
    data class Backfill(val table: String, val column: String, val value: Cell) : MigrationStep()
    class SetRelations(val table: String, relations: Map<String, Relation>) : MigrationStep() {
        val relations = immutableMap(relations)
    }
    fun toProtocolValue(): ProtocolValue = when (this) {
        is AddTable -> obj("op" to str("AddTable"), "table" to str(table), "definition" to definition.toProtocolValue())
        is DropTable -> obj("op" to str("DropTable"), "table" to str(table))
        is AddColumn -> obj("op" to str("AddColumn"), "table" to str(table), "name" to str(name), "column" to column.toProtocolValue(), "default" to default.wire())
        is DropColumn -> obj("op" to str("DropColumn"), "table" to str(table), "name" to str(name))
        is RenameColumn -> obj("op" to str("RenameColumn"), "table" to str(table), "from" to str(from), "to" to str(to))
        is Backfill -> obj("op" to str("Backfill"), "table" to str(table), "column" to str(column), "value" to value.toProtocolValue())
        is SetRelations -> obj("op" to str("SetRelations"), "table" to str(table), "relations" to relationsWire(relations))
    }
    final override fun toString() = "<MigrationStep>"
}
class Migration(val fromVersion: Long, val to: Schema, steps: List<MigrationStep>) {
    val steps = immutableList(steps)
    fun toProtocolValue(): ProtocolValue = obj("from_version" to wireU32(fromVersion), "to" to to.toProtocolValue(), "steps" to ProtocolValue.ArrayValue(steps.map { it.toProtocolValue() }))
    override fun toString() = "<Migration>"
}
enum class Operator { Eq, Ne, Lt, Le, Gt, Ge, In }
enum class Direction { Asc, Desc }
class Predicate(val field: String, val operator: Operator, values: List<Cell>) {
    val values = immutableList(values)
    fun toProtocolValue(): ProtocolValue = obj("field" to str(field), "operator" to str(operator.name), "values" to ProtocolValue.ArrayValue(values.map { it.toProtocolValue() }))
    override fun toString() = "<Predicate>"
}
data class Order(val field: String, val direction: Direction = Direction.Asc)
data class Source(val table: String, val id: String, val relation: String) {
    override fun toString() = "<Source>"
}
class Query(val table: String, predicates: List<Predicate> = emptyList(), val order: Order? = null, val limit: Long? = null, val source: Source? = null) {
    val predicates = immutableList(predicates)
    fun toProtocolValue(): ProtocolValue = obj(
        "table" to str(table), "predicates" to ProtocolValue.ArrayValue(predicates.map { it.toProtocolValue() }),
        "order" to (order?.let { obj("field" to str(it.field), "direction" to str(it.direction.name)) } ?: ProtocolValue.Null),
        "limit" to (limit?.let { str(UnsignedDecimal.fromLong(it).value) } ?: ProtocolValue.Null),
        "source" to (source?.let { obj("table" to str(it.table), "id" to str(it.id), "relation" to str(it.relation)) } ?: ProtocolValue.Null))
    override fun toString() = "<Query>"
}
sealed class Write {
    class Insert(val table: String, fields: Map<String, Cell>, val proposedId: String? = null) : Write() { val fields = immutableMap(fields) }
    class Replace(val table: String, val id: String, fields: Map<String, Cell>) : Write() { val fields = immutableMap(fields) }
    data class Remove(val table: String, val id: String) : Write()
    fun toProtocolValue(): ProtocolValue = when (this) {
        is Insert -> obj("op" to str("insert"), "table" to str(table), "fields" to fieldsWire(fields), "proposed_id" to nullable(proposedId))
        is Replace -> obj("op" to str("replace"), "table" to str(table), "id" to str(id), "fields" to fieldsWire(fields))
        is Remove -> obj("op" to str("remove"), "table" to str(table), "id" to str(id))
    }
    final override fun toString() = "<Write>"
}
class Row(val id: String, fields: Map<String, Cell>) {
    val fields = immutableMap(fields)
    fun toProtocolValue(): ProtocolValue = obj("id" to str(id), "fields" to fieldsWire(fields))
    override fun toString() = "<Row>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Row {
            val fields = wire.exact("id", "fields")
            return Row(fields.getValue("id").string(), fieldsFrom(fields.getValue("fields")))
        }
    }
}
class Change(val table: String, val id: String, val before: Row?, val after: Row?) {
    override fun toString() = "<Change>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Change {
            val fields = wire.exact("table", "id", "before", "after")
            fun row(value: ProtocolValue): Row? = if (value === ProtocolValue.Null) null else Row.fromProtocolValue(value)
            return Change(fields.getValue("table").string(), fields.getValue("id").string(), row(fields.getValue("before")), row(fields.getValue("after")))
        }
    }
}
