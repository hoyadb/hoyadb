package dev.hoyadb

// Reviewed Android PLAN: deterministic close on normal and exceptional exits.
suspend fun <T> useDatabase(database: Database, body: suspend (Database) -> T): T {
    try { return body(database) } finally { database.close() }
}
