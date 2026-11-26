// TC-0036 G07/G09: real queue, frozen retry and explicit application plan.
import Foundation
import XCTest

@testable import HoyaDB

final class SyncTests: XCTestCase, @unchecked Sendable {
  func testFrozenRetryReceiptAndExplicitGuardedMapping() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "82828282828282828282828282828282")
    let command = try await db.command(
      CommandDefinition("draft-create-v1"), value: .object([:]),
      context: .init(identity: .string("trusted"), nowMilliseconds: 1000)
    ) { scope in
      _ = try await scope.insert(
        "drafts", fields: ["body": .string("offline")], proposedID: "local:draft")
      return .integer(Int64.max)
    }
    let queued = try await db.queueStatus()
    XCTAssertEqual(queued.pending, 1)
    XCTAssertEqual(queued.entries.first?.key, command.key)
    let first = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    XCTAssertEqual(first.envelopes.count, 1)
    let early = try await db.preparePush(nowMilliseconds: 1002, jitterBasisPoints: 0)
    XCTAssertEqual(early.envelopes, [])
    let due = try XCTUnwrap(early.nextDueMilliseconds)
    let retry = try await db.preparePush(nowMilliseconds: due, jitterBasisPoints: 0)
    XCTAssertEqual(retry.envelopes, first.envelopes)
    let remote = RemoteOutcome.applied(
      .object(["large": .integer(Int64.max), "ratio": .float(42)]))
    try await db.receive(command.key, outcome: remote)
    let pending = try await db.pendingReconciliations()
    XCTAssertEqual(pending, [PendingReconciliation(key: command.key, outcome: remote)])
    let received = try await db.queueStatus()
    XCTAssertTrue(received.entries.first?.awaitingLocal == true)
    try await db.reconcile(
      command.key, plan: ReconcilePlan(mappings: ["local:draft": "server:draft"]))
    let rows = try await db.fetch(Query("drafts"))
    XCTAssertEqual(rows.map(\.id), ["server:draft"])
    let final = try await db.queueStatus()
    XCTAssertEqual(final.live, 0)
    try await db.close()
  }
}

private actor SyncGate {
  private var released = false
  private var waiting: CheckedContinuation<Void, Never>?
  func wait() async {
    if released { return }
    await withCheckedContinuation { waiting = $0 }
  }
  func release() {
    released = true
    waiting?.resume()
    waiting = nil
  }
}

private actor SyncCalls {
  var transports = 0
  var mappers = 0
  func sent() { transports += 1 }
  func mapped() { mappers += 1 }
}

private actor BusyReceiptRuntime: NativeCalling {
  private let busy: @Sendable () -> Void
  init(busy: @escaping @Sendable () -> Void) { self.busy = busy }
  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }
  func call(_ input: Data) async throws -> Wire {
    let value = try await NativeRuntime.shared.call(input)
    if try Wire.decode(input).object?["op"] == .string("receive_wire"),
      case .array(let reports) = value,
      reports.contains(where: { $0.object?["error"]?.object?["kind"] == .string("Busy") })
    {
      busy()
    }
    return value
  }
}

