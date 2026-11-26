// API-0011: one owned Rust facade; no database or admission algorithm in Kotlin.
package dev.hoyadb

import android.content.Context
import java.security.SecureRandom
import java.util.concurrent.Executor

class DatabaseOptions(val capacity: UnsignedDecimal = UnsignedDecimal.fromLong(1000), val history: UnsignedDecimal = UnsignedDecimal.fromLong(100),
    val initialDelayMs: Long = 1000, val multiplier: Long = 2, val maxDelayMs: Long = 60000) {
    internal fun wire(): ProtocolValue {
        if (multiplier !in 0..4294967295L) throw HoyaException("InvalidRequest")
        return obj("capacity" to str(capacity.value), "history" to str(history.value), "initial_delay_ms" to decimal(initialDelayMs), "multiplier" to decimal(multiplier), "max_delay_ms" to decimal(maxDelayMs))
    }
    override fun toString() = "<DatabaseOptions>"
}
class Protection(val encrypted: Boolean, val cipher: String, val version: String, val backup: String) {
    override fun toString() = "<Protection>"
}
internal interface NativeCalling {
    fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply)
    fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply)
}
internal object NativeRuntime : NativeCalling {
    override fun open(frame: ByteArray, key: ByteArray, reply: HoyaNativeTransport.Reply) = HoyaNativeTransport.open(frame, key, reply)
    override fun call(frame: ByteArray, reply: HoyaNativeTransport.Reply) = HoyaNativeTransport.call(frame, reply)
}
class MemoryStore internal constructor(internal val handle: UnsignedDecimal) {
    companion object {
        suspend fun create(): MemoryStore = MemoryStore(nativeExchange(NativeRuntime, "memory_store", null, emptyMap()).unsigned())
    }
    suspend fun drop() { nativeExchange(NativeRuntime, "memory_drop", null, mapOf("store" to str(handle.value))) }
    override fun toString() = "<MemoryStore>"
}
class Database internal constructor(internal val runtime: NativeCalling, val eventExecutor: Executor, internal val idGenerator: (() -> String?)?, val options: DatabaseOptions, internal val uptime: () -> Long, internal val timer: HoyaTimer, internal val onBackgroundError: ((BackgroundError) -> Unit)?, internal val conflict: ConflictPolicy?, internal val onResolution: ((String, Resolution) -> Unit)?) {
    internal constructor(runtime: NativeCalling, eventExecutor: Executor, idGenerator: (() -> String?)?, options: DatabaseOptions, uptime: () -> Long = { android.os.SystemClock.uptimeMillis() }, timer: HoyaTimer = SystemHoyaTimer, onBackgroundError: ((BackgroundError) -> Unit)? = null) : this(runtime, eventExecutor, idGenerator, options, uptime, timer, onBackgroundError, null, null)
    constructor(eventExecutor: Executor, idGenerator: (() -> String?)? = null, options: DatabaseOptions = DatabaseOptions(), uptime: () -> Long = { android.os.SystemClock.uptimeMillis() }, timer: HoyaTimer = SystemHoyaTimer, onBackgroundError: ((BackgroundError) -> Unit)? = null) : this(NativeRuntime, eventExecutor, idGenerator, options, uptime, timer, onBackgroundError)
    constructor(eventExecutor: Executor, idGenerator: (() -> String?)? = null, options: DatabaseOptions = DatabaseOptions(), uptime: () -> Long = { android.os.SystemClock.uptimeMillis() }, timer: HoyaTimer = SystemHoyaTimer, onBackgroundError: ((BackgroundError) -> Unit)? = null, conflict: ConflictPolicy? = null, onResolution: ((String, Resolution) -> Unit)? = null) : this(NativeRuntime, eventExecutor, idGenerator, options, uptime, timer, onBackgroundError, conflict, onResolution)
    internal val stateLock = Any()
    internal var owner: String? = null
    internal var epoch: Long = 0
    private var opening: Completion<Unit>? = null
    private var closing: Completion<Unit>? = null
    private var retired: String? = null
    internal val lifecycle = LinkedHashSet<AutoCloseable>()
    internal val mutationListeners = LinkedHashSet<() -> Unit>()
    internal fun registerLifetime(value: AutoCloseable) { synchronized(stateLock) { lifecycle.add(value) } }
    internal fun unregisterLifetime(value: AutoCloseable) { synchronized(stateLock) { lifecycle.remove(value) } }
    private fun invalidateManaged() {
        val values = synchronized(stateLock) { entityContext = null; mergeHooks = emptyMap(); lifecycle.toList().also { lifecycle.clear() } }
        for (value in values) value.close()
    }
    internal fun mutated(operation: String = "finish", generation: Long? = null) {
        val targets = synchronized(stateLock) {
            if (generation != null && (owner == null || epoch != generation)) return
            mutationListeners.toList() to if (operation == "prepare_push") emptyList() else syncWakeListeners.toList()
        }
        val listeners = targets.first
        for (listener in listeners) listener()
        if (operation != "prepare_push") {
            val syncing = targets.second
            for (listener in syncing) listener()
        }
    }
    internal val syncWakeListeners = LinkedHashSet<() -> Unit>()
    internal fun proposed(explicit: String? = null): ProtocolValue {
        if (explicit != null) return str(explicit)
        val generator = idGenerator ?: return ProtocolValue.Null
        val value = try { generator() } catch (_: Exception) { throw HoyaException("InvalidRequest") }
        if (value.isNullOrEmpty()) throw HoyaException("InvalidRequest")
        return str(value)
    }
    internal fun writesWire(writes: List<Write>): ProtocolValue = ProtocolValue.ArrayValue(writes.map { write ->
        if (write is Write.Insert) ProtocolValue.ObjectValue(write.toProtocolValue().fields() + ("proposed_id" to proposed(write.proposedId))) else write.toProtocolValue()
    })
    internal fun current(): Pair<String, Long> = synchronized(stateLock) { (owner ?: throw HoyaException("Closed")) to epoch }
    internal fun checkEpoch(value: Long) { synchronized(stateLock) { if (owner == null || epoch != value) throw HoyaException("Closed") } }
    private fun reserve(): Pair<Completion<Unit>, Long> = synchronized(stateLock) {
        if (owner != null || opening != null || closing != null || retired != null) throw HoyaException("Busy")
        if (epoch == Long.MAX_VALUE) throw HoyaException("LimitExceeded")
        epoch++; val attempt = Completion<Unit>(); opening = attempt; attempt to epoch
    }
    private fun namespace(value: String?): String {
        if (value != null) return value
        val bytes = ByteArray(16)
        try { SecureRandom().nextBytes(bytes) } catch (_: Exception) { throw HoyaException("NativeFailure") }
        return bytes.joinToString("") { "%02x".format(it.toInt() and 255) }.also { bytes.fill(0) }
    }
    suspend fun openMemory(schema: Schema, namespace: String? = null, store: MemoryStore? = null, migrations: List<Migration> = emptyList()) {
        val reservation = reserve()
        beginOpen(reservation, obj("kind" to str("memory"), "store" to (store?.let { str(it.handle.value) } ?: ProtocolValue.Null)), ByteArray(0), schema, namespace, migrations)
    }
    suspend fun openProtected(context: Context, name: String, key: ByteArray, schema: Schema, namespace: String? = null, backupExcluded: Boolean = true, migrations: List<Migration> = emptyList()) {
        // Android getters can create directories: validate BEFORE evaluating them.
        if (key.size != 32 || name.isEmpty() || name == "." || name == ".." || name.any { it == '/' || it == '\\' || it == '\u0000' }) throw HoyaException("InvalidRequest")
        val reservation = reserve()
        val ownedKey = key.copyOf()
        var policy: BackupPolicyLock.Held? = null
        try {
            // SPEC-0036 item 6: the policy is fixed by location. Both locks are held from the
            // opposite-parent check until the key-bearing open replies; a rejected open creates
            // only the lock file and its noBackupFilesDir child, never a store or its directory.
            policy = BackupPolicyLock.acquire(context)
            BackupPolicyLock.requireNoOppositeStore(context, name, backupExcluded)
            // The OS-provided app root may be reached through a platform alias;
            // resolve it once, then require the owned child to be canonical.
            val directory = BackupPolicyLock.child((if (backupExcluded) context.noBackupFilesDir else context.filesDir).canonicalFile)
            beginOpen(reservation, obj("kind" to str("native"), "directory" to str(directory.path), "name" to str(name), "backup_excluded" to ProtocolValue.BooleanValue(backupExcluded)), ownedKey, schema, namespace, migrations)
        } catch (error: HoyaException) { releaseReservation(reservation.first); throw error }
        catch (_: Exception) { releaseReservation(reservation.first); throw HoyaException("InvalidRequest") }
        finally { ownedKey.fill(0); policy?.close() }
    }
    private fun releaseReservation(attempt: Completion<Unit>) {
        synchronized(stateLock) { if (opening === attempt) opening = null }
        attempt.succeed(Unit)
    }
    private suspend fun beginOpen(reservation: Pair<Completion<Unit>, Long>, engine: ProtocolValue, key: ByteArray, schema: Schema, namespace: String?, migrations: List<Migration>) {
        val (attempt, token) = reservation
        var acquired: String? = null
        var published = false
        try {
            val args = mapOf("engine" to engine, "schema" to schema.toProtocolValue(), "migrations" to ProtocolValue.ArrayValue(migrations.map { it.toProtocolValue() }), "namespace" to str(namespace(namespace)), "options" to options.wire())
            val handle = nativeExchange(runtime, "open", null, args, key).unsigned().value
            if (handle == "0") invalidResponse()
            acquired = handle
            synchronized(stateLock) { if (closing != null || epoch != token || opening !== attempt) throw HoyaException("Closed") }
            // The owner is unavailable until its ephemeral constructor policy is installed.
            conflict?.let { nativeExchange(runtime, "set_conflict_policy", handle, mapOf("policy" to it.toProtocolValue())) }
            synchronized(stateLock) {
                if (closing != null || epoch != token || opening !== attempt) throw HoyaException("Closed")
                owner = handle; published = true
            }
        } catch (error: Throwable) {
            if (!published) acquired?.let { handle ->
                // A concurrent close waits for this reservation. It cannot also dispose
                // the unpublished handle while configuration is still in flight.
                try { nativeExchange(runtime, "close", handle, emptyMap()) }
                catch (_: Throwable) { synchronized(stateLock) { retired = handle } }
            }
            throw error
        } finally { releaseReservation(attempt) }
    }
    internal suspend fun request(operation: String, arguments: Map<String, ProtocolValue> = emptyMap()): ProtocolValue {
        return requestOwned(operation, arguments, current().second)
    }
    internal suspend fun requestOwned(operation: String, arguments: Map<String, ProtocolValue>, token: Long): ProtocolValue {
        val handle = synchronized(stateLock) { checkEpoch(token); owner!! }
        val value = nativeExchange(runtime, operation, handle, arguments)
        checkEpoch(token)
        return value
    }
    suspend fun execute(operation: String, arguments: Map<String, ProtocolValue> = emptyMap()): ProtocolValue {
        if (operation == "open") throw HoyaException("InvalidRequest")
        if (operation == "close") { close(); return ProtocolValue.Null }
        return executeOwned(operation, arguments, current().second)
    }
    internal suspend fun executeOwned(operation: String, arguments: Map<String, ProtocolValue>, generation: Long): ProtocolValue {
        if (operation == "open" || operation == "close") throw HoyaException("InvalidRequest")
        if (operation == "drain" && managedDrainBusy()) throw HoyaException("Busy")
        // Report-returning facade operations cover typed, advanced and managed calls.
        // Malformed legacy arguments retain their original exact-shape rejection.
        val reportingOperation = when {
            operation == "reconcile" && arguments.keys == setOf("key", "plan") -> "reconcile_resolving"
            operation == "reconcile_guarded" -> "reconcile_resolving"
            operation == "rollback" -> "rollback_resolving"
            else -> operation
        }
        val reportingArguments = if (operation == "reconcile" && reportingOperation == "reconcile_resolving") arguments + ("guards" to ProtocolValue.ArrayValue(emptyList())) else arguments
        val value = requestOwned(reportingOperation, reportingArguments, generation)
        if (reportingOperation in resolutionOperations) {
            notifyResolution(arguments.getValue("key").string(), Resolution.fromProtocolValue(value), generation)
        }
        if (operation == "reset") {
            synchronized(stateLock) { checkEpoch(generation); if (epoch == Long.MAX_VALUE) throw HoyaException("LimitExceeded"); epoch++ }
            invalidateManaged()
        }
        if (operation in mutationOperations) mutated(operation, if (operation == "reset") generation + 1 else generation)
        return if (operation in unitResolutionOperations) ProtocolValue.Null else value
    }
    internal fun managedDrainBusy(): Boolean = synchronized(stateLock) { drainGuard > 0 }
    internal var drainGuard = 0
    internal var observations: ManagedObservations? = null
    internal var syncDriver: SyncDriver? = null
    internal var entityContext: EntityContextProvider? = null
    internal var mergeHooks: Map<String, MergeHook> = emptyMap()
    suspend fun close() {
        var leader = false; var pendingOpen: Completion<Unit>? = null
        val done = synchronized(stateLock) {
            closing ?: run {
                if (owner == null && opening == null && retired == null) return
                if (epoch == Long.MAX_VALUE) throw HoyaException("LimitExceeded")
                epoch++; retired = owner ?: retired; owner = null
                pendingOpen = opening; leader = true
                Completion<Unit>().also { closing = it }
            }
        }
        if (!leader) { done.await(); return }
        invalidateManaged()
        try {
            pendingOpen?.await()
            val handle = synchronized(stateLock) { retired }
            if (handle != null) nativeExchange(runtime, "close", handle, emptyMap())
            synchronized(stateLock) { retired = null; closing = null }
            done.succeed(Unit)
        } catch (error: Throwable) {
            synchronized(stateLock) { if (closing === done) closing = null }
            done.fail(error); throw error
        }
    }
    suspend fun protection(): Protection {
        val value = request("protection").exact("encrypted", "cipher", "version", "backup")
        return Protection(value.getValue("encrypted").boolean(), value.getValue("cipher").string(), value.getValue("version").string(), value.getValue("backup").string())
    }
    suspend fun insert(table: String, fields: Map<String, Cell>, proposedId: String? = null): Row = Row.fromProtocolValue(execute("insert", mapOf("table" to str(table), "fields" to fieldsWire(fields), "proposed_id" to proposed(proposedId))))
    suspend fun replace(table: String, id: String, fields: Map<String, Cell>): Row = Row.fromProtocolValue(execute("replace", mapOf("table" to str(table), "id" to str(id), "fields" to fieldsWire(fields))))
    suspend fun remove(table: String, id: String): Boolean = execute("remove", mapOf("table" to str(table), "id" to str(id))).boolean()
    suspend fun batch(writes: List<Write>): List<Change> = immutableList(execute("batch", mapOf("writes" to writesWire(writes))).items().map(Change::fromProtocolValue))
    suspend fun link(table: String, id: String, relation: String, target: String, fields: Map<String, Cell> = emptyMap(), proposedId: String? = null): Row = Row.fromProtocolValue(execute("link", mapOf("table" to str(table), "id" to str(id), "relation" to str(relation), "target" to str(target), "fields" to fieldsWire(fields), "proposed_id" to proposed(proposedId))))
    suspend fun unlink(table: String, id: String, relation: String, target: String): UnsignedDecimal = execute("unlink", mapOf("table" to str(table), "id" to str(id), "relation" to str(relation), "target" to str(target))).unsigned()
    suspend fun schema(): Schema = Schema.fromProtocolValue(request("schema"))
    suspend fun fetch(query: Query): List<Row> = immutableList(request("fetch", mapOf("query" to query.toProtocolValue())).items().map { Row.fromProtocolValue(it) })
    override fun toString() = "<Database>"
}

internal val mutationOperations = setOf("prepare_push", "insert", "replace", "remove", "batch", "link", "unlink", "finish", "refresh", "receive", "receive_entity", "receive_wire", "request_redelivery", "reconcile", "reconcile_guarded", "rollback", "reconcile_resolving", "rollback_resolving", "discard", "cancel", "forget", "entity_configure", "entity_reconfigure", "entity_batch", "patch", "entity_link", "entity_unlink", "apply_pull", "merge_commit", "reset")
