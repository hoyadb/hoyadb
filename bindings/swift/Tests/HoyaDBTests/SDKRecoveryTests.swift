// SPEC-0038 / TC-0042 SDK1-2: scripted NativeCalling proves SDK translation only.
import Foundation
import XCTest

@testable import HoyaDB

private func recoveryEntry(_ awaiting: Wire = .string("A")) -> Wire {
  .object([
    "kind": .string("command"), "table": .null, "record_id": .null, "key": .string("B"),
    "state": .string("queued"), "blocked_on": .array([]), "awaiting_local": .boolean(false),
    "awaiting_key": awaiting, "rolled_back": .boolean(false), "attempts": .string("0"),
    "next_due_ms": .null,
  ])
}

private func recoveryStatus() -> Wire {
  .object([
    "pending": .string("1"), "sending": .string("0"), "awaiting_outcome": .string("0"),
    "live": .string("1"), "blocked": .boolean(false), "commands": .array([recoveryEntry()]),
  ])
}

private actor RecoverySDKRuntime: NativeCalling {
  var frames: [Wire] = []
  var failure: String?
  var ready = false
  var prepares = 0
  var statusDelivered = false
  let frozen = "{\"idempotencyKey\":\"A\",\"args\":{\"value\":42.0}}"

  init(failure: String? = nil) { self.failure = failure }
  func open(_ input: Data, key: Data) async throws -> Wire { .string("1") }
  func call(_ input: Data) async throws -> Wire {
    let frame = try Wire.decode(input)
    frames.append(frame)
    switch frame.object?["op"]?.string {
    case "queue_status": return recoveryStatus()
    case "observe_status": return .string("11")
    case "drain":
      let events: [Wire] =
        statusDelivered
        ? []
        : [
          .object([
            "kind": .string("status"), "subscription": .string("11"), "value": recoveryStatus(),
          ])
        ]
      statusDelivered = true
      return .object(["events": .array(events), "has_more": .boolean(false)])
    case "request_redelivery":
      if let failure { throw HoyaError.core(failure) }
      ready = true
      return .null
    case "prepare_push":
      prepares += 1
      let envelopes: [Wire] = ready ? [.string(frozen)] : []
      ready = false
      return .object([
        "envelopes": .array(envelopes), "next_due_ms": .null, "has_more": .boolean(false),
      ])
    case "receive_wire": return .array([])
    default: return .null
    }
  }
}

private actor RecoverySDKSent {
  var bodies: [String] = []
  func record(_ body: String) { bodies.append(body) }
}

final class SDKRecoveryTests: XCTestCase, @unchecked Sendable {
  private func open(_ runtime: RecoverySDKRuntime) async throws -> Database {
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["items": Table(columns: [:])]),
      namespace: String(repeating: "00", count: 16))
    addTeardownBlock { try await db.close() }
    return db
  }

  func testElevenFieldEntryDecodesBlockerAndAbsence() throws {
    XCTAssertEqual(try QueueEntry(wire: recoveryEntry()).awaitingKey, "A")
    XCTAssertNil(try QueueEntry(wire: recoveryEntry(.null)).awaitingKey)
  }

  func testEntryRejectsMalformedMissingAndUnknownFields() throws {
    XCTAssertThrowsError(try QueueEntry(wire: recoveryEntry(.boolean(false))))
    var fields = try XCTUnwrap(recoveryEntry().object)
    fields.removeValue(forKey: "awaiting_key")
    XCTAssertThrowsError(try QueueEntry(wire: .object(fields)))
    fields["awaiting_key"] = .null
    fields["unexpected"] = .null
    XCTAssertThrowsError(try QueueEntry(wire: .object(fields)))
  }

  func testQueueFetchExposesBlocker() async throws {
    let runtime = RecoverySDKRuntime()
    let db = try await open(runtime)
    let fetched = try await db.queueStatus()
    XCTAssertEqual(fetched.entries.first?.awaitingKey, "A")
    try await db.close()
  }

  func testStatusObservationUsesBlockerDecoder() async throws {
    let runtime = RecoverySDKRuntime()
    let db = try await open(runtime)
    let delivered = expectation(description: "status with blocker")
    let subscription = try await db.observeQueueStatus(
      executor: { work in work() },
      callback: { result in
        switch result {
        case .success(let status): XCTAssertEqual(status.entries.first?.awaitingKey, "A")
        case .failure(let error): XCTFail("status decode failed: \(error)")
        }
        delivered.fulfill()
      })
    try await db.drainNotifications()
    await fulfillment(of: [delivered], timeout: 2)
    withExtendedLifetime(subscription) {}
    try await db.close()
  }

  func testExplicitRedeliveryEncodesOnceWithoutStartingTransport() async throws {
    let runtime = RecoverySDKRuntime()
    let db = try await open(runtime)
    try await db.requestRedelivery("A", nowMilliseconds: 253_402_300_799_999)
    let frames = await runtime.frames
    let requests = frames.filter { $0.object?["op"]?.string == "request_redelivery" }
    XCTAssertEqual(requests.count, 1)
    XCTAssertEqual(
      requests.first?.object?["args"],
      .object(["key": .string("A"), "now_ms": .string("253402300799999")]))
    XCTAssertEqual(requests.first?.object?["owner"], .string("1"))
    XCTAssertFalse(frames.contains { $0.object?["op"]?.string == "prepare_push" })
    try await db.close()
  }

  func testRedeliveryCoreFailuresAreReturnedWithoutRetry() async throws {
    for kind in ["Protocol", "Busy", "InvalidCommand"] {
      let runtime = RecoverySDKRuntime(failure: kind)
      let db = try await open(runtime)
      do {
        try await db.requestRedelivery("A", nowMilliseconds: -1)
        XCTFail("rearm error was lost")
      } catch {
        XCTAssertEqual(error as? HoyaError, .core(kind))
      }
      let requests = await runtime.frames.filter {
        $0.object?["op"]?.string == "request_redelivery"
      }
      XCTAssertEqual(requests.count, 1)
      try await db.close()
    }
  }

  func testRedeliveryWakesParkedDriverAndSendsOriginalBytesOnce() async throws {
    let runtime = RecoverySDKRuntime()
    let db = try await open(runtime)
    let sent = RecoverySDKSent()
    try await db.startSync(
      transport: { body in
        await sent.record(body)
        return "[]"
      }, now: { 10 }, jitter: { 0 })
    for _ in 0..<1_000 {
      if await runtime.prepares > 0 { break }
      await Task.yield()
    }
    let initial = await runtime.prepares
    XCTAssertGreaterThan(initial, 0)
    try await db.requestRedelivery("A", nowMilliseconds: 10)
    for _ in 0..<200 {
      if await !sent.bodies.isEmpty { break }
      try await Task.sleep(nanoseconds: 10_000_000)
    }
    let bodies = await sent.bodies
    let frozen = runtime.frozen
    XCTAssertEqual(bodies, ["[" + frozen + "]"])
    try await db.close()
  }
}
