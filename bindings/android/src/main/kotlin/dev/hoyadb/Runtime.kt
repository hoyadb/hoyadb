package dev.hoyadb

import kotlin.coroutines.Continuation
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
import kotlin.coroutines.startCoroutine

internal class Completion<T> {
    private val lock = Any()
    private var result: Result<T>? = null
    private val waiting = ArrayList<Continuation<T>>()
    fun complete(value: Result<T>) {
        val ready = synchronized(lock) { if (result != null) return; result = value; waiting.toList().also { waiting.clear() } }
        for (continuation in ready) continuation.resumeWith(value)
    }
    fun succeed(value: T) = complete(Result.success(value))
    fun fail(error: Throwable) = complete(Result.failure(error))
    suspend fun await(): T = suspendCoroutine { continuation ->
        val done = synchronized(lock) { result.also { if (it == null) waiting.add(continuation) } }
        if (done != null) continuation.resumeWith(done)
    }
}
internal val errorKinds = setOf("MigrationFailure", "SchemaConflict", "InvalidSchema", "InvalidQuery", "InvalidRow", "BackendFailure", "TableNotFound", "RowAlreadyExists", "RowNotFound",
    "InvalidCommand", "InvalidTicket", "Busy", "Capacity", "InvalidPayload", "Protocol", "UnknownLocalReference", "RollbackConflict", "Closed", "LimitExceeded", "AuthorityRequired", "StalePull", "PullConflict",
    "InvalidRequest", "InvalidHandle", "UnexpectedPending", "UnsupportedOperation", "InvalidResponse", "NativeFailure", "BackupPolicyFailure")
internal fun decodeReply(bytes: ByteArray): ProtocolValue {
    val reply = WireCodec.decode(bytes).fields()
    if (reply["abi"]?.string() != "1") invalidResponse()
    if (reply["ok"]?.boolean() == true) { if (reply.keys != setOf("abi", "ok", "value")) invalidResponse(); return reply.getValue("value") }
    if (reply.keys != setOf("abi", "ok", "error")) invalidResponse()
    val kind = reply.getValue("error").exact("kind").getValue("kind").string()
    if (kind !in errorKinds) invalidResponse()
    throw HoyaException(kind)
}
internal fun requestFrame(operation: String, owner: String?, arguments: Map<String, ProtocolValue>) = WireCodec.encode(obj(
    "abi" to str("1"), "op" to str(operation), "owner" to nullable(owner), "args" to ProtocolValue.ObjectValue(arguments)))
internal suspend fun nativeExchange(runtime: NativeCalling, operation: String, owner: String?, arguments: Map<String, ProtocolValue>, key: ByteArray? = null): ProtocolValue {
    val frame = requestFrame(operation, owner, arguments)
    return suspendCoroutine { continuation ->
        val reply = object : HoyaNativeTransport.Reply {
            override fun onSuccess(bytes: ByteArray) {
                val decoded = try { Result.success(decodeReply(bytes)) } catch (error: HoyaException) { Result.failure(error) }
                // A plain stdlib continuation may execute arbitrary caller code.
                // Never let it occupy the shared Kotlin/RN byte reply executor.
                managedExecutor.execute { continuation.resumeWith(decoded) }
            }
            override fun onFailure(kind: String) {
                val error = HoyaException(if (kind in errorKinds) kind else "NativeFailure")
                managedExecutor.execute { continuation.resumeWithException(error) }
            }
        }
        if (key == null) runtime.call(frame, reply) else runtime.open(frame, key, reply)
    }
}

internal val managedExecutor = java.util.concurrent.Executors.newCachedThreadPool { work ->
    Thread(work, "hoyadb-managed").apply { isDaemon = true }
}
internal class ExecutorContext(private val executor: java.util.concurrent.Executor) : kotlin.coroutines.AbstractCoroutineContextElement(kotlin.coroutines.ContinuationInterceptor), kotlin.coroutines.ContinuationInterceptor {
    override fun <T> interceptContinuation(continuation: Continuation<T>): Continuation<T> = object : Continuation<T> {
        override val context = continuation.context
        override fun resumeWith(result: Result<T>) { executor.execute { continuation.resumeWith(result) } }
    }
}
internal fun launchManaged(body: suspend () -> Unit, failed: (Throwable) -> Unit = {}) {
    body.startCoroutine(object : Continuation<Unit> {
        override val context = ExecutorContext(managedExecutor)
        override fun resumeWith(result: Result<Unit>) { result.exceptionOrNull()?.let(failed) }
    })
}
