// SPEC-0039 / TC-0043 GW15 and the GW16 managed-pass rules, scripted NativeCalling only: SDK
// translation and driver behavior, not core guard semantics.
import Foundation
import XCTest

@testable import HoyaDB

private actor GuardRuntime: NativeCalling {
  private(set) var frames: [[String: Wire]] = []
  private let guardedError: String?
  private let reconcileErrors: [String: String]
  private var open: [String]
  private let pages: [Bool]
  private var page = 0

  init(
    guardedError: String? = nil, reconcileErrors: [String: String] = [:], pending: [String] = [],
    pages: [Bool] = [false]
  ) {
    self.guardedError = guardedError
    self.reconcileErrors = reconcileErrors
    open = pending
    self.pages = pages
  }

  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    guard let frame = try Wire.decode(input).object else { throw HoyaError.invalidResponse }
    frames.append(frame)
    switch frame["op"]?.string {
    case "reconcile_resolving":
      if case .array(let guards) = frame["args"]?.object?["guards"], !guards.isEmpty,
        let guardedError
      {
        throw HoyaError.core(guardedError)
      }
      let key = frame["args"]?.object?["key"]?.string ?? ""
      if let error = reconcileErrors[key] { throw HoyaError.core(error) }
      open.removeAll { $0 == key }
      return .object(["kind": .string("completed")])
    case "pending_reconciliations":
      return .array(
        open.map {
          .object([
            "key": .string($0),
            "outcome": .object([
              "status": .string("applied"), "result": .object(["kind": .string("null")]),
            ]),
          ])
        })
    case "prepare_push":
      let more = pages[min(page, pages.count - 1)]
      page += 1
      return .object([
        "envelopes": .array([]), "next_due_ms": .null, "has_more": .boolean(more),
      ])
    case "observe": return .string("11")
    case "drain": return .object(["events": .array([]), "has_more": .boolean(false)])
    default: return .null
    }
  }

  var operations: [String] { frames.map { $0["op"]?.string ?? "" } }
  func args(_ operation: String) -> [Wire] {
    frames.filter { $0["op"]?.string == operation }.compactMap { $0["args"] }
  }
  func count(_ operation: String) -> Int { operations.filter { $0 == operation }.count }
  /// Reconciled keys in the first managed pass: frames before the pass-ending prepare_push.
  func firstPassReconciles(pages: Int) -> [String] {
    var prepares = 0
    var keys: [String] = []
    for frame in frames {
      if frame["op"]?.string == "prepare_push" {
        prepares += 1
        if prepares > pages { break }
      }
      if frame["op"]?.string == "reconcile_resolving",
        let key = frame["args"]?.object?["key"]?.string
      {
        keys.append(key)
      }
    }
    return keys
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

private let schema = Schema(
  version: 1, tables: ["sensors": Table(columns: ["label": Column(.string)])])
private let plan = ReconcilePlan(writes: [
  .put(table: "sensors", row: Row(id: "s", fields: ["label": .string("server")]))
])
private let guards = [
  CorrectionGuard(table: "sensors", id: "s", expected: ["label": .string("seen"), "note": .null]),
  CorrectionGuard(table: "sensors", id: "u", expected: ["label": .string("omitted note")]),
  CorrectionGuard(table: "sensors", id: "t", expected: nil),
]

private func settle(_ condition: @escaping () async -> Bool) async {
  for _ in 0..<200 {
    if await condition() { return }
    try? await Task.sleep(nanoseconds: 10_000_000)
  }
}

final class GuardSDKTests: XCTestCase, @unchecked Sendable {
  private func open(_ runtime: GuardRuntime, _ reports: Recorder<BackgroundError>) async throws
    -> Database
  {
    let db = Database(runtime: runtime, onBackgroundError: { reports.add($0) })
    try await db.openMemory(schema: schema, namespace: "40404040404040404040404040400050")
    return db
  }

  /// Starts the managed driver without a reconciler and settles its initial pass.
  private func parked(_ db: Database, _ runtime: GuardRuntime) async throws {
    try await db.startSync(transport: { _ in "[]" }, now: { 10 }, jitter: { 0 })
    await settle { await runtime.count("prepare_push") >= 1 }
    try await Task.sleep(nanoseconds: 100_000_000)
  }

  func testGW15OneCallCarriesReconcilePlanEncodingAndLosslessGuardsInOrder() async throws {
    let runtime = GuardRuntime()
    let db = try await open(runtime, Recorder())
    try await db.reconcile("A", plan: plan)
    try await db.reconcileGuarded("A", plan: plan, guards: guards)
    let sent = await runtime.args("reconcile_resolving")
    XCTAssertEqual(sent.count, 2, "one call per operation, without fallback")
    let args = try XCTUnwrap(sent.last?.object)
    XCTAssertEqual(args["key"], .string("A"))
    let reconcilePlan = sent.first?.object?["plan"]
    XCTAssertEqual(sent.first?.object?["guards"], .array([]))
    XCTAssertEqual(args["plan"], reconcilePlan, "plan encoded as reconcile encodes it")
    guard case .array(let encoded)? = args["guards"], encoded.count == 3 else {
      return XCTFail("three guards in order")
    }
    XCTAssertEqual(encoded.map { $0.object?["id"] }, [.string("s"), .string("u"), .string("t")])
    XCTAssertEqual(encoded[0].object?["table"], .string("sensors"))
    XCTAssertEqual(encoded[0].object?["expected"]?.object?["note"], try Cell.null.wire)
    XCTAssertEqual(encoded[0].object?["expected"]?.object?["label"], try Cell.string("seen").wire)
    XCTAssertNil(encoded[1].object?["expected"]?.object?["note"], "omitted field stays omitted")
    XCTAssertEqual(encoded[2].object?["expected"], .null, "absence is null")
    XCTAssertEqual(encoded.map { $0.object?.count }, [3, 3, 3])
    try await db.close()
  }

  func testGW15CoreErrorsAreReturnedUnchangedWithoutRetryFallbackOrScheduling() async throws {
    for kind in ["RollbackConflict", "InvalidCommand", "Protocol", "Busy"] {
      let runtime = GuardRuntime(guardedError: kind)
      let reports = Recorder<BackgroundError>()
      let db = try await open(runtime, reports)
      let subscription = try await db.observe(
        Query("sensors"), executor: { $0() }, callback: { _ in })
      try await parked(db, runtime)
      let prepares = await runtime.count("prepare_push")
      let drains = await runtime.count("drain")
      do {
        try await db.reconcileGuarded("A", plan: plan, guards: guards)
        XCTFail("\(kind) was lost")
      } catch {
        XCTAssertEqual(error as? HoyaError, .core(kind))
      }
      try await Task.sleep(nanoseconds: 150_000_000)
      let calls = await runtime.count("reconcile_resolving")
      let fallback = await runtime.count("reconcile")
      let preparesAfter = await runtime.count("prepare_push")
      let drainsAfter = await runtime.count("drain")
      XCTAssertEqual(calls, 1, kind)
      XCTAssertEqual(fallback, 0, kind)
      XCTAssertEqual(preparesAfter, prepares, "\(kind): no driver wake")
      XCTAssertEqual(drainsAfter, drains, "\(kind): no observation drain")
      XCTAssertEqual(reports.values, [], "\(kind): a caller error is not a background fact")
      subscription.cancel()
      try await db.close()
    }
  }

  func testGW15SuccessWakesTheDriverAndDrainsObservationsAsReconcileDoes() async throws {
    for guarded in [false, true] {
      let runtime = GuardRuntime()
      let db = try await open(runtime, Recorder())
      let subscription = try await db.observe(
        Query("sensors"), executor: { $0() }, callback: { _ in })
      try await parked(db, runtime)
      let prepares = await runtime.count("prepare_push")
      let drains = await runtime.count("drain")
      if guarded {
        try await db.reconcileGuarded("A", plan: plan, guards: guards)
      } else {
        try await db.reconcile("A", plan: plan)
      }
      await settle {
        let woke = await runtime.count("prepare_push") > prepares
        let drained = await runtime.count("drain") > drains
        return woke && drained
      }
      let woke = await runtime.count("prepare_push") > prepares
      let drained = await runtime.count("drain") > drains
      XCTAssertTrue(woke, "guarded=\(guarded): driver woken")
      XCTAssertTrue(drained, "guarded=\(guarded): observation drain")
      subscription.cancel()
      try await db.close()
    }
  }

  private func managed(
    _ runtime: GuardRuntime, reconcile: @escaping Reconciler = { _ in plan }
  ) async throws -> (Database, Recorder<HoyaError>, Recorder<BackgroundError>) {
    let errors = Recorder<HoyaError>()
    let reports = Recorder<BackgroundError>()
    let db = try await open(runtime, reports)
    try await db.startSync(
      transport: { _ in "[]" }, reconcile: reconcile, now: { 10 }, jitter: { 0 },
      onError: { errors.add($0) })
    return (db, errors, reports)
  }

  func testGW16ManagedPassReportsRollbackConflictAndContinuesToLaterEntries() async throws {
    let runtime = GuardRuntime(reconcileErrors: ["P1": "RollbackConflict"], pending: ["P1", "P2"])
    let (db, errors, reports) = try await managed(runtime)
    await settle { await runtime.args("reconcile_resolving").count >= 2 }
    try await Task.sleep(nanoseconds: 100_000_000)
    let firstPass = await runtime.firstPassReconciles(pages: 1)
    XCTAssertEqual(firstPass, ["P1", "P2"])
    XCTAssertEqual(errors.values.first, .core("RollbackConflict"))
    XCTAssertEqual(
      reports.values.first, BackgroundError(source: .sync, kind: "RollbackConflict", token: nil))
    let remaining = try await db.pendingReconciliations().map(\.key)
    XCTAssertEqual(remaining, ["P1"], "the conflicted outcome stays pending")
    await db.stopSync()
    try await db.close()
  }

  func testGW16ConflictedKeyIsNotRetriedOnLaterPagesOfThePass() async throws {
    let runtime = GuardRuntime(
      reconcileErrors: ["P1": "RollbackConflict"], pending: ["P1", "P2"], pages: [true, false])
    let (db, _, _) = try await managed(runtime)
    await settle {
      let pages = await runtime.count("prepare_push")
      let reconciles = await runtime.args("reconcile_resolving").count
      return pages >= 2 && reconciles >= 2
    }
    try await Task.sleep(nanoseconds: 100_000_000)
    let firstPass = await runtime.firstPassReconciles(pages: 2)
    XCTAssertEqual(firstPass, ["P1", "P2"])
    await db.stopSync()
    try await db.close()
  }

  func testGW16SkippingAConflictAloneSchedulesNoFurtherPass() async throws {
    let runtime = GuardRuntime(reconcileErrors: ["P1": "RollbackConflict"], pending: ["P1"])
    let (db, errors, _) = try await managed(runtime)
    await settle { await runtime.args("reconcile_resolving").count >= 1 }
    try await Task.sleep(nanoseconds: 200_000_000)
    let prepares = await runtime.count("prepare_push")
    let reconciles = await runtime.args("reconcile_resolving").count
    XCTAssertEqual(prepares, 1)
    XCTAssertEqual(reconciles, 1)
    XCTAssertEqual(errors.values, [.core("RollbackConflict")])
    await db.stopSync()
    try await db.close()
  }

  func testGW16CallbackFailuresAndOtherCoreErrorsKeepTheExistingPassFailure() async throws {
    struct MapperFailure: Error {}
    let throwing = GuardRuntime(pending: ["P1", "P2"])
    let (first, firstErrors, _) = try await managed(throwing) { _ in throw MapperFailure() }
    await settle { !firstErrors.values.isEmpty }
    try await Task.sleep(nanoseconds: 100_000_000)
    let none = await throwing.args("reconcile_resolving").count
    XCTAssertEqual(none, 0)
    XCTAssertEqual(firstErrors.values, [.boundaryFailure])
    await first.stopSync()
    try await first.close()

    let protocolRuntime = GuardRuntime(reconcileErrors: ["P1": "Protocol"], pending: ["P1", "P2"])
    let (second, secondErrors, _) = try await managed(protocolRuntime)
    await settle { !secondErrors.values.isEmpty }
    try await Task.sleep(nanoseconds: 100_000_000)
    let keys = await protocolRuntime.firstPassReconciles(pages: 1)
    XCTAssertEqual(keys, ["P1"], "the pass stops at a non-conflict core error")
    XCTAssertEqual(secondErrors.values, [.core("Protocol")])
    await second.stopSync()
    try await second.close()
  }
}
