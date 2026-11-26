package dev.hoyadb

enum class ConflictReaction(internal val wireName: String) { Stop("stop"), Discard("discard") }

class ConflictPolicy(val default: ConflictReaction = ConflictReaction.Stop, commands: Map<String, ConflictReaction> = emptyMap()) {
    val commands = immutableMap(commands)
    fun toProtocolValue(): ProtocolValue = obj("default" to str(default.wireName), "commands" to ProtocolValue.ObjectValue(commands.mapValues { str(it.value.wireName) }))
    override fun toString() = "<ConflictPolicy>"
}

sealed class Resolution {
    object Completed : Resolution()
    class KeptCurrent(discardedKeys: List<String>, unmapped: List<String>, val skippedWrites: UnsignedDecimal) : Resolution() {
        val discardedKeys = immutableList(discardedKeys)
        val unmapped = immutableList(unmapped)
    }
    final override fun toString() = "<Resolution>"
    companion object {
        fun fromProtocolValue(wire: ProtocolValue): Resolution = when (wire.fields()["kind"]?.string()) {
            "completed" -> { wire.exact("kind"); Completed }
            "kept_current" -> {
                val fields = wire.exact("kind", "discarded_keys", "unmapped", "skipped_writes")
                KeptCurrent(fields.getValue("discarded_keys").items().map { it.string() }, fields.getValue("unmapped").items().map { it.string() }, fields.getValue("skipped_writes").unsigned())
            }
            else -> invalidResponse()
        }
    }
}

suspend fun Database.setConflictPolicy(policy: ConflictPolicy) { execute("set_conflict_policy", mapOf("policy" to policy.toProtocolValue())) }
suspend fun Database.reconcileResolving(key: String, plan: ReconcilePlan, guards: List<CorrectionGuard> = emptyList()): Resolution =
    Resolution.fromProtocolValue(execute("reconcile_resolving", mapOf("key" to str(key), "plan" to plan.toProtocolValue(), "guards" to ProtocolValue.ArrayValue(guards.map { it.toProtocolValue() }))))
suspend fun Database.rollbackResolving(key: String): Resolution = Resolution.fromProtocolValue(execute("rollback_resolving", mapOf("key" to str(key))))
suspend fun Database.discard(key: String): Resolution = Resolution.fromProtocolValue(execute("discard", mapOf("key" to str(key))))

internal val resolutionOperations = setOf("reconcile_resolving", "rollback_resolving", "discard")
internal val unitResolutionOperations = setOf("reconcile", "reconcile_guarded", "rollback")

internal fun Database.notifyResolution(key: String, result: Resolution, generation: Long) {
    if (result !is Resolution.KeptCurrent) return
    val callback = onResolution ?: return
    if (synchronized(stateLock) { owner == null || epoch != generation }) return
    try {
        eventExecutor.execute {
            if (synchronized(stateLock) { owner == null || epoch != generation }) return@execute
            // As with observation delivery, no owner lock spans application code.
            try { callback(key, result) }
            catch (_: Throwable) { reportBackground(BackgroundError.Source.Callback, "CallbackFailure", null, generation) }
        }
    } catch (_: Throwable) { /* An executor failure cannot replace committed resolution. */ }
}
