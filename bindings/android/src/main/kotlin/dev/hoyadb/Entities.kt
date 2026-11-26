package dev.hoyadb

import java.util.concurrent.Executor
import java.util.concurrent.atomic.AtomicReference

typealias EntityContextProvider = () -> CapturedContext
typealias MergeHook = suspend (MergeInput) -> Row?
class FieldStamp(val timestampMs: Long, val authoritative: Boolean) { override fun toString() = "<FieldStamp>" }
class RowMetadata(fields: Map<String, FieldStamp>, val tombstone: FieldStamp?) { val fields = immutableMap(fields); override fun toString() = "<RowMetadata>" }
class EntityResolution(val row: Row?, val metadata: RowMetadata) { override fun toString() = "<EntityResolution>" }
class MergeInput(val schema: Schema, val current: Row?, val metadata: RowMetadata, val incoming: EntityChange, val defaultResolution: EntityResolution) { override fun toString() = "<MergeInput>" }
enum class ResetMode { PreserveRecovery, DiscardRecovery }

private suspend fun Database.installEntityProvider(operation: String, config: EntityConfig, provider: EntityContextProvider) {
    val token = current().second
    executeOwned(operation, mapOf("config" to config.toProtocolValue()), token)
    synchronized(stateLock) { checkEpoch(token); entityContext = provider }
}
suspend fun Database.configureEntities(config: EntityConfig, context: EntityContextProvider) = installEntityProvider("entity_configure", config, context)
suspend fun Database.reconfigureEntities(config: EntityConfig, context: EntityContextProvider) = installEntityProvider("entity_reconfigure", config, context)
private suspend fun Database.entityMutation(operation: String, arguments: Map<String, ProtocolValue>): ProtocolValue {
    val token = current().second
    val provider = synchronized(stateLock) { checkEpoch(token); entityContext } ?: throw HoyaException("InvalidCommand")
    val context = try { provider() } catch (_: Exception) { throw HoyaException("InvalidCommand") }
    return executeOwned(operation, arguments + ("context" to context.wire()), token)
}
suspend fun Database.entityBatch(writes: List<Write>): List<Change> = immutableList(entityMutation("entity_batch", mapOf("writes" to ProtocolValue.ArrayValue(writes.map(Write::toProtocolValue)))).items().map(Change::fromProtocolValue))
suspend fun Database.patch(table: String, id: String, fields: Map<String, Cell>): Row = Row.fromProtocolValue(entityMutation("patch", mapOf("table" to str(table), "id" to str(id), "fields" to fieldsWire(fields))))
suspend fun Database.entityLink(table: String, id: String, relation: String, target: String, fields: Map<String, Cell> = emptyMap()): Row = Row.fromProtocolValue(entityMutation("entity_link", mapOf("table" to str(table), "id" to str(id), "relation" to str(relation), "target" to str(target), "fields" to fieldsWire(fields))))
suspend fun Database.entityUnlink(table: String, id: String, relation: String, target: String): UnsignedDecimal = entityMutation("entity_unlink", mapOf("table" to str(table), "id" to str(id), "relation" to str(relation), "target" to str(target))).unsigned()
suspend fun Database.beginPull(): PullRequest = PullRequest.fromProtocolValue(execute("begin_pull"))
suspend fun Database.applyPull(request: PullRequest, page: PullPage) { execute("apply_pull", mapOf("request" to str(request.request.value), "page" to page.toProtocolValue())) }
suspend fun Database.abortPull(request: PullRequest) { execute("abort_pull", mapOf("request" to str(request.request.value))) }
suspend fun Database.syncProgress(): SyncProgress = SyncProgress.fromProtocolValue(execute("sync_progress"))
fun Database.setMergeHooks(hooks: Map<String, MergeHook>) { synchronized(stateLock) { mergeHooks = immutableMap(hooks) } }

