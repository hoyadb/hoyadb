// TC-0044 CP13: controlled NativeCalling tests prove Swift host routing/lifecycle,
// not core discard semantics or current native artifact provenance.
import Foundation
import XCTest

@testable import HoyaDB

private let conflictSchema = Schema(
  version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
private let kept: Wire = .object([
  "kind": .string("kept_current"), "discarded_keys": .array([.string("command:1")]),
  "unmapped": .array([.string("local:one")]), "skipped_writes": .string("0"),
])

private actor PolicyRuntime: NativeCalling {
  private(set) var calls: [(String, Wire)] = []
  var policyError: HoyaError?
  var closeError: HoyaError?
  var holdPolicy = false
  var holdResolution = false
  var held: CheckedContinuation<Void, Never>?
  var holding: Bool { held != nil }
  var pending: [String] = []
  var resolution: Wire = kept

  func configure(error: HoyaError? = nil, hold: Bool = false, closeError: HoyaError? = nil) {
    policyError = error
    holdPolicy = hold
    self.closeError = closeError
  }

  func holdNextResolution() { holdResolution = true }
  func setPending(_ keys: [String]) { pending = keys }
  func setResolution(_ value: Wire) { resolution = value }
  func release() {
    held?.resume()
    held = nil
  }
  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    let frame = try Wire.decode(input)
    let op = frame.object?["op"]?.string ?? ""
    calls.append((op, frame.object?["args"] ?? .null))
    if op == "set_conflict_policy" {
      if holdPolicy { await withCheckedContinuation { held = $0 } }
      if let policyError { throw policyError }
    }
    if op == "close", let closeError { throw closeError }
    if ["reconcile_resolving", "rollback_resolving", "discard"].contains(op) {
      if holdResolution { await withCheckedContinuation { held = $0 } }
      pending.removeAll { $0 == frame.object?["args"]?.object?["key"]?.string }
      return resolution
    }
    if op == "prepare_push" {
      return .object(["envelopes": .array([]), "next_due_ms": .null, "has_more": .boolean(false)])
    }
    if op == "pending_reconciliations" {
      return .array(
        pending.map {
          .object([
            "key": .string($0),
            "outcome": .object([
              "status": .string("applied"), "result": .object(["kind": .string("null")]),
            ]),
          ])
        })
    }
    return .null
  }
}

private final class PolicyReports: @unchecked Sendable {
  let lock = NSLock()
  private var resolutions: [(String, Resolution)] = []
  private var errors: [BackgroundError] = []
  func resolved(_ key: String, _ value: Resolution) {
    lock.lock()
    resolutions.append((key, value))
    lock.unlock()
  }
  func failed(_ error: BackgroundError) {
    lock.lock()
    errors.append(error)
    lock.unlock()
  }
  var count: Int {
    lock.lock()
    defer { lock.unlock() }
    return resolutions.count
  }
  var failures: [BackgroundError] {
    lock.lock()
    defer { lock.unlock() }
    return errors
  }
}

final class ConflictPolicyTests: XCTestCase, @unchecked Sendable {
  private func open(_ db: Database) async throws {
    try await db.openMemory(schema: conflictSchema, namespace: "44444444444444444444444444444444")
  }

  private func waitForHold(_ runtime: PolicyRuntime) async {
    for _ in 0..<200 {
      if await runtime.holding { return }
      try? await Task.sleep(nanoseconds: 1_000_000)
    }
    XCTFail("expected controlled runtime to be held")
  }

  func testLegacyMethodsUseReportingPathAndDeliverEveryKeptCurrent() async throws {
    let runtime = PolicyRuntime()
    let reports = PolicyReports()
    let db = Database(runtime: runtime, onResolution: { reports.resolved($0, $1) })
    try await open(db)
    try await db.reconcile("a", plan: ReconcilePlan())
    try await db.reconcileGuarded("b", plan: ReconcilePlan(), guards: [])
    try await db.rollback("c")
    let calls = await runtime.calls.map(\.0)
    XCTAssertEqual(calls, ["reconcile_resolving", "reconcile_resolving", "rollback_resolving"])
    XCTAssertEqual(reports.count, 3)
    try await db.close()
  }

