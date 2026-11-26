// TC-0040 OD04/OD07/OD08 (SPEC-0036 items 2-3), Swift SDK delivery only: scripted fake
// NativeCalling replies prove SDK decoding and sink rules, not end-to-end Rust behavior.
import Foundation
import XCTest

@testable import HoyaDB

private actor ScriptedRuntime: NativeCalling {
  var pages: [[Wire]] = []
  var failDrain: String?
  private var next = 10
  private(set) var operations: [String] = []
  private var held: CheckedContinuation<Void, Never>?
  private var holdNextDrain = false

  func script(_ pages: [[Wire]]) { self.pages = pages }
  func fail(_ kind: String?) { failDrain = kind }
  func holdDrain() { holdNextDrain = true }
  func release() {
    held?.resume()
    held = nil
  }
  var isHolding: Bool { held != nil }

  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    let op = try Wire.decode(input).object?["op"]?.string ?? ""
    operations.append(op)
    switch op {
    case "observe", "observe_status", "observe_progress", "observe_outcome":
      next += 1
      return .string(String(next))
    case "drain":
      if holdNextDrain {
        holdNextDrain = false
        await withCheckedContinuation { held = $0 }
      }
      if let failDrain { throw HoyaError.core(failDrain) }
      let page = pages.isEmpty ? [] : pages.removeFirst()
      return .object(["events": .array(page), "has_more": .boolean(false)])
    case "unsubscribe", "unsubscribe_sync": return .boolean(true)
    case "prepare_push":
      return .object([
        "envelopes": .array([.string("{}")]), "next_due_ms": .null, "has_more": .boolean(false),
      ])
    default: return .null
    }
  }
}

private final class Recorder<T: Sendable>: @unchecked Sendable {
  private let lock = NSLock()
  private var items: [T] = []
  func add(_ item: T) {
    lock.lock()
    items.append(item)
    lock.unlock()
  }
  var values: [T] {
    lock.lock()
    defer { lock.unlock() }
    return items
  }
}

private let now: EventExecutor = { work in work() }

private func rows(_ subscription: String, _ count: Int) -> Wire {
  .object([
    "kind": .string("data"), "subscription": .string(subscription),
    "value": .object([
      "kind": .string("rows"),
      "rows": .array(
        (0..<count).map { index in
          .object([
            "id": .string("r\(index)"),
            "fields": .object([
              "label": .object(["kind": .string("string"), "value": .string("x")])
            ]),
          ])
        }),
    ]),
  ])
}

private func terminal(_ subscription: String) -> Wire {
  .object([
    "kind": .string("error"), "subscription": .string(subscription),
    "value": .object(["kind": .string("LimitExceeded")]),
  ])
}

private func eventually(_ condition: @escaping () async -> Bool) async {
  for _ in 0..<200 {
    if await condition() { return }
    try? await Task.sleep(nanoseconds: 10_000_000)
  }
}

private let schema = Schema(
  version: 1,
  tables: [
    "a": Table(columns: ["label": Column(.string)]),
    "b": Table(columns: ["label": Column(.string)]),
  ])

final class BackgroundErrorTests: XCTestCase, @unchecked Sendable {
  private func open(_ runtime: ScriptedRuntime, _ reports: Recorder<BackgroundError>) async throws
    -> Database
  {
    let db = Database(runtime: runtime, onBackgroundError: { reports.add($0) })
    try await db.openMemory(schema: schema, namespace: "40404040404040404040404040400010")
    return db
  }