extension SyncTests {
  func testManagedSyncRetainsInflightResponseAcrossAdmittedScopeAndMapsOnce() async throws {
    let busy = expectation(description: "response retained while scope owns core")
    let sent = expectation(description: "opaque transport awaiting server")
    let mapped = expectation(description: "one application mapper")
    let runtime = BusyReceiptRuntime(busy: { busy.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "84848484848484848484848484848484")
    let command = try await db.command(
      CommandDefinition("sync-create-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { scope in
      _ = try await scope.insert(
        "drafts", fields: ["body": .string("offline")], proposedID: "local:sync")
      return .null
    }
    let gate = SyncGate()
    let calls = SyncCalls()
    try await db.startSync(
      transport: { body in
        await calls.sent()
        let batch = try await JSONValue.decodeJSON(body)
        guard case .array(let envelopes) = batch else { throw HoyaError.invalidResponse }
        XCTAssertEqual(envelopes.count, 1)
        sent.fulfill()
        await gate.wait()
        return try await JSONValue.array([
          .object([
            "idempotencyKey": .string(command.key), "status": .string("applied"),
            "result": .integer(Int64.max),
          ])
        ]).encodeJSON()
      },
      reconcile: { pending in
        await calls.mapped()
        XCTAssertEqual(pending.key, command.key)
        mapped.fulfill()
        return ReconcilePlan(mappings: ["local:sync": "server:sync"])
      }, now: { 1001 }, jitter: { 0 }, onError: { error in XCTFail("sync error: \(error)") })
    await fulfillment(of: [sent], timeout: 2)
    let ticket = try await db.execute("request")
    _ = try await db.execute(
      "begin",
      arguments: [
        "request": ticket, "definition": CommandDefinition("overlap-v1").wire,
        "value": JSONValue.object([:]).wire,
        "context": CapturedContext(identity: .null, nowMilliseconds: 1001).wire,
      ])
    await gate.release()
    await fulfillment(of: [busy], timeout: 2)
    _ = try await db.execute("abort", arguments: ["request": ticket])
    await fulfillment(of: [mapped], timeout: 2)
    // The mapper callback precedes its one guarded commit; synchronize through
    // the managed loop's own status observation rather than a fixed sleep.
    let applied = expectation(description: "terminal status delivered")
    let subscription = try await db.observeQueueStatus { value in
      if (try? value.get().live) == 0 { applied.fulfill() }
    }
    await fulfillment(of: [applied], timeout: 2)
    let rows = try await db.fetch(Query("drafts"))
    XCTAssertEqual(rows.map(\.id), ["server:sync"])
    let transports = await calls.transports
    let mappers = await calls.mappers
    XCTAssertEqual(transports, 1)
    XCTAssertEqual(mappers, 1)
    subscription.cancel()
    await db.stopSync()
    try await db.close()
  }
}

extension SyncTests {
  func testManagedOutcomeCarriesLosslessTerminalApplicationValue() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "89898989898989898989898989898989")
    let command = try await db.command(
      CommandDefinition("outcome-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { _ in .null }
    let initial = expectation(description: "typed queued outcome")
    let terminal = expectation(description: "typed applied i64/float outcome")
    let payload = JSONValue.object(["large": .integer(Int64.max), "ratio": .float(42)])
    let sub = try await db.observeOutcome(command.key) { value in
      guard case .success(let outcome) = value else {
        XCTFail("outcome delivery failed")
        return
      }
      if outcome == .queued { initial.fulfill() }
      if outcome == .applied(payload) { terminal.fulfill() }
    }
    await fulfillment(of: [initial], timeout: 2)
    _ = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    try await db.receive(command.key, outcome: .applied(payload))
    try await db.reconcile(command.key, plan: ReconcilePlan())
    await fulfillment(of: [terminal], timeout: 2)
    sub.cancel()
    try await db.close()
  }
}

private actor DropSyncRuntime: NativeCalling {
  private let closed: @Sendable () -> Void
  private(set) var owner: String?
  init(closed: @escaping @Sendable () -> Void) { self.closed = closed }
  func open(_ input: Data, key: Data) async throws -> Wire {
    let value = try await NativeRuntime.shared.open(input, key: key)
    owner = value.string
    return value
  }
  func call(_ input: Data) async throws -> Wire {
    let value = try await NativeRuntime.shared.call(input)
    if try Wire.decode(input).object?["op"] == .string("close") { closed() }
    return value
  }
}

extension SyncTests {
  func testDroppingOwnerClosesNativeHandleWhileTransportRemainsAwaiting() async throws {
    let closed = expectation(description: "native owner disposed before network resumes")
    let sent = expectation(description: "transport holds owned envelopes")
    let gate = SyncGate()
    let runtime = DropSyncRuntime(closed: { closed.fulfill() })
    var db: Database? = Database(runtime: runtime)
    try await db!.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "94949494949494949494949494949494")
    _ = try await db!.command(
      CommandDefinition("drop-sync-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { _ in .null }
    try await db!.startSync(
      transport: { _ in
        sent.fulfill()
        await gate.wait()
        return "[]"
      }, now: { 1001 }, jitter: { 0 }, onError: { _ in XCTFail("retired sync delivered error") })
    await fulfillment(of: [sent], timeout: 2)
    let recordedOwner = await runtime.owner
    let owner = try XCTUnwrap(recordedOwner)
    db = nil
    await fulfillment(of: [closed], timeout: 2)
    do {
      _ = try await NativeRuntime.shared.call(
        Wire.frame("schema", owner: owner, args: [:]).encoded())
      XCTFail("network task retained usable database owner")
    } catch { XCTAssertEqual(error as? HoyaError, .core("InvalidHandle")) }
    await gate.release()
  }
}

extension SyncTests {
  func testPreparePushAutomaticallyDeliversSentOutcomeWithoutReceiptOrManualDrain() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "95959595959595959595959595959595")
    let command = try await db.command(
      CommandDefinition("sent-observe-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { _ in .null }
    let queued = expectation(description: "initial queued before prepare")
    let sent = expectation(description: "sent without another operation")
    let subscription = try await db.observeOutcome(command.key) { result in
      guard case .success(let outcome) = result else {
        XCTFail("outcome failed")
        return
      }
      if outcome == .queued { queued.fulfill() }
      if outcome == .sent { sent.fulfill() }
    }
    await fulfillment(of: [queued], timeout: 2)
    _ = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    await fulfillment(of: [sent], timeout: 2)
    subscription.cancel()
    try await db.close()
  }
}

// DEF-0017: suspend only the owned successful reply, after real native work.
private actor HeldSyncReplyRuntime: NativeCalling {
  private let operation: String
  private let held: @Sendable () -> Void
  private var intercepted = false
  private var continuation: CheckedContinuation<Void, Never>?

  init(operation: String, held: @escaping @Sendable () -> Void) {
    self.operation = operation
    self.held = held
  }

  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }

  func call(_ input: Data) async throws -> Wire {
    let reply = try await NativeRuntime.shared.call(input)
    if !intercepted, try Wire.decode(input).object?["op"] == .string(operation) {
      intercepted = true
      await withCheckedContinuation { continuation in
        self.continuation = continuation
        held()
      }
    }
    return reply
  }

  func release() {
    continuation?.resume()
    continuation = nil
  }
}

extension SyncTests {
  func testStoppedDriverDoesNotStartOldTransportAfterSuccessfulPrepareReply() async throws {
    let held = expectation(description: "real Sent prepare reply held")
    let oldTransport = expectation(description: "retired transport must not start")
    oldTransport.isInverted = true
    let oldError = expectation(description: "retired driver must not report errors")
    oldError.isInverted = true
    let currentTransport = expectation(description: "current driver retries at durable due")
    let runtime = HeldSyncReplyRuntime(operation: "prepare_push", held: { held.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "96969696969696969696969696969696")
    let command = try await db.command(
      CommandDefinition("retired-prepare-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { _ in .integer(Int64.max) }
    try await db.startSync(
      transport: { _ in
        oldTransport.fulfill()
        return "[]"
      },
      now: { 1001 }, jitter: { 0 }, onError: { _ in oldError.fulfill() })
    await fulfillment(of: [held], timeout: 2)
    await db.stopSync()
    let prepared = try await db.queueStatus()
    let entry = try XCTUnwrap(prepared.entries.first)
    XCTAssertEqual(entry.key, command.key)
    XCTAssertEqual(entry.state, .sent)
    XCTAssertEqual(entry.attempts, 1)
    let due = try XCTUnwrap(entry.nextDueMilliseconds)
    // Replacement at the same time cannot resend the already committed Sent.
    try await db.startSync(
      transport: { _ in
        currentTransport.fulfill()
        return "[]"
      },
      now: { 1001 }, jitter: { 0 }, onError: { error in XCTFail("current sync: \(error)") })
    await runtime.release()
    await fulfillment(of: [oldTransport, oldError], timeout: 0.15)
    let retained = try await db.queueStatus()
    XCTAssertEqual(retained.entries.first?.attempts, 1)
    await db.stopSync()
    try await db.startSync(
      transport: { _ in
        currentTransport.fulfill()
        return "[]"
      },
      now: { due }, jitter: { 0 }, onError: { error in XCTFail("current retry: \(error)") })
    await fulfillment(of: [currentTransport], timeout: 2)
    await db.stopSync()
    try await db.close()
  }

  func testStoppedDriverDoesNotStartOldMapperAfterSuccessfulPendingReply() async throws {
    let held = expectation(description: "real pending reconciliation reply held")
    let oldMapper = expectation(description: "retired mapper must not start")
    oldMapper.isInverted = true
    let oldError = expectation(description: "retired mapper driver must not report errors")
    oldError.isInverted = true
    let currentMapper = expectation(description: "replacement mapper receives retained outcome")
    let runtime = HeldSyncReplyRuntime(
      operation: "pending_reconciliations", held: { held.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["drafts": Table(columns: ["body": Column(.string)])]),
      namespace: "97979797979797979797979797979797")
    let command = try await db.command(
      CommandDefinition("retired-mapper-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { _ in .null }
    _ = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    let outcome = RemoteOutcome.applied(.integer(Int64.max))
    try await db.receive(command.key, outcome: outcome)
    try await db.startSync(
      transport: { _ in
        XCTFail("received work must not retransmit")
        return "[]"
      },
      reconcile: { _ in
        oldMapper.fulfill()
        return ReconcilePlan()
      },
      now: { 1002 }, jitter: { 0 }, onError: { _ in oldError.fulfill() })
    await fulfillment(of: [held], timeout: 2)
    await db.stopSync()
    try await db.startSync(
      transport: { _ in
        XCTFail("replacement must not retransmit outcome")
        return "[]"
      },
      reconcile: { pending in
        XCTAssertEqual(pending.key, command.key)
        XCTAssertEqual(pending.outcome, outcome)
        currentMapper.fulfill()
        return ReconcilePlan()
      }, now: { 1002 }, jitter: { 0 }, onError: { error in XCTFail("replacement: \(error)") })
    await runtime.release()
    await fulfillment(of: [currentMapper], timeout: 2)
    await fulfillment(of: [oldMapper, oldError], timeout: 0.15)
    // Query the actual terminal event rather than assuming the callback committed.
    let applied = expectation(description: "replacement reconciliation durable")
    let subscription = try await db.observeOutcome(command.key) { value in
      if (try? value.get()) == .applied(.integer(Int64.max)) { applied.fulfill() }
    }
    await fulfillment(of: [applied], timeout: 2)
    subscription.cancel()
    await db.stopSync()
    try await db.close()
  }
}