internal suspend fun Database.resumeOwned(operation: String, arguments: Map<String, ProtocolValue>, generation: Long): ProtocolValue {
    val waiting = AtomicReference<Completion<Unit>>()
    val subscription = HoyaNativeTransport.subscribeRelease(Executor { it.run() }, Runnable { waiting.get()?.succeed(Unit) })
    val cancellation = AutoCloseable { waiting.get()?.fail(HoyaException("Closed")) }
    registerLifetime(cancellation)
    try {
        while (true) {
            val wake = Completion<Unit>(); waiting.set(wake)
            checkEpoch(generation)
            try { return executeOwned(operation, arguments, generation) }
            catch (error: HoyaException) { if (error.kind != "Busy") throw error; wake.await() }
        }
    } finally { subscription.close(); unregisterLifetime(cancellation) }
}
private fun metadata(wire: ProtocolValue): RowMetadata {
    fun stamp(wire: ProtocolValue): FieldStamp {
        val fields = wire.exact("timestamp_ms", "authoritative")
        return FieldStamp(fields.getValue("timestamp_ms").signed(), fields.getValue("authoritative").boolean())
    }
    val fields = wire.exact("fields", "tombstone")
    return RowMetadata(fields.getValue("fields").fields().mapValues { stamp(it.value) }, fields.getValue("tombstone").let { if (it === ProtocolValue.Null) null else stamp(it) })
}
private fun optionalRow(wire: ProtocolValue): Row? = if (wire === ProtocolValue.Null) null else Row.fromProtocolValue(wire)
suspend fun Database.pullOnce(transport: suspend (String?, String) -> PullPage): SyncProgress {
    val token = current().second
    val hooks = synchronized(stateLock) { mergeHooks }
    val normalizedSchema = if (hooks.isEmpty()) null else schema()
    while (true) {
        checkEpoch(token)
        val pull = PullRequest.fromProtocolValue(resumeOwned("begin_pull", emptyMap(), token))
        try {
            // Actual HTTP/page transport is external; no native lock/permit spans it.
            val page = transport(pull.since, pull.scope)
            checkEpoch(token)
            if (hooks.isEmpty()) resumeOwned("apply_pull", mapOf("request" to str(pull.request.value), "page" to page.toProtocolValue()), token)
            else {
                var step = resumeOwned("merge_prepare", mapOf("request" to str(pull.request.value), "page" to page.toProtocolValue(), "tables" to ProtocolValue.ArrayValue(hooks.keys.sorted().map(::str))), token)
                while (step.fields()["kind"]?.string() == "offer") {
                    val offer = step.exact("session", "step", "kind", "table", "record_id")
                    val session = offer.getValue("session").unsigned().value
                    val index = offer.getValue("step").unsigned().value
                    val hook = hooks[offer.getValue("table").string()] ?: invalidResponse()
                    suspend fun input(part: String) = requestOwned("merge_input", mapOf("session" to str(session), "step" to str(index), "part" to str(part)), token)
                    val current = input("current"); val stamps = input("metadata"); val incoming = input("incoming"); val defaults = input("default").exact("row", "metadata")
                    val proposal = try { hook(MergeInput(normalizedSchema!!, optionalRow(current), metadata(stamps), EntityChange.fromProtocolValue(incoming), EntityResolution(optionalRow(defaults.getValue("row")), metadata(defaults.getValue("metadata"))))) }
                    catch (error: HoyaException) { throw error }
                    catch (_: Exception) { throw HoyaException("InvalidCommand") }
                    checkEpoch(token)
                    val owned = proposal?.toProtocolValue() ?: ProtocolValue.Null
                    step = resumeOwned("merge_reply", mapOf("session" to str(session), "step" to str(index), "row" to owned), token)
                }
                val ready = step.exact("session", "step", "kind")
                if (ready.getValue("kind").string() != "ready") invalidResponse()
                resumeOwned("merge_commit", mapOf("session" to str(ready.getValue("session").unsigned().value)), token)
            }
            return SyncProgress.fromProtocolValue(executeOwned("sync_progress", emptyMap(), token))
        } catch (error: Throwable) {
            try { executeOwned("abort_pull", mapOf("request" to str(pull.request.value)), token) } catch (_: HoyaException) { }
            if (error is HoyaException && error.kind == "StalePull") { checkEpoch(token); continue }
            throw error
        }
    }
}
suspend fun Database.reset(mode: ResetMode = ResetMode.PreserveRecovery) { execute("reset", mapOf("mode" to str(if (mode == ResetMode.PreserveRecovery) "preserve_recovery" else "discard_recovery"))) }
