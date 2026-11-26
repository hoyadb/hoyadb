// A Todo app with the HoyaDB Swift binding. The documentation build compiles and runs this file
// against a locally built native core; the named regions are the Swift getting-started snippets.
// The Swift binding uses typed values (`Cell`, `JSONValue`) rather than plain dictionaries.
// docs:start schema
import Foundation
import HoyaDB
import HoyaDBTesting

let schema = Schema(
  version: 1,
  tables: [
    "todos": Table(columns: [
      "title": Column(.string),
      "done": Column(.boolean),
    ])
  ])
// docs:end schema

struct CheckFailure: Error { let message: String }
func check(_ condition: Bool, _ message: String) throws {
  if !condition { throw CheckFailure(message: message) }
}
/// Observation callbacks run on an executor; this records what they saw, thread-safely.
final class Seen: @unchecked Sendable {
  private let lock = NSLock()
  private var titles: [String] = []
  func record(_ rows: [Row]) {
    lock.lock()
    titles = rows.compactMap {
      if case .string(let title)? = $0.fields["title"] { title } else { nil }
    }.sorted()
    lock.unlock()
  }
  var current: [String] {
    lock.lock()
    defer { lock.unlock() }
    return titles
  }
}
func until(_ message: String, _ predicate: () -> Bool) async throws {
  for _ in 0..<2000 {
    if predicate() { return }
    try await Task.sleep(nanoseconds: 1_000_000)
  }
  throw CheckFailure(message: "timed out waiting for \(message)")
}

// docs:start open
let db = Database()
try await db.openMemory(schema: schema)
// docs:end open

do {
  // docs:start query
  let openTasks = Query(
    "todos", predicates: [.init("done", .equal, [.boolean(false)])], order: .init("title"))
  let rows = try await db.fetch(openTasks)
  // rows: [Row] — each has an `id` and `fields`, such as fields["title"] == .string("Buy milk")
  // docs:end query
  try check(rows.isEmpty, "a new database has no tasks")

  // docs:start observe
  let seen = Seen()
  let subscription = try await db.observe(openTasks) { result in
    if case .success(.rows(let rows)) = result { seen.record(rows) }
  }
  // The callback first receives the current rows, then again after each change.
  // docs:end observe

  // docs:start command-definition
  let addTaskV1 = CommandDefinition("add-task-v1", arguments: ["title": .init(.string)])
  // The local handler runs on the device. Its scope is a view for this command only, and all of
  // its writes commit together when it returns. New rows get temporary "local:" IDs.
  let addTask: @Sendable (CommandScope) async throws -> JSONValue = { scope in
    guard case .object(let args) = try await scope.arguments(),
      case .string(let title)? = args["title"]
    else { throw CheckFailure(message: "missing title") }
    let task = try await scope.insert(
      "todos", fields: ["title": .string(title), "done": .boolean(false)])
    return .object(["id": .string(task.id)])
  }
  // docs:end command-definition

  // docs:start execute
  let added = try await db.command(
    addTaskV1, value: .object(["title": .string("Buy milk")]),
    context: CapturedContext(identity: .string("ada"), nowMilliseconds: 1000), handler: addTask)
  guard case .object(let result) = added.value, case .string(let localID)? = result["id"]
  else { throw CheckFailure(message: "the handler returned the new task") }
  // localID starts with "local:"; added.key identifies the queued command.
  // docs:end execute
  try check(localID.hasPrefix("local:"), "the new task has a temporary local ID, got \(localID)")
  try await until("the observer sees the new task") { seen.current == ["Buy milk"] }
  try check(
    (try await db.queueStatus()).entries.first?.state == .queued, "the command is queued for sync")

  // docs:start backend
  // Your server runs its own version of add-task-v1 and answers with the task's real ID. In a real
  // app, send the envelopes to your HTTP API; here the test backend plays the server.
  let backend = FakeTransport(handlers: [
    "add-task-v1": { _ in .applied(.object(["id": .string("server-task-1")])) }
  ])
  // docs:end backend

  // docs:start sync
  let batch = try await db.preparePush(nowMilliseconds: 1000, jitterBasisPoints: 0)
  let answers = try await backend.push("[" + batch.envelopes.joined(separator: ",") + "]")
  _ = try await db.receiveWire(answers)
  // Confirm the command: map the one local ID it created to the ID the server assigned.
  for pending in try await db.pendingReconciliations() where pending.key == added.key {
    try await db.reconcile(pending.key, plan: ReconcilePlan(mappings: [localID: "server-task-1"]))
  }
  // docs:end sync
  try check(
    (try await db.queueStatus()).entries.first?.state == .applied, "the command was confirmed")
  let synced = try await db.fetch(openTasks)
  try check(
    synced.map(\.id) == ["server-task-1"], "the task now has the server ID, got \(synced.map(\.id))"
  )

  // docs:start close
  subscription.cancel()
  try await db.close()
  // docs:end close
} catch {
  try? await db.close()
  throw error
}
print("swift tour: ok")
