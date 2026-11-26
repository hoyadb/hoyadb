// API-0011 §2 lifecycle (close/reset invalidate host callbacks) and host sync scheduling:
// scripted NativeCalling replies prove SDK state and scheduling only, not Rust behavior.
import Foundation
import XCTest

@testable import HoyaDB

private actor ScriptedOwnerRuntime: NativeCalling {
  private(set) var operations: [String] = []
  private var prepares = 0

  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    let op = try Wire.decode(input).object?["op"]?.string ?? ""
    operations.append(op)
    if op == "prepare_push" {
      prepares += 1
      // One successful pass with a deadline already in the past, then a persistent failure.
      guard prepares == 1 else { throw HoyaError.core("BackendFailure") }
      return .object([
        "envelopes": .array([]), "next_due_ms": .string("5"), "has_more": .boolean(false),
      ])
    }
    return .null
  }

  func count(_ operation: String) -> Int { operations.filter { $0 == operation }.count }
}

private final class Counter: @unchecked Sendable {
  private let lock = NSLock()
  private var value = 0
  func increment() {
    lock.lock()
    value += 1
    lock.unlock()
  }
  var current: Int {
    lock.lock()
    defer { lock.unlock() }
    return value
  }
}

private struct ImmediateTicket: TimerCancellation {
  func cancel() {}
}

final class LifecycleAndSyncRegressionTests: XCTestCase, @unchecked Sendable {
  private let schema = Schema(
    version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
  private let context: EntityContextProvider = {
    CapturedContext(identity: .string("first-session"), nowMilliseconds: 1)
  }

  func testFailedPrepareDoesNotReuseThePreviousDeadline() async throws {
    let runtime = ScriptedOwnerRuntime()
    let errors = Counter()
    // Like the default scheduler at delay zero: the wake is immediate.
    let db = Database(
      runtime: runtime,
      timerScheduler: { _, work in
        Task { work() }
        return ImmediateTicket()
      })
    try await db.openMemory(schema: schema, namespace: "61616161616161616161616161616161")
    try await db.startSync(
      transport: { _ in "[]" }, now: { 10 }, jitter: { 0 }, onError: { _ in errors.increment() })
    try await Task.sleep(nanoseconds: 300_000_000)
    await db.stopSync()
    let prepares = await runtime.count("prepare_push")
    // Pass 1 succeeds (deadline 5 <= now 10 wakes pass 2). Pass 2 fails and must then wait for a
    // real wake instead of rescheduling itself from pass 1's deadline.
    XCTAssertEqual(prepares, 2)
    XCTAssertEqual(errors.current, 1)
    try await db.close()
  }

  func testCloseClearsEntityContextAndMergeHooks() async throws {
    let runtime = ScriptedOwnerRuntime()
    let db = Database(runtime: runtime)
    try await db.openMemory(schema: schema, namespace: "62626262626262626262626262626262")
    try await db.configureEntities(scope: "s", tables: ["notes"], context: context)
    await db.setMergeHooks(["notes": { $0.defaultResolution.row }])
    try await db.close()
    try await db.openMemory(schema: schema, namespace: "62626262626262626262626262626262")
    let hooks = await db.mergeHooks
    XCTAssertTrue(hooks.isEmpty, "a reopened owner must not run the previous owner's merge hooks")
    do {
      _ = try await db.patch("notes", id: "n", fields: [:])
      XCTFail("the previous owner's context provider was used")
    } catch {
      XCTAssertEqual(error as? HoyaError, .core("InvalidCommand"))
    }
    let patches = await runtime.count("patch")
    XCTAssertEqual(patches, 0)
    try await db.close()
  }

  func testResetClearsEntityContextAndMergeHooks() async throws {
    let runtime = ScriptedOwnerRuntime()
    let db = Database(runtime: runtime)
    try await db.openMemory(schema: schema, namespace: "63636363636363636363636363636363")
    try await db.configureEntities(scope: "s", tables: ["notes"], context: context)
    await db.setMergeHooks(["notes": { $0.defaultResolution.row }])
    _ = try await db.execute("reset", arguments: ["mode": .string("preserve_recovery")])
    let hooks = await db.mergeHooks
    XCTAssertTrue(hooks.isEmpty)
    do {
      _ = try await db.patch("notes", id: "n", fields: [:])
      XCTFail("the pre-reset context provider was used")
    } catch {
      XCTAssertEqual(error as? HoyaError, .core("InvalidCommand"))
    }
    let patches = await runtime.count("patch")
    XCTAssertEqual(patches, 0)
    try await db.close()
  }
}

// API-0011 §3.2/§7 SDK parity: raw resolving rewrite, per-receipt sync reports, Busy begin_pull.
private actor ParityRuntime: NativeCalling {
  private(set) var frames: [(op: String, args: [String: Wire])] = []
  private var prepares = 0
  private var pulls = 0
  private let keptCurrent: Wire = .object([
    "kind": .string("kept_current"), "discarded_keys": .array([.string("K")]),
    "unmapped": .array([]), "skipped_writes": .string("0"),
  ])

  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    let frame = try Wire.decode(input).object ?? [:]
    let op = frame["op"]?.string ?? ""
    frames.append((op, frame["args"]?.object ?? [:]))
    switch op {
    case "reconcile_resolving", "rollback_resolving": return keptCurrent
    case "prepare_push":
      prepares += 1
      return .object([
        "envelopes": .array(prepares == 1 ? [.string("{}")] : []), "next_due_ms": .null,
        "has_more": .boolean(false),
      ])
    case "receive_wire":
      func failed(_ key: String, _ kind: String) -> Wire {
        .object([
          "key": .string(key), "ok": .boolean(false), "error": .object(["kind": .string(kind)]),
        ])
      }
      return .array([
        failed("a", "UnknownLocalReference"), .object(["key": .string("b"), "ok": .boolean(true)]),
        failed("c", "Protocol"),
      ])
    case "pending_reconciliations":
      return .array([
        .object([
          "key": .string("K"),
          "outcome": .object(["status": .string("rejected"), "reason": .string("no")]),
        ])
      ])
    case "begin_pull":
      pulls += 1
      if pulls == 1 { throw HoyaError.busy }
      return .object([
        "request": .string("4"), "since": .null, "scope": .string("s"), "generation": .string("1"),
      ])
    case "sync_progress":
      return .object([
        "phase": .string("ready"), "ready": .boolean(true), "pages": .string("1"),
        "changes": .string("0"), "tombstones": .string("0"),
      ])
    default: return .null
    }
  }

