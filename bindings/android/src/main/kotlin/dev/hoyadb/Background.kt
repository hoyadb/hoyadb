package dev.hoyadb

/** API-0011: a redacted fact, never an exception, message, row or command key. */
class BackgroundError(val source: Source, val kind: String, val token: String?) {
    enum class Source { Observation, Callback, Sync, Merge, Timer }
    override fun toString() = "<BackgroundError>"
}

internal fun backgroundKind(kind: String?): String = when {
    kind == "InvalidResponse" -> "InvalidPayload"
    kind == "CallbackFailure" || kind == "Cancelled" -> kind
    // Host-only kinds are not public background kinds (Swift maps its backup failure the same way).
    kind != "NativeFailure" && kind != "BackupPolicyFailure" && kind in errorKinds -> kind!!
    else -> "BackendFailure"
}
internal fun backgroundKind(error: Throwable): String = backgroundKind((error as? HoyaException)?.kind)

internal fun Database.reportBackground(source: BackgroundError.Source, kind: String, token: String?, generation: Long) {
    val sink = onBackgroundError ?: return
    if (synchronized(stateLock) { owner == null || epoch != generation }) return
    val fact = BackgroundError(source, backgroundKind(kind), token)
    try {
        eventExecutor.execute {
            if (synchronized(stateLock) { owner == null || epoch != generation }) return@execute
            // No database/observation lock spans foreign code. Once begun, the
            // callback follows the same lifecycle rule as managed delivery.
            try { sink(fact) } catch (_: Throwable) { /* Reporting never owns a caller's result. */ }
        }
    } catch (_: Throwable) { /* A rejecting sink executor cannot replace committed work. */ }
}
