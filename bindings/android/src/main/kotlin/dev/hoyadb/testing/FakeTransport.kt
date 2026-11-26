package dev.hoyadb.testing

import dev.hoyadb.*
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine

typealias FakeHandler = suspend (JSONValue) -> RemoteOutcome
class FakeTransport(commandHandlers: Map<String, FakeHandler> = emptyMap(), private val entityHandler: FakeHandler? = null,
    private val delayMilliseconds: Long = 0, private val timer: HoyaTimer? = null, private val reorder: Boolean = false,
    private val duplicate: Boolean = false, private val dropPush: () -> Boolean = { false }) {
    private val handlers = immutableMap(commandHandlers)
    init { if (delayMilliseconds < 0) throw HoyaException("InvalidRequest") }
    suspend fun push(body: String): String {
        try {
            if (dropPush()) throw HoyaException("NativeFailure")
            if (delayMilliseconds > 0) suspendCoroutine<Unit> { continuation ->
                (timer ?: SystemHoyaTimer).schedule(delayMilliseconds, Runnable {
                    managedExecutor.execute { continuation.resume(Unit) }
                })
            }
            val input = JSONValue.decodeJSON(body) as? JSONValue.ArrayValue ?: throw HoyaException("InvalidPayload")
            val replies = ArrayList<JSONValue>()
            for (envelope in input.value) {
                val fields = (envelope as? JSONValue.ObjectValue)?.value ?: throw HoyaException("InvalidPayload")
                fun text(name: String): JSONValue.StringValue = fields[name] as? JSONValue.StringValue ?: throw HoyaException("InvalidPayload")
                val correlation: Map<String, JSONValue>
                val handler: FakeHandler
                if (fields.containsKey("writeId")) {
                    correlation = mapOf("table" to text("table"), "recordId" to text("recordId"), "writeId" to text("writeId"))
                    handler = entityHandler ?: throw HoyaException("InvalidCommand")
                } else {
                    correlation = mapOf("idempotencyKey" to text("idempotencyKey"))
                    handler = handlers[text("command").value] ?: throw HoyaException("InvalidCommand")
                }
                val outcome = handler(envelope)
                val business = when (outcome) {
                    is RemoteOutcome.Applied -> mapOf("status" to JSONValue.StringValue("applied"), "result" to outcome.result)
                    is RemoteOutcome.Corrected -> mapOf("status" to JSONValue.StringValue("corrected"), "result" to outcome.result)
                    is RemoteOutcome.Rejected -> mapOf("status" to JSONValue.StringValue("rejected"), "reason" to JSONValue.StringValue(outcome.reason))
                }
                replies.add(JSONValue.ObjectValue(correlation + business))
            }
            val ordered = if (reorder) replies.reversed() else replies
            return JSONValue.ArrayValue(if (duplicate) ordered + ordered else ordered).encodeJSON()
        } catch (error: HoyaException) { throw error }
        catch (_: Exception) { throw HoyaException("NativeFailure") }
    }
    override fun toString() = "<FakeTransport>"
}
