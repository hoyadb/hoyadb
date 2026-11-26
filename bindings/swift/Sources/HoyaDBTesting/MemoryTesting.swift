// API-0011 testing entry / ST02: real native memory, never a fake database.
import HoyaDB

public enum MemoryTesting {
  public static func open(
    schema: Schema, namespace: String, store: MemoryStore? = nil,
    migrations: [Migration] = [], idGenerator: IDGenerator? = nil,
    options: DatabaseOptions = .init(), monotonicClock: MonotonicClock? = nil,
    timerScheduler: TimerScheduler? = nil
  ) async throws -> Database {
    let database = Database(
      idGenerator: idGenerator, options: options,
      monotonicClock: monotonicClock, timerScheduler: timerScheduler)
    try await database.openMemory(
      schema: schema, namespace: namespace, store: store, migrations: migrations)
    return database
  }
}
