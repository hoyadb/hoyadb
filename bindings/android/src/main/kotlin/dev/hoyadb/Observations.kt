package dev.hoyadb

import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine

interface HoyaTimer { fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable }
internal object SystemHoyaTimer : HoyaTimer {
    private val timer = Executors.newSingleThreadScheduledExecutor { Thread(it, "hoyadb-timer").apply { isDaemon = true } }
    override fun schedule(delayMilliseconds: Long, work: Runnable): AutoCloseable {
        val future = timer.schedule(work, delayMilliseconds.coerceAtLeast(0), TimeUnit.MILLISECONDS)
        return AutoCloseable { future.cancel(false) }
    }
}
enum class ObservationMode { Membership, Columns, Count }
sealed class ObservedValue {
    class Rows(rows: List<Row>) : ObservedValue() { val rows = immutableList(rows) }
    class Count(val count: UnsignedDecimal) : ObservedValue()
    class Failure(val kind: String) : ObservedValue()
    class Terminated(val kind: String) : ObservedValue()
    final override fun toString() = "<ObservedValue>"
}
class Subscription internal constructor(private val cancel: () -> Unit) : AutoCloseable {
    private val live = AtomicBoolean(true)
    override fun close() { if (live.getAndSet(false)) cancel() }
    override fun toString() = "<Subscription>"
}
internal class ManagedObservations(private val database: Database) : AutoCloseable {
    private val target = database.current()
    internal class Listener(val callback: (ProtocolValue) -> Unit, val onTerminated: (String) -> Unit, val unsubscribe: String, val count: Boolean, val owner: String) {
        val live = AtomicBoolean(true)
    }
    private val lock = Any()
    private val listeners = LinkedHashMap<String, Listener>()
    private var draining = false
    private var again = false
    private var deadlineArmed = false
    private var deadline: AutoCloseable? = null
    private var deadlineToken: Any? = null
    // Registration in flight: core may already have queued the new subscription's initial
    // snapshot, so drained events wait here until its listener is installed.
    private var registering = 0
    private val buffered = ArrayList<ProtocolValue>()
    // A drain sampled the clock after the outstanding deadline timer was armed, so core may have
    // stamped a count delivery later than that timer assumes.
    private var drainedSinceArm = false
    // Count listeners with a committed write since their last delivered value.
    private val countDirty = HashSet<String>()
    // A fired deadline whose timer was armed before a later drain awaits that drain's outcome.
    private var deadlineFollowUp = false
    @Volatile var closed = false
    private val wake: () -> Unit = {
        synchronized(lock) { for ((handle, listener) in listeners) if (listener.count && listener.live.get()) countDirty.add(handle) }
        schedule(); armDeadline()
    }
    init { database.registerLifetime(this); synchronized(database.stateLock) { database.mutationListeners.add(wake) } }
    suspend fun register(operation: String, arguments: Map<String, ProtocolValue>, unsubscribe: String, count: Boolean, onTerminated: (String) -> Unit = {}, callback: (ProtocolValue) -> Unit): Subscription {
        synchronized(database.stateLock) { database.drainGuard++ }
        synchronized(lock) { registering++ }
        var committed = false
        try {
            val handle = database.requestOwned(operation, arguments, target.second).unsigned().value
            if (handle == "0") invalidResponse()
            val listener = Listener(callback, onTerminated, unsubscribe, count, target.first)
            synchronized(lock) { if (closed) throw HoyaException("Closed"); listeners[handle] = listener }
            committed = true
            return Subscription { retire(handle, listener) }
        } finally {
            if (!committed) synchronized(database.stateLock) { database.drainGuard-- }
            // The listener is installed (or the attempt ended) before the next drain may start
            // and before events buffered during this registration are delivered.
            synchronized(lock) { registering-- }
            schedule()
        }
    }
    private fun retire(handle: String, listener: Listener, unsubscribe: Boolean = true, reportCleanupFailure: Boolean = false) {
        var cancel: AutoCloseable? = null
        val owned = synchronized(lock) {
            listener.live.set(false)
            if (listeners[handle] !== listener) false
            else {
                listeners.remove(handle); countDirty.remove(handle)
                if (listener.count && listeners.values.none { it.count && it.live.get() }) {
                    cancel = deadline; deadline = null; deadlineArmed = false; deadlineToken = null
                }
                true
            }
        }
        if (!owned) return
        synchronized(database.stateLock) { database.drainGuard-- }
        try { cancel?.close() } catch (error: Throwable) { reportTimer(error) }
        if (unsubscribe) launchManaged({
            try { nativeExchange(database.runtime, listener.unsubscribe, listener.owner, mapOf("subscription" to str(handle))) }
            catch (error: Throwable) {
                if (reportCleanupFailure) database.reportBackground(BackgroundError.Source.Observation, backgroundKind(error), handle, target.second)
            }
        })
    }
    fun schedule() {
        val start = synchronized(lock) {
            if (closed || listeners.isEmpty() || registering != 0) false
            else if (draining) { again = true; false }
            else { draining = true; true }
        }
        if (!start) return
        synchronized(database.stateLock) { database.drainGuard++ }
        launchManaged({ drain() }, { error -> database.reportBackground(BackgroundError.Source.Observation, backgroundKind(error), null, target.second) })
    }
    suspend fun explicitDrain() {
        synchronized(lock) {
            if (closed) throw HoyaException("Closed")
            if (draining || registering != 0) throw HoyaException("Busy")
            draining = true
        }
        synchronized(database.stateLock) { database.drainGuard++ }
        drain()
    }
    private fun armDeadline() {
        val token = Any()
        val arm = synchronized(lock) {
            if (closed || deadlineArmed || listeners.values.none { it.count && it.live.get() }) false
            else { deadlineArmed = true; deadlineToken = token; drainedSinceArm = false; true }
        }
        if (!arm) return
        val scheduled = try {
            database.timer.schedule(16, Runnable {
                val fire = synchronized(lock) {
                    if (closed || deadlineToken !== token) false
                    else {
                        deadlineArmed = false; deadline = null; deadlineToken = null
                        // Core alone decides whether a count is due. A drain sampled after this
                        // timer was armed may have stamped the last delivery later than the timer
                        // assumed; the outcome is settled when the deadline drain completes.
                        if (drainedSinceArm) deadlineFollowUp = true
                        true
                    }
                }
                if (fire) schedule()
            })
        } catch (error: Throwable) {
            val affected = synchronized(lock) {
                if (deadlineToken !== token) emptyList()
                else {
                    deadlineArmed = false; deadline = null; deadlineToken = null
                    listeners.entries.filter { it.value.count }.map { it.key to it.value }
                }
            }
            reportTimer(error)
            for ((handle, listener) in affected) retire(handle, listener)
            return
        }
        val keep = synchronized(lock) { if (closed || deadlineToken !== token) false else { deadline = scheduled; true } }
        if (!keep) try { scheduled.close() } catch (error: Throwable) { reportTimer(error) }
    }
    private fun reportTimer(error: Throwable) = database.reportBackground(BackgroundError.Source.Timer, backgroundKind(error), null, target.second)
    private suspend fun drain() {
        try {
            var more: Boolean
            do {
                val now = database.uptime()
                if (now < 0) throw HoyaException("InvalidRequest")
                synchronized(lock) { drainedSinceArm = true }
                val reply = database.requestOwned("drain", mapOf("now_ms" to decimal(now)), target.second).exact("events", "has_more")
                more = reply.getValue("has_more").boolean()
                val events = synchronized(lock) {
                    buffered.addAll(reply.getValue("events").items())
                    if (registering != 0) null else ArrayList(buffered).also { buffered.clear() }
                } ?: return // The pending registration schedules the delivering drain.
                if (events.isNotEmpty()) deliver(events, target.second)
                if (closed) return
            } while (more || synchronized(lock) { again.also { again = false } })
        } catch (error: Throwable) {
            // Wakes coalesced into this failed attempt do not create a retry loop.
            // A later mutation can still start a new attempt.
            synchronized(lock) { again = false }
            throw error
        } finally { finishDrain() }
    }
    private suspend fun deliver(events: List<ProtocolValue>, generation: Long) = suspendCoroutine<Unit> { continuation ->
        try {
            database.eventExecutor.execute {
                var failure: Throwable? = null
                try {
                    for (event in events) {
                        if (closed) break
                        try { database.checkEpoch(generation) } catch (_: HoyaException) { break }
                        val fields = event.fields()
                        val handle = fields["subscription"]?.unsigned()?.value ?: invalidResponse()
                        val listener = synchronized(lock) { listeners[handle] }
                        if (listener != null && listener.live.get()) {
                            if (fields["kind"]?.string() == "error") {
                                val kind = backgroundKind(fields.getValue("value").fields()["kind"]?.string())
                                // Core has already retired this token; do not unsubscribe.
                                retire(handle, listener, unsubscribe = false)
                                database.reportBackground(BackgroundError.Source.Observation, kind, handle, generation)
                                if (closed) break
                                try { database.checkEpoch(generation) } catch (_: HoyaException) { break }
                                try { listener.onTerminated(kind) }
                                catch (_: Throwable) { database.reportBackground(BackgroundError.Source.Callback, "CallbackFailure", handle, generation) }
                            } else {
                                if (listener.count) synchronized(lock) { countDirty.remove(handle) }
                                try { listener.callback(fields.getValue("value")) }
                                catch (_: Throwable) {
                                    retire(handle, listener, reportCleanupFailure = true)
                                    database.reportBackground(BackgroundError.Source.Callback, "CallbackFailure", handle, generation)
                                }
                            }
                        }
                    }
                } catch (error: Throwable) { failure = error }
                finally {
                    val error = failure
                    if (error == null) continuation.resume(Unit) else continuation.resumeWithException(error)
                }
            }
        } catch (error: Throwable) { close(); continuation.resumeWithException(error) }
    }
    private fun finishDrain() {
        val restart = synchronized(lock) {
            if (!draining) return
            draining = false; val restart = again && !closed; again = false; restart
        }
        synchronized(database.stateLock) { database.drainGuard-- }
        if (restart) { schedule(); return }
        // After the drain that follows a fired deadline: a count listener still without a value
        // for a later write gets one follow-up timer. It is the only outstanding timer, and a
        // timer window without any drain ends the sequence: no polling loop, no second throttle.
        val followUp = synchronized(lock) {
            deadlineFollowUp.also { deadlineFollowUp = false } && listeners.any { it.value.count && it.value.live.get() && it.key in countDirty }
        }
        if (followUp) armDeadline()
    }
    override fun close() {
        val cleanup = synchronized(lock) {
            if (closed) return
            closed = true
            val timer = deadline; deadline = null; deadlineArmed = false; deadlineToken = null
            buffered.clear(); deadlineFollowUp = false
            timer to listeners.entries.map { it.key to it.value }
        }
        try { cleanup.first?.close() } catch (error: Throwable) { reportTimer(error) }
        for ((handle, listener) in cleanup.second) retire(handle, listener)
        synchronized(database.stateLock) { database.mutationListeners.remove(wake) }
        database.unregisterLifetime(this)
    }
}
internal fun Database.observers(): ManagedObservations = synchronized(stateLock) {
    val current = observations
    if (current != null && !current.closed) current else ManagedObservations(this).also { observations = it }
}
suspend fun Database.observe(query: Query, mode: ObservationMode = ObservationMode.Membership, columns: List<String> = emptyList(), callback: (ObservedValue) -> Unit): Subscription =
    observers().register("observe", mapOf("query" to query.toProtocolValue(), "mode" to str(mode.name.lowercase()), "columns" to ProtocolValue.ArrayValue(columns.map(::str))), "unsubscribe", mode == ObservationMode.Count, { callback(ObservedValue.Terminated(it)) }) { wire ->
        val fields = wire.fields()
        val value = when (fields["kind"]?.string()) {
            "rows" -> ObservedValue.Rows(fields.getValue("rows").items().map(Row::fromProtocolValue))
            "count" -> ObservedValue.Count(fields.getValue("count").unsigned())
            "error" -> {
                val kind = fields.getValue("error").exact("kind").getValue("kind").string()
                if (kind !in errorKinds) invalidResponse()
                ObservedValue.Failure(kind)
            }
            else -> invalidResponse()
        }
        callback(value)
    }