  func testTerminalEventRetiresOnlyItsListenerAndReportsOnce() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    let a = Recorder<Result<ObservedValue, HoyaError>>()
    let b = Recorder<Result<ObservedValue, HoyaError>>()
    let sa = try await db.observe(Query("a"), executor: now) { a.add($0) }
    let sb = try await db.observe(Query("b"), executor: now) { b.add($0) }
    await eventually { await runtime.operations.contains("drain") }
    try await Task.sleep(nanoseconds: 50_000_000)
    await runtime.script([
      [rows("11", 1), terminal("11"), rows("12", 0)], [rows("11", 2), rows("12", 1)],
    ])
    try await db.drainNotifications()
    try await db.drainNotifications()
    XCTAssertEqual(
      a.values,
      [
        .success(.rows([Row(id: "r0", fields: ["label": .string("x")])])),
        .failure(.subscriptionTerminated("LimitExceeded")),
      ])
    XCTAssertEqual(b.values.count, 2)
    XCTAssertEqual(
      reports.values, [BackgroundError(source: .observation, kind: "LimitExceeded", token: "11")])
    let operations = await runtime.operations
    XCTAssertFalse(operations.contains("unsubscribe"), "no core call for a terminated handle")
    sa.cancel()
    sb.cancel()
    try await db.close()
  }

  func testRecoverableDataErrorKeepsListener() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    let a = Recorder<Result<ObservedValue, HoyaError>>()
    let sa = try await db.observe(Query("a"), executor: now) { a.add($0) }
    await eventually { await runtime.operations.contains("drain") }
    try await Task.sleep(nanoseconds: 50_000_000)
    let recoverable: Wire = .object([
      "kind": .string("data"), "subscription": .string("11"),
      "value": .object([
        "kind": .string("error"), "error": .object(["kind": .string("BackendFailure")]),
      ]),
    ])
    await runtime.script([[recoverable], [rows("11", 0)]])
    try await db.drainNotifications()
    try await db.drainNotifications()
    XCTAssertEqual(a.values, [.failure(.core("BackendFailure")), .success(.rows([]))])
    XCTAssertEqual(reports.values, [])
    sa.cancel()
    try await db.close()
  }

  func testOwnerWideManagedDrainFailureReachesSinkNotListeners() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    await runtime.fail("BackendFailure")
    let a = Recorder<Result<ObservedValue, HoyaError>>()
    let sa = try await db.observe(Query("a"), executor: now) { a.add($0) }
    await eventually { !reports.values.isEmpty }
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertEqual(
      reports.values, [BackgroundError(source: .observation, kind: "BackendFailure", token: nil)])
    XCTAssertEqual(a.values, [], "owner-wide failure is not broadcast to listeners")
    sa.cancel()
    try await db.close()
  }

  func testExplicitDrainFailureIsThrownNotReported() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    let a = Recorder<Result<ObservedValue, HoyaError>>()
    let sa = try await db.observe(Query("a"), executor: now) { a.add($0) }
    await eventually { await runtime.operations.contains("drain") }
    try await Task.sleep(nanoseconds: 50_000_000)
    await runtime.fail("BackendFailure")
    do {
      try await db.drainNotifications()
      XCTFail("explicit drain failure must be returned to the caller")
    } catch {
      XCTAssertEqual(error as? HoyaError, .core("BackendFailure"))
    }
    XCTAssertEqual(reports.values, [])
    XCTAssertEqual(
      a.values.filter { if case .failure = $0 { return true } else { return false } }, [])
    sa.cancel()
    try await db.close()
  }

  func testGenericTerminalReachesTypedSyncAndRawObserversOnce() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    let status = Recorder<Result<QueueStatus, HoyaError>>()
    let progress = Recorder<Result<SyncProgress, HoyaError>>()
    let outcome = Recorder<Result<CommandOutcome, HoyaError>>()
    let raw = Recorder<Result<ProtocolValue, HoyaError>>()
    let s1 = try await db.observeQueueStatus(executor: now) { status.add($0) }
    let s2 = try await db.observeProgress(executor: now) { progress.add($0) }
    let s3 = try await db.observeOutcome("k", executor: now) { outcome.add($0) }
    let query: ProtocolValue = .object([
      "table": .string("a"), "predicates": .array([]), "order": .null, "limit": .null,
      "source": .null,
    ])
    let s4 = try await db.observe(query, executor: now) { raw.add($0) }
    await eventually { await runtime.operations.contains("drain") }
    try await Task.sleep(nanoseconds: 50_000_000)
    await runtime.script([
      [terminal("11"), terminal("12"), terminal("13"), terminal("14")], [terminal("11")],
    ])
    try await db.drainNotifications()
    try await db.drainNotifications()
    let ended = HoyaError.subscriptionTerminated("LimitExceeded")
    XCTAssertEqual(status.values, [.failure(ended)])
    XCTAssertEqual(progress.values, [.failure(ended)])
    XCTAssertEqual(outcome.values, [.failure(ended)])
    XCTAssertEqual(raw.values, [.failure(ended)])
    XCTAssertEqual(reports.values.map(\.token), ["11", "12", "13", "14"])
    for subscription in [s1, s2, s3, s4] { subscription.cancel() }
    try await db.close()
  }

  func testAutomaticSyncFailureReachesSinkAndExistingOnError() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    let adapter = Recorder<HoyaError>()
    try await db.startSync(
      transport: { _ in throw HoyaError.core("BackendFailure") }, jitter: { 0 },
      onError: { adapter.add($0) })
    await eventually { !reports.values.isEmpty && !adapter.values.isEmpty }
    await db.stopSync()
    XCTAssertEqual(adapter.values.first, .core("BackendFailure"))
    XCTAssertEqual(
      reports.values.first, BackgroundError(source: .sync, kind: "BackendFailure", token: nil))
    try await db.close()
  }

  /// Parity with JS OD07 (closed kind allowlist); added with the implementation, not RED-first.
  func testMalformedTerminalKindIsReportedAsBackendFailure() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    let a = Recorder<Result<ObservedValue, HoyaError>>()
    let sa = try await db.observe(Query("a"), executor: now) { a.add($0) }
    await eventually { await runtime.operations.contains("drain") }
    try await Task.sleep(nanoseconds: 50_000_000)
    let malformed: Wire = .object([
      "kind": .string("error"), "subscription": .string("11"),
      "value": .object(["kind": .string("NotAKind")]),
    ])
    await runtime.script([[malformed]])
    try await db.drainNotifications()
    XCTAssertEqual(a.values, [.failure(.subscriptionTerminated("BackendFailure"))])
    XCTAssertEqual(
      reports.values, [BackgroundError(source: .observation, kind: "BackendFailure", token: "11")])
    sa.cancel()
    try await db.close()
  }

  func testLateFailureAfterCloseIsSuppressed() async throws {
    let runtime = ScriptedRuntime()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    await runtime.holdDrain()
    let a = Recorder<Result<ObservedValue, HoyaError>>()
    let sa = try await db.observe(Query("a"), executor: now) { a.add($0) }
    await eventually { await runtime.isHolding }
    await runtime.fail("BackendFailure")
    try await db.close()
    await runtime.release()
    try await Task.sleep(nanoseconds: 100_000_000)
    XCTAssertEqual(reports.values, [])
    XCTAssertEqual(a.values, [])
    sa.cancel()
  }
}
