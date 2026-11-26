// TC-0043 GW16b (a379e8b) deterministic scripted scheduling controls: a skipped managed
// conflict does not self-loop. Added after the SDK implementation; scripted channel only.
import Foundation
import XCTest

@testable import HoyaDB

private actor PassRuntime: NativeCalling {
  private(set) var operations: [String] = []
  private var open: [String]
  private let conflicts: Set<String>

  init(pending: [String], conflicts: Set<String>) {
    open = pending
    self.conflicts = conflicts
  }

  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }

  func call(_ input: Data) async throws -> Wire {
    guard let frame = try Wire.decode(input).object, let op = frame["op"]?.string else {
      throw HoyaError.invalidResponse
    }
    let key = frame["args"]?.object?["key"]?.string
    operations.append(key.map { "\(op):\($0)" } ?? op)
    switch op {
    case "reconcile_resolving":
      if let key, conflicts.contains(key) { throw HoyaError.core("RollbackConflict") }
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
      return .object([
        "envelopes": .array([]), "next_due_ms": .null, "has_more": .boolean(false),
      ])
    case "drain": return .object(["events": .array([]), "has_more": .boolean(false)])
    default: return .null
    }
  }

  func count(_ operation: String) -> Int { operations.filter { $0 == operation }.count }
}

private final class Counter: @unchecked Sendable {
  private let lock = NSLock()
  private var items: [String] = []
  func add(_ item: String) {
    lock.lock()
    items.append(item)
    lock.unlock()
  }
  func count(_ item: String) -> Int {
    lock.lock()
    defer { lock.unlock() }
    return items.filter { $0 == item }.count
  }
  var total: Int {
    lock.lock()
    defer { lock.unlock() }
    return items.count
  }
}

final class GuardGW16bTests: XCTestCase, @unchecked Sendable {
  private struct Observed {
    let prepares: Int
    let reconcilesX: Int
    let reconcilesY: Int
    let callbacksX: Int
    let errors: Int
    let reports: Int
    let settled: Bool
  }

  private func run(pending: [String], conflicts: Set<String>) async throws -> Observed {
    let runtime = PassRuntime(pending: pending, conflicts: conflicts)
    let reports = Counter()
    let db = Database(runtime: runtime, onBackgroundError: { reports.add($0.kind) })
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["sensors": Table(columns: ["label": Column(.string)])]),
      namespace: "40404040404040404040404040400060")
    let callbacks = Counter()
    let errors = Counter()
    try await db.startSync(
      transport: { _ in "[]" },
      reconcile: { pending in
        callbacks.add(pending.key)
        return ReconcilePlan()
      }, now: { 10 }, jitter: { 0 }, onError: { errors.add("\($0)") })
    try await Task.sleep(nanoseconds: 400_000_000)
    let prepares = await runtime.count("prepare_push")
    let reconcilesX = await runtime.count("reconcile_resolving:X")
    let reconcilesY = await runtime.count("reconcile_resolving:Y")
    try await Task.sleep(nanoseconds: 200_000_000)
    let later = await runtime.count("prepare_push")
    await db.stopSync()
    try await db.close()
    return Observed(
      prepares: prepares, reconcilesX: reconcilesX, reconcilesY: reconcilesY,
      callbacksX: callbacks.count("X"), errors: errors.total, reports: reports.total,
      settled: later == prepares)
  }

  func testGW16bOnlyConflictingXIsTriedOnceThenIdle() async throws {
    let result = try await run(pending: ["X"], conflicts: ["X"])
    XCTAssertEqual(result.callbacksX, 1)
    XCTAssertEqual(result.reconcilesX, 1)
    XCTAssertEqual(result.errors, 1)
    XCTAssertEqual(result.reports, 1)
    XCTAssertEqual(result.prepares, 1, "no additional pass")
    XCTAssertTrue(result.settled, "no later activity")
  }

  func testGW16bConflictingXWithSuccessfulYGetsExactlyOneFurtherPass() async throws {
    let result = try await run(pending: ["X", "Y"], conflicts: ["X"])
    XCTAssertEqual(result.reconcilesY, 1)
    // 0e6455d: the driver's successful reconcile routes through Database.execute and wakes it.
    XCTAssertEqual(result.callbacksX, 2)
    XCTAssertEqual(result.reconcilesX, 2)
    XCTAssertEqual(result.errors, 2)
    XCTAssertEqual(result.reports, 2)
    XCTAssertEqual(result.prepares, 2, "exactly one further pass")
    XCTAssertTrue(result.settled, "no later activity")
  }
}
