package dev.hoyadb

sealed class CommandOutcome {
    object Queued : CommandOutcome(); object Sent : CommandOutcome(); object Acknowledged : CommandOutcome()
    class Applied(val result: JSONValue) : CommandOutcome(); class Corrected(val result: JSONValue) : CommandOutcome(); class Rejected(val reason: String) : CommandOutcome()
    final override fun toString() = "<CommandOutcome>"
}
/**
 * API-0011 §4 paged reads. The unpaged call stays the fast path; only when its complete reply
 * exceeds one frame are the core-owned prefixes followed by key cursor. Core orders and bounds
 * every page; this loop only concatenates them into the unpaged shape.
 */
internal suspend fun pagedRead(operation: String, list: String, call: suspend (String, Map<String, ProtocolValue>) -> ProtocolValue): ProtocolValue {
    val whole = try { return call(operation, emptyMap()) } catch (error: HoyaException) { if (error.kind != "LimitExceeded") throw error; error }
    var attempt = 0
    while (true) {
        val items = ArrayList<ProtocolValue>()
        var after: ProtocolValue = ProtocolValue.Null
        var first: Map<String, ProtocolValue>? = null
        try {
            do {
                val page = call(operation + "_page", mapOf("after" to after)).fields()
                val entries = (page[list] ?: invalidResponse()).items()
                val more = (page["has_more"] ?: invalidResponse()).boolean()
                if (entries.isEmpty() && more) invalidResponse()
                if (first == null) first = page
                items.addAll(entries)
                if (entries.isNotEmpty()) after = str((entries.last().fields()["key"] ?: invalidResponse()).string())
            } while (more)
        } catch (error: HoyaException) {
            // An older native library has no paged operation: keep the original failure.
            if (error.kind == "InvalidRequest" && first == null) throw whole
            // The cursor's entry settled between pages: restart from the first prefix.
            if (error.kind != "Protocol" || after === ProtocolValue.Null || attempt >= 2) throw error
            attempt++
            continue
        }
        if (list == "entries") return ProtocolValue.ArrayValue(items)
        return ProtocolValue.ObjectValue(first!! - "has_more" + (list to ProtocolValue.ArrayValue(items)))
    }
}
suspend fun Database.queueStatus(): QueueStatus = QueueStatus.fromProtocolValue(pagedRead("queue_status", "commands") { operation, arguments -> execute(operation, arguments) })
suspend fun Database.preparePush(nowMilliseconds: Long, jitterBasisPoints: Int): PushBatch {
    if (jitterBasisPoints !in 0..65535) throw HoyaException("InvalidRequest")
    return PushBatch.fromProtocolValue(execute("prepare_push", mapOf("now_ms" to decimal(nowMilliseconds), "jitter_basis_points" to decimal(jitterBasisPoints.toLong()))))
}
suspend fun Database.receive(key: String, outcome: RemoteOutcome?) { execute("receive", mapOf("key" to str(key), "outcome" to (outcome?.toProtocolValue() ?: ProtocolValue.Null))) }
suspend fun Database.receiveEntity(table: String, recordId: String, writeId: String, outcome: RemoteOutcome?) { execute("receive_entity", mapOf("table" to str(table), "record_id" to str(recordId), "write_id" to str(writeId), "outcome" to (outcome?.toProtocolValue() ?: ProtocolValue.Null))) }
suspend fun Database.receiveWire(body: String): ProtocolValue = execute("receive_wire", mapOf("body" to str(body)))
// SPEC-0038: one explicit rearm with the caller's clock sample; a started managed driver is woken.
suspend fun Database.requestRedelivery(key: String, nowMilliseconds: Long) { execute("request_redelivery", mapOf("key" to str(key), "now_ms" to decimal(nowMilliseconds))) }
suspend fun Database.pendingReconciliations(): List<PendingReconciliation> = immutableList(pagedRead("pending_reconciliations", "entries") { operation, arguments -> execute(operation, arguments) }.items().map(PendingReconciliation::fromProtocolValue))
suspend fun Database.reconcile(key: String, plan: ReconcilePlan) { execute("reconcile", mapOf("key" to str(key), "plan" to plan.toProtocolValue())) }
// SPEC-0039: one reconcile_guarded with the caller's exact expectations; core errors propagate unchanged.
suspend fun Database.reconcileGuarded(key: String, plan: ReconcilePlan, guards: List<CorrectionGuard>) {
    execute("reconcile_guarded", mapOf("key" to str(key), "plan" to plan.toProtocolValue(), "guards" to ProtocolValue.ArrayValue(guards.map { it.toProtocolValue() })))
}
suspend fun Database.rollback(key: String) { execute("rollback", mapOf("key" to str(key))) }
suspend fun Database.cancel(key: String) { execute("cancel", mapOf("key" to str(key))) }
suspend fun Database.forget(key: String) { execute("forget", mapOf("key" to str(key))) }
suspend fun Database.observeStatus(callback: (QueueStatus) -> Unit): Subscription = observeStatus(callback, {})
suspend fun Database.observeStatus(callback: (QueueStatus) -> Unit, onTerminated: (String) -> Unit): Subscription = observers().register("observe_status", emptyMap(), "unsubscribe_sync", false, onTerminated) { callback(QueueStatus.fromProtocolValue(it)) }
suspend fun Database.observeOutcome(key: String, callback: (CommandOutcome) -> Unit): Subscription = observeOutcome(key, callback, {})
suspend fun Database.observeOutcome(key: String, callback: (CommandOutcome) -> Unit, onTerminated: (String) -> Unit): Subscription = observers().register("observe_outcome", mapOf("key" to str(key)), "unsubscribe_sync", false, onTerminated) { wire ->
    val value = wire.fields()
    val outcome = when (value["state"]?.string()) {
        "queued" -> { wire.exact("state"); CommandOutcome.Queued }
        "sent" -> { wire.exact("state"); CommandOutcome.Sent }
        "acknowledged" -> { wire.exact("state"); CommandOutcome.Acknowledged }
        "applied" -> { wire.exact("state", "result"); CommandOutcome.Applied(JSONValue.fromProtocolValue(value.getValue("result"))) }
        "corrected" -> { wire.exact("state", "result"); CommandOutcome.Corrected(JSONValue.fromProtocolValue(value.getValue("result"))) }
        "rejected" -> { wire.exact("state", "reason"); CommandOutcome.Rejected(value.getValue("reason").string()) }
        else -> invalidResponse()
    }
    callback(outcome)
}
suspend fun Database.observeProgress(callback: (SyncProgress) -> Unit): Subscription = observeProgress(callback, {})
suspend fun Database.observeProgress(callback: (SyncProgress) -> Unit, onTerminated: (String) -> Unit): Subscription = observers().register("observe_progress", emptyMap(), "unsubscribe_sync", false, onTerminated) { callback(SyncProgress.fromProtocolValue(it)) }
suspend fun JSONValue.Companion.decodeJSON(body: String): JSONValue = JSONValue.fromProtocolValue(nativeExchange(NativeRuntime, "json_decode", null, mapOf("body" to str(body))))
suspend fun JSONValue.encodeJSON(): String = nativeExchange(NativeRuntime, "json_encode", null, mapOf("value" to toProtocolValue())).string()
class DiagnosticSnapshot(val schemaVersion: Long, val pending: UnsignedDecimal, val sending: UnsignedDecimal, val awaitingOutcome: UnsignedDecimal,
    val live: UnsignedDecimal, val blocked: Boolean, val ready: Boolean, val pages: Long, val changes: Long, val tombstones: UnsignedDecimal) {
    override fun toString() = "<DiagnosticSnapshot>"
}
suspend fun Database.diagnostics(): DiagnosticSnapshot {
    val value = execute("diagnostics").exact("schema_version", "pending", "sending", "awaiting_outcome", "live", "blocked", "ready", "pages", "changes", "tombstones")
    return DiagnosticSnapshot(value.getValue("schema_version").u32(), value.getValue("pending").unsigned(), value.getValue("sending").unsigned(), value.getValue("awaiting_outcome").unsigned(), value.getValue("live").unsigned(), value.getValue("blocked").boolean(), value.getValue("ready").boolean(), value.getValue("pages").signed(), value.getValue("changes").signed(), value.getValue("tombstones").unsigned())
}