  var operations: [String] { frames.map(\.op) }
  func arguments(_ operation: String) -> [[String: Wire]] {
    frames.filter { $0.op == operation }.map(\.args)
  }
}

private final class Log<T: Sendable>: @unchecked Sendable {
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

final class SDKParityRegressionTests: XCTestCase, @unchecked Sendable {
  private let schema = Schema(
    version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
  private let plan: Wire = .object(["writes": .array([]), "mappings": .object([:])])

  private func eventually(_ condition: () async -> Bool) async throws {
    for _ in 0..<400 {
      if await condition() { return }
      try await Task.sleep(nanoseconds: 5_000_000)
    }
    XCTFail("condition not reached")
  }

  func testRawReconcileAndRollbackUseResolvingOperationsAndDeliverResolution() async throws {
    let runtime = ParityRuntime()
    let resolved = Log<String>()
    let db = Database(runtime: runtime, onResolution: { key, _ in resolved.add(key) })
    try await db.openMemory(schema: schema, namespace: "64646464646464646464646464646464")
    let rollback = try await db.execute("rollback", arguments: ["key": .string("K")])
    let reconcile = try await db.execute(
      "reconcile", arguments: ["key": .string("K"), "plan": plan])
    let guarded = try await db.execute(
      "reconcile_guarded", arguments: ["key": .string("K"), "plan": plan, "guards": .array([])])
    // Unit-returning operations drop only the report.
    XCTAssertEqual([rollback, reconcile, guarded], [.null, .null, .null])
    let operations = await runtime.operations
    XCTAssertEqual(
      operations, ["rollback_resolving", "reconcile_resolving", "reconcile_resolving"])
    let reconciled = await runtime.arguments("reconcile_resolving")
    XCTAssertEqual(
      reconciled.first, ["key": .string("K"), "plan": plan, "guards": .array([])])
    XCTAssertEqual(resolved.values, ["K", "K", "K"])
    // A malformed legacy frame keeps the core's exact-shape rejection of the original operation.
    _ = try await db.execute("reconcile", arguments: ["key": .string("K")])
    let last = await runtime.operations.last
    XCTAssertEqual(last, "reconcile")
    try await db.close()
  }

  func testFailedReceiptsAreEachReportedAndThePassContinuesToReconciliation() async throws {
    let runtime = ParityRuntime()
    let errors = Log<HoyaError>()
    let mapped = Log<String>()
    let db = Database(runtime: runtime)
    try await db.openMemory(schema: schema, namespace: "65656565656565656565656565656565")
    try await db.startSync(
      transport: { _ in "[]" },
      reconcile: { pending in
        mapped.add(pending.key)
        return ReconcilePlan()
      }, now: { 10 }, jitter: { 0 }, onError: { errors.add($0) })
    try await eventually { !mapped.values.isEmpty }
    await db.stopSync()
    let operations = await runtime.operations
    let receipt = try XCTUnwrap(operations.firstIndex(of: "receive_wire"))
    // The same pass goes on to reconciliation instead of ending at the first failed receipt.
    XCTAssertEqual(operations[receipt + 1], "pending_reconciliations")
    XCTAssertEqual(
      Array(errors.values.prefix(2)), [.core("UnknownLocalReference"), .core("Protocol")])
    try await db.close()
  }

  func testPullWaitsForAdmissionReleaseWhenBeginPullIsBusy() async throws {
    let runtime = ParityRuntime()
    let db = Database(runtime: runtime)
    try await db.openMemory(schema: schema, namespace: "66666666666666666666666666666666")
    let pull = Task {
      try await db.pullOnce { _, _ in PullPage(checkpoint: "c", complete: true, changes: []) }
    }
    try await eventually { await runtime.operations.contains("begin_pull") }
    // The existing payload-free release notification wakes the retry of the same operation.
    await CommandWake.shared.notify()
    let progress = try await pull.value
    XCTAssertTrue(progress.ready)
    let begins = await runtime.operations.filter { $0 == "begin_pull" }.count
    XCTAssertEqual(begins, 2)
    try await db.close()
  }
}

// API-0011 §4 paged reads: the unpaged call stays the fast path; scripted replies prove the loop only.
private actor PagedRuntime: NativeCalling {
  enum Mode {
    case paged, older
    case vanishing(Int)
    case small
  }
  private let mode: Mode
  private var protocols = 0
  private(set) var calls: [String] = []

  init(_ mode: Mode) { self.mode = mode }

  private func entry(_ key: String) -> Wire {
    .object([
      "key": .string(key),
      "outcome": .object(["status": .string("rejected"), "reason": .string("no")]),
    ])
  }

  private func command(_ key: String) -> Wire {
    .object([
      "kind": .string("command"), "table": .null, "record_id": .null, "key": .string(key),
      "state": .string("queued"), "blocked_on": .array([]), "awaiting_local": .boolean(false),
      "awaiting_key": .null, "rolled_back": .boolean(false), "attempts": .string("0"),
      "next_due_ms": .null,
    ])
  }

  private func status(_ keys: [String], more: Bool?) -> Wire {
    var fields: [String: Wire] = [
      "pending": .string("3"), "sending": .string("0"), "awaiting_outcome": .string("0"),
      "live": .string("3"), "blocked": .boolean(false), "commands": .array(keys.map(command)),
    ]
    if let more { fields["has_more"] = .boolean(more) }
    return .object(fields)
  }

  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    let frame = try Wire.decode(input).object ?? [:]
    let op = frame["op"]?.string ?? ""
    let after = frame["args"]?.object?["after"]
    calls.append(after.map { "\(op):\($0.string ?? "null")" } ?? op)
    switch op {
    case "pending_reconciliations", "queue_status":
      if case .small = mode {
        return op == "queue_status" ? status(["a"], more: nil) : .array([entry("a")])
      }
      throw HoyaError.core("LimitExceeded")
    case "pending_reconciliations_page", "queue_status_page":
      if case .older = mode { throw HoyaError.invalidRequest }
      guard frame["args"]?.object?.count == 1, let after else { throw HoyaError.invalidRequest }
      let keys: [String]
      let more: Bool
      if after == .null {
        (keys, more) = (["a", "b"], true)
      } else {
        if case .vanishing(let times) = mode, protocols < times {
          protocols += 1
          throw HoyaError.core("Protocol")
        }
        guard after == .string("b") else { throw HoyaError.core("Protocol") }
        (keys, more) = (["c"], false)
      }
      return op == "queue_status_page"
        ? status(keys, more: more)
        : .object(["entries": .array(keys.map(entry)), "has_more": .boolean(more)])
    default: return .null
    }
  }
}

final class PagedReadRegressionTests: XCTestCase, @unchecked Sendable {
  private func open(_ mode: PagedRuntime.Mode) async throws -> (Database, PagedRuntime) {
    let runtime = PagedRuntime(mode)
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])]),
      namespace: "67676767676767676767676767676767")
    return (db, runtime)
  }

  func testOversizedPendingReconciliationsFollowCorePagesByKeyCursor() async throws {
    let (db, runtime) = try await open(.paged)
    let pending = try await db.pendingReconciliations()
    XCTAssertEqual(pending.map(\.key), ["a", "b", "c"])
    let calls = await runtime.calls
    XCTAssertEqual(
      calls,
      [
        "pending_reconciliations", "pending_reconciliations_page:null",
        "pending_reconciliations_page:b",
      ])
    try await db.close()
  }

  func testOversizedQueueStatusKeepsFirstPageCountersAndAllEntries() async throws {
    let (db, runtime) = try await open(.paged)
    let status = try await db.queueStatus()
    XCTAssertEqual(status.entries.map(\.key), ["a", "b", "c"])
    XCTAssertEqual(status.pending, 3)
    let calls = await runtime.calls
    XCTAssertEqual(calls, ["queue_status", "queue_status_page:null", "queue_status_page:b"])
    try await db.close()
  }

  func testUnpagedReplyThatFitsNeverPages() async throws {
    let (db, runtime) = try await open(.small)
    let pending = try await db.pendingReconciliations()
    let status = try await db.queueStatus()
    XCTAssertEqual(pending.map(\.key) + status.entries.map(\.key), ["a", "a"])
    let calls = await runtime.calls
    XCTAssertEqual(calls, ["pending_reconciliations", "queue_status"])
    try await db.close()
  }

  func testVanishedCursorRestartsFromTheFirstPageAtMostTwice() async throws {
    let (db, runtime) = try await open(.vanishing(2))
    let pending = try await db.pendingReconciliations()
    XCTAssertEqual(pending.map(\.key), ["a", "b", "c"])
    let restarts = await runtime.calls.filter { $0 == "pending_reconciliations_page:null" }.count
    XCTAssertEqual(restarts, 3)
    try await db.close()
    let (stuck, _) = try await open(.vanishing(3))
    do {
      _ = try await stuck.pendingReconciliations()
      XCTFail("a third vanished cursor must surface Protocol")
    } catch { XCTAssertEqual(error as? HoyaError, .core("Protocol")) }
    try await stuck.close()
  }

  func testOlderNativeLibraryWithoutPagesSurfacesTheOriginalLimit() async throws {
    let (db, _) = try await open(.older)
    do {
      _ = try await db.queueStatus()
      XCTFail("older runtime cannot page")
    } catch { XCTAssertEqual(error as? HoyaError, .core("LimitExceeded")) }
    try await db.close()
  }
}
