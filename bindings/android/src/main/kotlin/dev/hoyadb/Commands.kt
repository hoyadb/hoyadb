package dev.hoyadb

class CapturedContext(val identity: JSONValue, val nowMilliseconds: Long) {
    internal fun wire() = obj("identity" to identity.toProtocolValue(), "now_ms" to decimal(nowMilliseconds))
    override fun toString() = "<CapturedContext>"
}
class CommandDefinition(val name: String, arguments: Map<String, Argument> = emptyMap(), referencePaths: List<List<PathPart>> = emptyList()) {
    enum class Kind { Any, String, Integer, Float, Boolean, Array, Object }
    data class Argument(val kind: Kind, val optional: Boolean = false)
    sealed class PathPart { data class Key(val key: String) : PathPart(); data class Index(val index: UnsignedDecimal) : PathPart() }
    val arguments = immutableMap(arguments)
    val referencePaths = immutableList(referencePaths.map(::immutableList))
    internal fun wire() = obj("name" to str(name), "arguments" to ProtocolValue.ObjectValue(arguments.mapValues { obj("kind" to str(it.value.kind.name), "optional" to ProtocolValue.BooleanValue(it.value.optional)) }),
        "reference_paths" to ProtocolValue.ArrayValue(referencePaths.map { ProtocolValue.ArrayValue(it.map { part -> when (part) { is PathPart.Key -> obj("key" to str(part.key)); is PathPart.Index -> obj("index" to str(part.index.value)) } }) }))
    override fun toString() = "<CommandDefinition>"
}
class CommandResult(val key: String, val value: JSONValue) { override fun toString() = "<CommandResult>" }
class CommandTask internal constructor(
    private val database: Database, private val definition: CommandDefinition, private val arguments: JSONValue,
    private val captured: CapturedContext, private val handler: suspend (CommandScope) -> JSONValue
) : AutoCloseable {
    private enum class Phase { Queued, Active, Finishing, Done }
    private val lock = Any()
    private var phase = Phase.Queued
    private val result = Completion<CommandResult>()
    private val target = database.current()
    private var ticket: String? = null
    private var wake: Completion<Unit>? = null
    private var cancelled = false
    private var subscription: AutoCloseable? = null
    internal fun start() {
        // Registration precedes even request allocation; notifications never admit.
        subscription = HoyaNativeTransport.subscribeRelease(java.util.concurrent.Executor { it.run() }, Runnable {
            val event = synchronized(lock) { wake }; event?.succeed(Unit)
        })
        database.registerLifetime(this)
        launchManaged({ run() }, { fail(it) })
    }
    suspend fun await(): CommandResult = result.await()
    fun cancel() = close()
    override fun close() {
        val cleanup = synchronized(lock) {
            if (phase == Phase.Done || phase == Phase.Finishing) return
            cancelled = true; val active = phase == Phase.Active; phase = Phase.Done
            ticket?.let { it to active }
        }
        synchronized(lock) { wake }?.succeed(Unit)
        result.fail(HoyaException("Closed")); subscription?.close(); database.unregisterLifetime(this)
        if (cleanup != null) launchManaged({ cleanup(cleanup.first, cleanup.second) })
    }
    internal fun requireActive(): String {
        val value = synchronized(lock) { if (cancelled || phase != Phase.Active) throw HoyaException("Closed"); ticket ?: throw HoyaException("Closed") }
        database.checkEpoch(target.second); return value
    }
    private suspend fun cleanup(request: String, active: Boolean) {
        try { nativeExchange(database.runtime, if (active) "abort" else "cancel_request", target.first, mapOf("request" to str(request))) } catch (_: HoyaException) { /* owner may already be retired */ }
    }
    internal suspend fun scoped(operation: String, args: Map<String, ProtocolValue>): ProtocolValue {
        val request = requireActive()
        val value = nativeExchange(database.runtime, operation, target.first, args + ("request" to str(request)))
        requireActive(); return value
    }
    private suspend fun run() {
        var request: String? = null; var admitted = false; var finished = false
        try {
            request = nativeExchange(database.runtime, "request", target.first, emptyMap()).unsigned().value
            synchronized(lock) { ticket = request }
            while (true) {
                val event = Completion<Unit>()
                synchronized(lock) { if (cancelled) throw HoyaException("Closed"); wake = event }
                database.checkEpoch(target.second)
                val ready = nativeExchange(database.runtime, "begin", target.first, mapOf("request" to str(request), "definition" to definition.wire(), "value" to arguments.toProtocolValue(), "context" to captured.wire())).boolean()
                if (ready) {
                    admitted = true
                    synchronized(lock) { if (cancelled) throw HoyaException("Closed"); phase = Phase.Active; wake = null }
                    break
                }
                event.await()
            }
            val value = handler(CommandScope(this, database))
            synchronized(lock) { if (cancelled || phase != Phase.Active) throw HoyaException("Closed"); phase = Phase.Finishing }
            val wire = nativeExchange(database.runtime, "finish", target.first, mapOf("request" to str(request), "result" to value.toProtocolValue())).exact("key", "result")
            finished = true
            result.succeed(CommandResult(wire.getValue("key").string(), JSONValue.fromProtocolValue(wire.getValue("result"))))
            database.mutated(generation = target.second)
        } catch (error: Throwable) {
            if (request != null && !finished) cleanup(request, admitted)
            fail(error)
        } finally {
            synchronized(lock) { phase = Phase.Done; wake = null }
            subscription?.close(); database.unregisterLifetime(this)
        }
    }
    private fun fail(error: Throwable) { result.fail(if (error is HoyaException) error else HoyaException("InvalidCommand")) }
    override fun toString() = "<CommandTask>"
}
class CommandScope internal constructor(private val task: CommandTask, private val database: Database) {
    suspend fun arguments(): JSONValue = JSONValue.fromProtocolValue(task.scoped("scope_args", emptyMap()))
    suspend fun context(): CapturedContext {
        val value = task.scoped("scope_context", emptyMap()).exact("identity", "now_ms")
        return CapturedContext(JSONValue.fromProtocolValue(value.getValue("identity")), value.getValue("now_ms").signed())
    }
    suspend fun insert(table: String, fields: Map<String, Cell>, proposedId: String? = null): Row = Row.fromProtocolValue(task.scoped("scope_insert", mapOf("table" to str(table), "fields" to fieldsWire(fields), "proposed_id" to database.proposed(proposedId))))
    suspend fun replace(table: String, id: String, fields: Map<String, Cell>): Row = Row.fromProtocolValue(task.scoped("scope_replace", mapOf("table" to str(table), "id" to str(id), "fields" to fieldsWire(fields))))
    suspend fun remove(table: String, id: String): Boolean = task.scoped("scope_remove", mapOf("table" to str(table), "id" to str(id))).boolean()
    suspend fun fetch(query: Query): List<Row> = immutableList(task.scoped("scope_fetch", mapOf("query" to query.toProtocolValue())).items().map(Row::fromProtocolValue))
    suspend fun batch(writes: List<Write>): List<Change> = immutableList(task.scoped("scope_batch", mapOf("writes" to database.writesWire(writes))).items().map(Change::fromProtocolValue))
    suspend fun link(table: String, id: String, relation: String, target: String, fields: Map<String, Cell> = emptyMap(), proposedId: String? = null): Row = Row.fromProtocolValue(task.scoped("scope_link", mapOf("table" to str(table), "id" to str(id), "relation" to str(relation), "target" to str(target), "fields" to fieldsWire(fields), "proposed_id" to database.proposed(proposedId))))
    suspend fun unlink(table: String, id: String, relation: String, target: String): UnsignedDecimal = task.scoped("scope_unlink", mapOf("table" to str(table), "id" to str(id), "relation" to str(relation), "target" to str(target))).unsigned()
    override fun toString() = "<CommandScope>"
}
fun Database.startCommand(definition: CommandDefinition, arguments: JSONValue, context: CapturedContext, handler: suspend (CommandScope) -> JSONValue): CommandTask = CommandTask(this, definition, arguments, context, handler).also { it.start() }
suspend fun Database.command(definition: CommandDefinition, arguments: JSONValue, context: CapturedContext, handler: suspend (CommandScope) -> JSONValue): CommandResult = startCommand(definition, arguments, context, handler).await()
