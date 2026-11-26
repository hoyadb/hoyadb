package dev.hoyadb.testing

import dev.hoyadb.*
import java.util.concurrent.Executor

// d44d9bb: capability bootstrap is explicit and precedes hermetic memory opening.
class PreloadedRuntime private constructor() {
    internal companion object {
        fun load(): PreloadedRuntime {
            if (HoyaNativeTransport.abiVersion() != 1) throw HoyaException("InvalidResponse")
            return PreloadedRuntime()
        }
    }
    override fun toString() = "<PreloadedRuntime>"
}
fun preloadRuntime(): PreloadedRuntime = PreloadedRuntime.load()

suspend fun openMemoryDatabase(runtime: PreloadedRuntime, schema: Schema, namespace: String,
    eventExecutor: Executor, idGenerator: (() -> String?)? = null, options: DatabaseOptions = DatabaseOptions(),
    store: MemoryStore? = null, migrations: List<Migration> = emptyList()): Database {
    val database = Database(eventExecutor, idGenerator, options)
    try { database.openMemory(schema, namespace, store, migrations); return database }
    catch (error: Throwable) { database.close(); throw error }
}

class ManualClock(initial: Long = 0) {
    private var value = initial
    @Synchronized fun now(): Long = value
    @Synchronized fun set(value: Long) { this.value = value }
    @Synchronized fun advance(delta: Long) {
        val next = try { Math.addExact(value, delta) } catch (_: ArithmeticException) { throw HoyaException("InvalidRequest") }
        value = next
    }
    override fun toString() = "<ManualClock>"
}
