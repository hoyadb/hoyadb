// API-0011 / TC-0036 K09–K12: owned structural translation, no sync algorithms.
package dev.hoyadb

private fun ProtocolValue.optionalString(): String? = if (this === ProtocolValue.Null) null else string()
private fun ProtocolValue.optionalSigned(): Long? = if (this === ProtocolValue.Null) null else signed()
private fun strings(values: List<String>): ProtocolValue = ProtocolValue.ArrayValue(values.map(::str))

enum class EntityOperation(internal val wireName: String) { Create("create"), Update("update"), Delete("delete") }
class EntityConfig(val scope: String, tables: List<String>) {
    val tables = immutableList(tables)
    fun toProtocolValue(): ProtocolValue = obj("scope" to str(scope), "tables" to strings(tables))
    override fun toString() = "<EntityConfig>"
}
class EntityChange(val table: String, val recordId: String, val operation: EntityOperation,
    changes: Map<String, Cell> = emptyMap(), removedFields: List<String> = emptyList(),
    val timestampMs: Long, fieldTimestamps: Map<String, Long> = emptyMap()) {
    val changes = immutableMap(changes)
    val removedFields = immutableList(removedFields)
    val fieldTimestamps = immutableMap(fieldTimestamps)
    fun toProtocolValue(): ProtocolValue = obj(
        "table" to str(table), "record_id" to str(recordId), "op" to str(operation.wireName),
        "changes" to fieldsWire(changes), "removed_fields" to strings(removedFields),
        "timestamp_ms" to decimal(timestampMs), "field_timestamps" to ProtocolValue.ObjectValue(fieldTimestamps.mapValues { decimal(it.value) }))
    override fun toString() = "<EntityChange>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): EntityChange {
            val fields = wire.exact("table", "record_id", "op", "changes", "removed_fields", "timestamp_ms", "field_timestamps")
            val operation = EntityOperation.entries.firstOrNull { it.wireName == fields.getValue("op").string() } ?: invalidResponse()
            return EntityChange(fields.getValue("table").string(), fields.getValue("record_id").string(), operation,
                fieldsFrom(fields.getValue("changes")), fields.getValue("removed_fields").items().map { it.string() },
                fields.getValue("timestamp_ms").signed(), fields.getValue("field_timestamps").fields().mapValues { it.value.signed() })
        }
    }
}
class PullPage(val checkpoint: String, val complete: Boolean, changes: List<EntityChange>) {
    val changes = immutableList(changes)
    fun toProtocolValue(): ProtocolValue = obj("checkpoint" to str(checkpoint), "complete" to ProtocolValue.BooleanValue(complete), "changes" to ProtocolValue.ArrayValue(changes.map { it.toProtocolValue() }))
    override fun toString() = "<PullPage>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): PullPage {
            val fields = wire.exact("checkpoint", "complete", "changes")
            return PullPage(fields.getValue("checkpoint").string(), fields.getValue("complete").boolean(), fields.getValue("changes").items().map { EntityChange.fromProtocolValue(it) })
        }
    }
}
class PullRequest(val request: UnsignedDecimal, val since: String?, val scope: String, val generation: Long) {
    override fun toString() = "<PullRequest>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): PullRequest {
            val fields = wire.exact("request", "since", "scope", "generation")
            val handle = fields.getValue("request").unsigned()
            if (handle.value == "0") invalidResponse()
            return PullRequest(handle, fields.getValue("since").optionalString(), fields.getValue("scope").string(), fields.getValue("generation").signed())
        }
    }
}
sealed class RemoteOutcome {
    data class Applied(val result: JSONValue) : RemoteOutcome()
    data class Corrected(val result: JSONValue) : RemoteOutcome()
    data class Rejected(val reason: String) : RemoteOutcome()
    fun toProtocolValue(): ProtocolValue = when (this) {
        is Applied -> obj("status" to str("applied"), "result" to result.toProtocolValue())
        is Corrected -> obj("status" to str("corrected"), "result" to result.toProtocolValue())
        is Rejected -> obj("status" to str("rejected"), "reason" to str(reason))
    }
    final override fun toString() = "<RemoteOutcome>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): RemoteOutcome {
            val fields = wire.fields()
            return when (fields["status"]?.string() ?: invalidResponse()) {
                "applied" -> { wire.exact("status", "result"); Applied(JSONValue.fromProtocolValue(fields.getValue("result"))) }
                "corrected" -> { wire.exact("status", "result"); Corrected(JSONValue.fromProtocolValue(fields.getValue("result"))) }
                "rejected" -> { wire.exact("status", "reason"); Rejected(fields.getValue("reason").string()) }
                else -> invalidResponse()
            }
        }
    }
}
sealed class PlanWrite {
    class Put(val table: String, val id: String, fields: Map<String, Cell>) : PlanWrite() { val fields = immutableMap(fields) }
    data class Remove(val table: String, val id: String) : PlanWrite()
    data class Entity(val change: EntityChange) : PlanWrite()
    fun toProtocolValue(): ProtocolValue = when (this) {
        is Put -> obj("op" to str("put"), "table" to str(table), "id" to str(id), "fields" to fieldsWire(fields))
        is Remove -> obj("op" to str("remove"), "table" to str(table), "id" to str(id))
        is Entity -> obj("op" to str("entity"), "change" to change.toProtocolValue())
    }
    final override fun toString() = "<PlanWrite>"
}
/** SPEC-0039: an explicit expected before-image of a row outside the command closure; null expects absence. */
class CorrectionGuard(val table: String, val id: String, expected: Map<String, Cell>?) {
    val expected: Map<String, Cell>? = expected?.let { immutableMap(it) }
    fun toProtocolValue(): ProtocolValue = obj("table" to str(table), "id" to str(id), "expected" to (expected?.let { fieldsWire(it) } ?: ProtocolValue.Null))
    override fun toString() = "<CorrectionGuard>"
}
class ReconcilePlan(writes: List<PlanWrite>, mappings: Map<String, String> = emptyMap()) {
    val writes = immutableList(writes)
    val mappings = immutableMap(mappings)
    fun toProtocolValue(): ProtocolValue = obj("writes" to ProtocolValue.ArrayValue(writes.map { it.toProtocolValue() }), "mappings" to ProtocolValue.ObjectValue(mappings.mapValues { str(it.value) }))
    override fun toString() = "<ReconcilePlan>"
}
class PendingReconciliation(val key: String, val outcome: RemoteOutcome) {
    override fun toString() = "<PendingReconciliation>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): PendingReconciliation {
            val fields = wire.exact("key", "outcome")
            return PendingReconciliation(fields.getValue("key").string(), RemoteOutcome.fromProtocolValue(fields.getValue("outcome")))
        }
    }
}
enum class QueueKind { Command, Entity }
enum class QueueState(internal val wireName: String) {
    Queued("queued"), Sent("sent"), Acknowledged("acknowledged"), Applied("applied"), Corrected("corrected"), Rejected("rejected"), Cancelled("cancelled"), Discarded("discarded")
}
class QueueEntry(val kind: QueueKind, val table: String?, val recordId: String?, val key: String, val state: QueueState,
    blockedOn: List<String>, val awaitingLocal: Boolean, val rolledBack: Boolean, val attempts: Long, val nextDueMs: Long?, val awaitingKey: String? = null) {
    val blockedOn = immutableList(blockedOn)
    override fun toString() = "<QueueEntry>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): QueueEntry {
            // SPEC-0038: the exact eleven-field entry; awaiting_key is the row-causal blocker or null.
            val fields = wire.exact("kind", "table", "record_id", "key", "state", "blocked_on", "awaiting_local", "awaiting_key", "rolled_back", "attempts", "next_due_ms")
            val kind = when (fields.getValue("kind").string()) { "command" -> QueueKind.Command; "entity" -> QueueKind.Entity; else -> invalidResponse() }
            val state = QueueState.entries.firstOrNull { it.wireName == fields.getValue("state").string() } ?: invalidResponse()
            return QueueEntry(kind, fields.getValue("table").optionalString(), fields.getValue("record_id").optionalString(), fields.getValue("key").string(), state,
                fields.getValue("blocked_on").items().map { it.string() }, fields.getValue("awaiting_local").boolean(), fields.getValue("rolled_back").boolean(),
                fields.getValue("attempts").u32(), fields.getValue("next_due_ms").optionalSigned(), fields.getValue("awaiting_key").optionalString())
        }
    }
}
class QueueStatus(val pending: UnsignedDecimal, val sending: UnsignedDecimal, val awaitingOutcome: UnsignedDecimal,
    val live: UnsignedDecimal, val blocked: Boolean, commands: List<QueueEntry>) {
    val commands = immutableList(commands)
    override fun toString() = "<QueueStatus>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): QueueStatus {
            val fields = wire.exact("pending", "sending", "awaiting_outcome", "live", "blocked", "commands")
            return QueueStatus(fields.getValue("pending").unsigned(), fields.getValue("sending").unsigned(), fields.getValue("awaiting_outcome").unsigned(),
                fields.getValue("live").unsigned(), fields.getValue("blocked").boolean(), fields.getValue("commands").items().map { QueueEntry.fromProtocolValue(it) })
        }
    }
}
class PushBatch(envelopes: List<String>, val nextDueMs: Long?, val hasMore: Boolean) {
    val envelopes = immutableList(envelopes)
    override fun toString() = "<PushBatch>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): PushBatch {
            val fields = wire.exact("envelopes", "next_due_ms", "has_more")
            return PushBatch(fields.getValue("envelopes").items().map { it.string() }, fields.getValue("next_due_ms").optionalSigned(), fields.getValue("has_more").boolean())
        }
    }
}
enum class SyncPhase(internal val wireName: String) { NotStarted("not_started"), Bootstrapping("bootstrapping"), Ready("ready") }
class SyncProgress(val phase: SyncPhase, val ready: Boolean, val pages: Long, val changes: Long, val tombstones: UnsignedDecimal) {
    override fun toString() = "<SyncProgress>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): SyncProgress {
            val fields = wire.exact("phase", "ready", "pages", "changes", "tombstones")
            val phase = SyncPhase.entries.firstOrNull { it.wireName == fields.getValue("phase").string() } ?: invalidResponse()
            return SyncProgress(phase, fields.getValue("ready").boolean(), fields.getValue("pages").signed(), fields.getValue("changes").signed(), fields.getValue("tombstones").unsigned())
        }
    }
}