  func testConstructorPolicyInstallsBeforeOpenReturnsAndReappliesOnReopen() async throws {
    let runtime = PolicyRuntime()
    let db = Database(runtime: runtime, conflict: .init(default: .discard))
    try await open(db)
    let first = await runtime.calls
    XCTAssertEqual(first.map(\.0), ["set_conflict_policy"])
    XCTAssertEqual(
      first.first?.1,
      .object(["policy": .object(["default": .string("discard"), "commands": .object([:])])]))
    try await db.close()
    try await open(db)
    let calls = await runtime.calls.map(\.0)
    XCTAssertEqual(calls, ["set_conflict_policy", "close", "set_conflict_policy"])
    try await db.close()
  }

  func testFailedPolicyDisposesOwnerAndCanReopen() async throws {
    let runtime = PolicyRuntime()
    await runtime.configure(error: .core("InvalidPayload"))
    let db = Database(runtime: runtime, conflict: .init(default: .discard))
    do {
      try await open(db)
      XCTFail("failed policy published owner")
    } catch { XCTAssertEqual(error as? HoyaError, .core("InvalidPayload")) }
    let calls = await runtime.calls.map(\.0)
    XCTAssertEqual(calls, ["set_conflict_policy", "close"])
    await runtime.configure()
    try await open(db)
    try await db.close()
  }

  func testThrowingResolutionCallbackRemainsInstalledAndCannotMaskResult() async throws {
    let runtime = PolicyRuntime()
    let reports = PolicyReports()
    let db = Database(
      runtime: runtime, onBackgroundError: { reports.failed($0) },
      onResolution: {
        reports.resolved($0, $1)
        throw HoyaError.core("PrivateFailure")
      })
    try await open(db)
    let first = try await db.discard("a")
    let second = try await db.rollbackResolving("b")
    let expected = Resolution.keptCurrent(
      discardedKeys: ["command:1"], unmapped: ["local:one"], skippedWrites: 0)
    XCTAssertEqual(first, expected)
    XCTAssertEqual(second, expected)
    XCTAssertEqual(reports.count, 2)
    XCTAssertEqual(
      reports.failures,
      Array(
        repeating: BackgroundError(source: .callback, kind: "CallbackFailure", token: nil), count: 2
      ))
    try await db.close()
  }

  func testCloseDuringConfigurationWaitsForAndDisposesUnpublishedOwnerOnce() async throws {
    let runtime = PolicyRuntime()
    await runtime.configure(hold: true)
    let db = Database(runtime: runtime, conflict: .init(default: .discard))
    let opening = Task { try await self.open(db) }
    await waitForHold(runtime)
    do {
      _ = try await db.schema()
      XCTFail("unconfigured owner was usable")
    } catch { XCTAssertEqual(error as? HoyaError, .closed) }
    let generation = await db.epoch
    let closing = Task { try await db.close() }
    for _ in 0..<200 {
      if await db.epoch != generation { break }
      try await Task.sleep(nanoseconds: 1_000_000)
    }
    let retired = await db.epoch
    XCTAssertNotEqual(retired, generation)
    await runtime.release()
    do {
      try await opening.value
      XCTFail("retired owner was published")
    } catch { XCTAssertEqual(error as? HoyaError, .closed) }
    try await closing.value
    let calls = await runtime.calls.map(\.0)
    XCTAssertEqual(calls, ["set_conflict_policy", "close"])
  }

  func testFailedConfigurationCleanupRetainsHandleForExplicitCloseRetry() async throws {
    let runtime = PolicyRuntime()
    await runtime.configure(error: .core("InvalidPayload"), closeError: .core("BackendFailure"))
    let db = Database(runtime: runtime, conflict: .init(default: .discard))
    do {
      try await open(db)
      XCTFail("cleanup failure was hidden")
    } catch { XCTAssertEqual(error as? HoyaError, .core("BackendFailure")) }
    do {
      try await open(db)
      XCTFail("retained owner allowed another open")
    } catch { XCTAssertEqual(error as? HoyaError, .busy) }
    await runtime.configure()
    try await db.close()
    let calls = await runtime.calls.map(\.0)
    XCTAssertEqual(calls, ["set_conflict_policy", "close", "close"])
    try await open(db)
    try await db.close()
  }

