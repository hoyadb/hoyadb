package dev.hoyadb

import java.security.SecureRandom
import java.util.concurrent.Executor
import java.util.concurrent.atomic.AtomicReference

typealias PushTransport = suspend (String) -> String
typealias Reconciler = suspend (PendingReconciliation) -> ReconcilePlan

class SyncDriver internal constructor(private val database: Database, private val transport: PushTransport,
    private val reconciler: Reconciler?, private val now: () -> Long, private val jitter: () -> Int,
    private val timer: HoyaTimer, private val onError: (HoyaException) -> Unit) : AutoCloseable {
    private val generation = database.current().second
    private val lock = Any()
    private var live = true
    private var running = false
    private var requested = false
    private var deadline: AutoCloseable? = null
    private var deadlineToken: Any? = null
    private val waiting = AtomicReference<Completion<Unit>>()
    private val signal: () -> Unit = { wake() }

    internal fun start() {
        database.registerLifetime(this)
        synchronized(database.stateLock) { database.syncWakeListeners.add(signal) }
        wake()
    }
    private fun checkLive() {
        if (!synchronized(lock) { live }) throw HoyaException("Closed")
        synchronized(database.stateLock) {
            database.checkEpoch(generation)
            if (database.syncDriver !== this) throw HoyaException("Closed")
        }
    }
    private fun isLive(): Boolean = try { checkLive(); true } catch (_: HoyaException) { false }
    private fun error(error: Throwable, source: BackgroundError.Source = BackgroundError.Source.Sync) {
        if (!isLive()) return
        val failure = if (error is HoyaException) error else HoyaException("NativeFailure")
        database.reportBackground(source, backgroundKind(error), null, generation)
        // An inline database sink may close/reset before the adapter starts.
        if (!isLive()) return
        try { onError(failure) } catch (_: Throwable) { /* application error sink cannot own the worker */ }
    }
    override fun close() {
        val cancel = synchronized(lock) {
            if (!live) return
            live = false; requested = false; deadlineToken = null
            deadline.also { deadline = null }
        }
        waiting.get()?.fail(HoyaException("Closed"))
        try { cancel?.close() } catch (error: Throwable) { database.reportBackground(BackgroundError.Source.Timer, backgroundKind(error), null, generation) }
        synchronized(database.stateLock) {
            database.syncWakeListeners.remove(signal)
            if (database.syncDriver === this) database.syncDriver = null
        }
        database.unregisterLifetime(this)
    }
    fun wake() {
        val old: AutoCloseable?
        val start = synchronized(lock) {
            if (!live) return
            requested = true; deadlineToken = null; old = deadline; deadline = null
            if (running) false else { running = true; true }
        }
        try { old?.close() } catch (error: Throwable) { database.reportBackground(BackgroundError.Source.Timer, backgroundKind(error), null, generation) }
        if (start) launchManaged({ run() }, { error(it) })
    }
    private suspend fun owned(operation: String, arguments: Map<String, ProtocolValue>, receipt: Boolean = false): ProtocolValue {
        val wake = AtomicReference<Completion<Unit>>()
        val subscription = HoyaNativeTransport.subscribeRelease(Executor { it.run() }, Runnable { wake.get()?.succeed(Unit) })
        try {
            while (true) {
                val event = Completion<Unit>(); wake.set(event); waiting.set(event)
                checkLive()
                try {
                    val value = database.executeOwned(operation, arguments, generation)
                    checkLive()
                    if (receipt && value.items().any { entry ->
                            val fields = entry.fields()
                            !fields.getValue("ok").boolean() && fields.getValue("error").fields().getValue("kind").string() == "Busy"
                        }) {
                        event.await()
                        continue
                    }
                    return value
                } catch (failure: HoyaException) {
                    if (failure.kind != "Busy") throw failure
                    event.await()
                }
            }
        } finally { waiting.set(null); subscription.close() }
    }
    private suspend fun reconcilePending(conflicted: MutableSet<String>) {
        val mapper = reconciler ?: return
        val pending = pagedRead("pending_reconciliations", "entries") { operation, arguments -> owned(operation, arguments) }.items().map(PendingReconciliation::fromProtocolValue)
        for (entry in pending) {
            checkLive()
            if (entry.key in conflicted) continue
            val plan = mapper(entry)
            checkLive()
            // Serialize one immutable application plan once; only its original
            // owned core operation is retried when its database admission is Busy.
            val arguments = mapOf("key" to str(entry.key), "plan" to plan.toProtocolValue())
            try { owned("reconcile", arguments) }
            catch (failure: HoyaException) {
                // SPEC-0039: a core RollbackConflict leaves the entry pending; later entries still reconcile.
                if (failure.kind != "RollbackConflict") throw failure
                conflicted.add(entry.key); error(failure)
            }
        }
    }
    private suspend fun run() {
        var due: Long? = null
        // SPEC-0039: keys whose core reconcile conflicted are not retried in this pass.
        val conflicted = HashSet<String>()
        try {
            do {
                checkLive()
                synchronized(lock) { requested = false }
                val clock = try { now() } catch (_: Exception) { throw HoyaException("InvalidRequest") }
                val sample = try { jitter() } catch (_: Exception) { throw HoyaException("InvalidRequest") }
                if (sample !in 0..2500) throw HoyaException("InvalidRequest")
                val batch = PushBatch.fromProtocolValue(owned("prepare_push", mapOf("now_ms" to decimal(clock), "jitter_basis_points" to decimal(sample.toLong()))))
                due = batch.nextDueMs
                if (batch.envelopes.isNotEmpty()) {
                    checkLive()
                    // Frozen envelopes are opaque canonical Rust JSON. Joining
                    // this array does not parse or roundtrip their numbers.
                    val response = transport(batch.envelopes.joinToString(",", "[", "]"))
                    checkLive()
                    val receipts = owned("receive_wire", mapOf("body" to str(response)), receipt = true)
                    for (entry in receipts.items()) {
                        val fields = entry.fields()
                        if (!fields.getValue("ok").boolean()) {
                            val kind = fields.getValue("error").exact("kind").getValue("kind").string()
                            error(HoyaException(if (kind in errorKinds) kind else "InvalidResponse"))
                        }
                    }
                }
                // Also process externally delivered receipts when there was no
                // outgoing batch; a sync wake must not require another send.
                reconcilePending(conflicted)
                if (!batch.hasMore && !synchronized(lock) { requested }) break
                // A requested wake starts an independently scheduled pass; outgoing pages do not.
                if (!batch.hasMore) conflicted.clear()
            } while (isLive())
        } catch (failure: Throwable) {
            if (failure is HoyaException && failure.kind == "Closed") close() else error(failure)
        } finally {
            val again = synchronized(lock) { running = false; live && requested }
            if (again) wake() else if (isLive() && due != null) arm(due)
        }
    }
    private fun arm(due: Long) {
        val token = Any()
        val shouldArm = synchronized(lock) {
            if (!live || running || requested) false else { deadlineToken = token; true }
        }
        if (!shouldArm) return
        try {
            checkLive()
            val clock = now()
            val delay = if (due <= clock) 0L else try { Math.subtractExact(due, clock) } catch (_: ArithmeticException) { Long.MAX_VALUE }
            val handle = timer.schedule(delay, Runnable {
                val fire = synchronized(lock) {
                    if (!live || deadlineToken !== token) false else { deadlineToken = null; deadline = null; true }
                }
                if (fire) wake()
            })
            val keep = synchronized(lock) {
                if (live && deadlineToken === token) { deadline = handle; true } else false
            }
            if (!keep) handle.close()
        } catch (failure: Throwable) {
            synchronized(lock) { if (deadlineToken === token) deadlineToken = null }
            error(failure, BackgroundError.Source.Timer)
        }
    }
    override fun toString() = "<SyncDriver>"
}

fun Database.startSync(transport: PushTransport, reconciler: Reconciler? = null,
    now: () -> Long = { System.currentTimeMillis() }, jitter: () -> Int = { SecureRandom().nextInt(2501) },
    timer: HoyaTimer = this.timer, onError: (HoyaException) -> Unit = {}): SyncDriver {
    val driver = SyncDriver(this, transport, reconciler, now, jitter, timer, onError)
    val previous = synchronized(stateLock) { syncDriver.also { syncDriver = driver } }
    previous?.close()
    driver.start()
    return driver
}
