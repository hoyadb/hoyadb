// API-0011: common Kotlin/RN owned-byte channel; Rust owns all admission.
package dev.hoyadb

import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

object HoyaNativeTransport {
    interface Reply {
        fun onSuccess(bytes: ByteArray)
        fun onFailure(kind: String)
    }
    @JvmStatic fun abiVersion(): Int = NativeBridge.abiVersion()
    @JvmStatic fun open(frame: ByteArray, key: ByteArray, reply: Reply) {
        if (key.size != 0 && key.size != 32) { replies.execute { reply.onFailure("InvalidRequest") }; return }
        if (frame.size > 1_048_576) { replies.execute { reply.onFailure("LimitExceeded") }; return }
        val owned = frame.copyOf(); val ownedKey = key.copyOf()
        nativeExecutor.execute {
            var output: ByteArray? = null; var failure: String? = null
            try { output = NativeBridge.open(owned, ownedKey) ?: throw HoyaException("NativeFailure") }
            catch (_: IllegalArgumentException) { failure = "InvalidRequest" }
            catch (_: Throwable) { failure = "NativeFailure" }
            finally { ownedKey.fill(0) }
            deliver(reply, output, failure)
        }
    }
    private val nativeExecutor = Executors.newSingleThreadExecutor { work -> Thread(work, "hoyadb-native").apply { isDaemon = true } }
    private val replies = Executors.newSingleThreadExecutor { work -> Thread(work, "hoyadb-native-replies").apply { isDaemon = true } }
    @JvmStatic fun call(frame: ByteArray, reply: Reply) {
        if (frame.size > 1_048_576) { replies.execute { reply.onFailure("LimitExceeded") }; return }
        val owned = frame.copyOf()
        // This read selects a payload-free wake only. Original bytes always go to
        // Rust, whose exact parser alone decides validity and effects.
        val terminal = try {
            (WireCodec.decode(owned).fields()["op"] as? ProtocolValue.StringValue)?.value in terminalOperations
        } catch (_: Exception) { false }
        nativeExecutor.execute {
            var output: ByteArray? = null
            var failure: String? = null
            try { output = NativeBridge.call(owned) ?: throw HoyaException("NativeFailure") }
            catch (_: IllegalArgumentException) { failure = "InvalidRequest" }
            catch (_: Throwable) { failure = "NativeFailure" }
            finally { if (terminal) released() }
            val result = output
            val kind = failure
            replies.execute {
                if (kind != null) reply.onFailure(kind)
                else if (result != null) reply.onSuccess(result)
                else reply.onFailure("NativeFailure")
            }
        }
    }
    @JvmStatic fun validateOpen(frame: ByteArray, keyLength: Int, reply: Reply) {
        if (keyLength != 0 && keyLength != 32) { replies.execute { reply.onFailure("InvalidRequest") }; return }
        if (frame.size > 1_048_576) { replies.execute { reply.onFailure("LimitExceeded") }; return }
        val owned = frame.copyOf()
        nativeExecutor.execute {
            var output: ByteArray? = null; var failure: String? = null
            try { output = NativeBridge.validateOpen(owned, keyLength) ?: throw HoyaException("NativeFailure") }
            catch (_: IllegalArgumentException) { failure = "InvalidRequest" }
            catch (_: Throwable) { failure = "NativeFailure" }
            deliver(reply, output, failure)
        }
    }
    private fun deliver(reply: Reply, output: ByteArray?, failure: String?) {
        replies.execute {
            if (failure != null) reply.onFailure(failure)
            else if (output != null) reply.onSuccess(output)
            else reply.onFailure("NativeFailure")
        }
    }
    private val terminalOperations = setOf("finish", "abort", "cancel_request", "close", "reset")
    private val registrations = LinkedHashSet<ReleaseRegistration>()
    private class ReleaseRegistration(val executor: Executor, val listener: Runnable) : AutoCloseable {
        val live = AtomicBoolean(true)
        override fun close() {
            if (live.getAndSet(false)) synchronized(registrations) { registrations.remove(this) }
        }
        override fun toString() = "<AdmissionReleaseSubscription>"
    }
    @JvmStatic fun subscribeRelease(executor: Executor, listener: Runnable): AutoCloseable {
        val registration = ReleaseRegistration(executor, listener)
        synchronized(registrations) { registrations.add(registration) }
        return registration
    }
    private fun released() {
        val snapshot = synchronized(registrations) { registrations.toList() }
        for (registration in snapshot) {
            if (!registration.live.get()) continue
            try {
                registration.executor.execute {
                    if (registration.live.get()) {
                        try { registration.listener.run() }
                        catch (_: Exception) { registration.close() }
                    }
                }
            } catch (_: Exception) {
                // Retire a rejecting/throwing consumer, never replace an already
                // durable native reply with an application executor failure.
                registration.close()
            }
        }
    }
}