  func testCloseAndResetSuppressLateResolutionAndCallbackFailureReports() async throws {
    for retire in ["close", "reset"] {
      let runtime = PolicyRuntime()
      let reports = PolicyReports()
      let db = Database(
        runtime: runtime, onBackgroundError: { reports.failed($0) },
        onResolution: {
          reports.resolved($0, $1)
          throw HoyaError.boundaryFailure
        })
      try await open(db)
      await runtime.holdNextResolution()
      let resolving = Task { try await db.discard("late") }
      await waitForHold(runtime)
      _ = try await db.execute(
        retire, arguments: retire == "reset" ? ["mode": .string("preserve_recovery")] : [:])
      await runtime.release()
      do {
        _ = try await resolving.value
        XCTFail("late result escaped retired epoch")
      } catch { XCTAssertEqual(error as? HoyaError, .closed) }
      XCTAssertEqual(reports.count, 0)
      XCTAssertEqual(reports.failures, [])
      try await db.close()
    }
  }

  func testReportingDecoderRejectsMalformedResultAndPreservesFullWidthCount() async throws {
    XCTAssertThrowsError(
      try Resolution(wire: .object(["kind": .string("completed"), "extra": .null])))
    XCTAssertThrowsError(
      try Resolution(
        wire: .object([
          "kind": .string("kept_current"), "discarded_keys": .array([.null]),
          "unmapped": .array([]), "skipped_writes": .string("0"),
        ])))
    XCTAssertThrowsError(
      try Resolution(
        wire: .object([
          "kind": .string("kept_current"), "discarded_keys": .array([]),
          "unmapped": .array([]), "skipped_writes": .string("18446744073709551616"),
        ])))
    let exact = try Resolution(
      wire: .object([
        "kind": .string("kept_current"), "discarded_keys": .array([]),
        "unmapped": .array([]), "skipped_writes": .string(String(UInt64.max)),
      ]))
    XCTAssertEqual(exact, .keptCurrent(discardedKeys: [], unmapped: [], skippedWrites: UInt64.max))
    XCTAssertEqual(QueueEntry.State(rawValue: "discarded"), .discarded)
  }

  func testCompletedResultHasNoCallbackAndSetterLeavesConstructorPolicyForReopen() async throws {
    let runtime = PolicyRuntime()
    let reports = PolicyReports()
    let db = Database(
      runtime: runtime, conflict: .init(default: .stop), onResolution: { reports.resolved($0, $1) })
    try await open(db)
    try await db.setConflictPolicy(.init(default: .discard, commands: ["edit-v1": .stop]))
    await runtime.setResolution(.object(["kind": .string("completed")]))
    let result = try await db.reconcileResolving(
      "a", plan: ReconcilePlan(),
      guards: [
        CorrectionGuard(table: "notes", id: "one", expected: nil)
      ])
    XCTAssertEqual(result, .completed)
    XCTAssertEqual(reports.count, 0)
    try await db.close()
    try await open(db)
    let calls = await runtime.calls
    let policies = calls.filter { $0.0 == "set_conflict_policy" }.map { $0.1.object?["policy"] }
    XCTAssertEqual(
      policies,
      [
        .object(["default": .string("stop"), "commands": .object([:])]),
        .object(["default": .string("discard"), "commands": .object(["edit-v1": .string("stop")])]),
        .object(["default": .string("stop"), "commands": .object([:])]),
      ])
    try await db.close()
  }

  func testManagedResolutionCallbackFailureDoesNotBecomeSyncFailure() async throws {
    let runtime = PolicyRuntime()
    await runtime.setPending(["managed"])
    let reports = PolicyReports()
    let errors = PolicyReports()
    let delivered = expectation(description: "managed resolution")
    let db = Database(
      runtime: runtime, onBackgroundError: { reports.failed($0) },
      onResolution: {
        reports.resolved($0, $1)
        delivered.fulfill()
        throw HoyaError.boundaryFailure
      })
    try await open(db)
    try await db.startSync(
      transport: { _ in "[]" }, reconcile: { _ in ReconcilePlan() }, jitter: { 0 },
      onError: {
        errors.failed(BackgroundError(source: .sync, kind: $0.publicKind, token: nil))
      })
    await fulfillment(of: [delivered], timeout: 2)
    await db.stopSync()
    XCTAssertEqual(reports.count, 1)
    XCTAssertEqual(
      reports.failures, [BackgroundError(source: .callback, kind: "CallbackFailure", token: nil)])
    XCTAssertEqual(errors.failures, [])
    let calls = await runtime.calls.filter { $0.0 == "reconcile_resolving" }
    XCTAssertEqual(calls.count, 1)
    try await db.close()
  }
}
