// Ordinary app-only mixed-language control. No internal SDK or engine API.
package dev.hoyadb.consumer

import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import dev.hoyadb.CapturedContext
import dev.hoyadb.Column
import dev.hoyadb.ColumnKind
import dev.hoyadb.CommandDefinition
import dev.hoyadb.CommandTask
import dev.hoyadb.Database
import dev.hoyadb.JSONValue
import dev.hoyadb.Query
import dev.hoyadb.Schema
import dev.hoyadb.Table
import dev.hoyadb.startCommand
import java.util.concurrent.Executors
import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.resume
import kotlin.coroutines.startCoroutine
import kotlin.coroutines.suspendCoroutine

private class ForeignGate {
  private val lock = Any()
  private var released = false
  private var waiting: Continuation<Unit>? = null

  suspend fun await() = suspendCoroutine<Unit> { continuation ->
    val ready = synchronized(lock) {
      if (released) true else { waiting = continuation; false }
    }
    if (ready) continuation.resume(Unit)
  }

  fun release() {
    val continuation = synchronized(lock) {
      released = true
      waiting.also { waiting = null }
    }
    continuation?.resume(Unit)
  }
}

class HoyaConsumerModule(context: ReactApplicationContext) : NativeHoyaConsumerSpec(context) {
  private val executor = Executors.newSingleThreadExecutor { work ->
    Thread(work, "hoya-rn-owned-consumer").apply { isDaemon = true }
  }
  private val lock = Any()
  private var database: Database? = null
  private var command: CommandTask? = null
  private var gate: ForeignGate? = null
  private var live = true

  override fun getName() = NAME

  private fun launch(promise: Promise, work: suspend () -> Unit) {
    executor.execute {
      work.startCoroutine(object : Continuation<Unit> {
        override val context = EmptyCoroutineContext
        override fun resumeWith(result: Result<Unit>) {
          if (result.isSuccess) promise.resolve(null)
          else promise.reject("ForeignFailure", "ForeignFailure")
        }
      })
    }
  }

  override fun prepareForeign(promise: Promise) = launch(promise) {
    synchronized(lock) { check(live && database == null) }
    val db = Database(executor)
    try {
      db.openMemory(
        Schema(1, mapOf("foreign_notes" to Table(mapOf("label" to Column(ColumnKind.String))))),
        namespace = "a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6")
      synchronized(lock) { check(live); database = db }
    } catch (error: Throwable) {
      db.close()
      throw error
    }
  }

  override fun startForeign(promise: Promise) {
    try {
      synchronized(lock) {
        check(live && command == null)
        val db = checkNotNull(database)
        val hold = ForeignGate()
        gate = hold
        command = db.startCommand(
          CommandDefinition("kotlin-foreign-v1"), JSONValue.ObjectValue(emptyMap()),
          CapturedContext(JSONValue.StringValue("foreign-consumer"), 1000)
        ) { scope ->
          scope.fetch(Query("foreign_notes"))
          promise.resolve(null)
          hold.await()
          JSONValue.Null
        }
      }
    } catch (_: Throwable) { promise.reject("ForeignFailure", "ForeignFailure") }
  }

  override fun releaseForeign(promise: Promise) = launch(promise) {
    val retained = synchronized(lock) { checkNotNull(command) to checkNotNull(gate) }
    retained.second.release()
    retained.first.await()
    synchronized(lock) { command = null; gate = null }
  }

  override fun closeForeign(promise: Promise) = launch(promise) { closeOwned() }

  private suspend fun closeOwned() {
    val retained = synchronized(lock) {
      Triple(command, gate, database).also { command = null; gate = null; database = null }
    }
    retained.first?.cancel()
    retained.second?.release()
    try { retained.first?.await() } catch (_: Throwable) { /* Explicit owned cancellation. */ }
    retained.third?.close()
  }

  override fun invalidate() {
    synchronized(lock) { live = false }
    executor.execute {
      suspend { closeOwned() }.startCoroutine(object : Continuation<Unit> {
        override val context = EmptyCoroutineContext
        override fun resumeWith(result: Result<Unit>) { executor.shutdown() }
      })
    }
    super.invalidate()
  }
}
