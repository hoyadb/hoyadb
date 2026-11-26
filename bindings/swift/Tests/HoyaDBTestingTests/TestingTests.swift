// ST01–06 / TC-0036: ordinary imports only, actual native Rust owner.
import Foundation
import HoyaDB
import HoyaDBTesting
import XCTest

private final class TimingProbe: @unchecked Sendable {
  private let lock = NSLock()
  private var clock: UInt64 = 0
  private var next = 0
  private var jobs: [(Int, UInt64, @Sendable () -> Void)] = []
  private let scheduled: @Sendable () -> Void

  init(scheduled: @escaping @Sendable () -> Void = {}) { self.scheduled = scheduled }

  func now() -> UInt64 {
    lock.lock()
    defer { lock.unlock() }
    return clock
  }
  func set(_ value: UInt64) {
    lock.lock()
    clock = value
    lock.unlock()
  }
  var delays: [UInt64] {
    lock.lock()
    defer { lock.unlock() }
    return jobs.map { $0.1 }
  }
  func schedule(_ delay: UInt64, _ work: @escaping @Sendable () -> Void) -> any TimerCancellation {
    lock.lock()
    next += 1
    let id = next
    jobs.append((id, delay, work))
    lock.unlock()
    scheduled()
    return ProbeTicket { [weak self] in self?.cancel(id) }
  }
  private func cancel(_ id: Int) {
    lock.lock()
    jobs.removeAll { $0.0 == id }
    lock.unlock()
  }
  func fire() {
    lock.lock()
    let work = jobs.isEmpty ? nil : jobs.removeFirst().2
    lock.unlock()
    work?()
  }
}

private struct ProbeTicket: TimerCancellation {
  let work: @Sendable () -> Void
  func cancel() { work() }
}

private final class Counts: @unchecked Sendable {
  private let lock = NSLock()
  private var counts: [UInt64] = []
  func append(_ value: UInt64) {
    lock.lock()
    counts.append(value)
    lock.unlock()
  }
  var values: [UInt64] {
    lock.lock()
    defer { lock.unlock() }
    return counts
  }
}

private final class IDs: @unchecked Sendable {
  private let lock = NSLock()
  private var remaining = ["test-fixed-id-001"]
  func next() -> String? {
    lock.lock()
    defer { lock.unlock() }
    return remaining.isEmpty ? nil : remaining.removeFirst()
  }
}

private actor DropFirst {
  private var first = true
  func consume() -> Bool {
    defer { first = false }
    return first
  }
}

private actor DelayGate {
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

final class TestingTests: XCTestCase, @unchecked Sendable {
  private var schema: Schema {
    Schema(version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
  }

  func testST01PublicTimingInjectionControlsManagedCount() async throws {
    let deadline = expectation(description: "injected scheduler owns count deadline")
    let timing = TimingProbe(scheduled: { deadline.fulfill() })
    let counts = Counts()
    let db = Database(monotonicClock: { timing.now() }, timerScheduler: { timing.schedule($0, $1) })
    try await db.openMemory(schema: schema, namespace: "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1")
    let initial = expectation(description: "initial count0")
    let changed = expectation(description: "manual deadline count1")
    let sub = try await db.observe(Query("notes"), mode: .count) { result in
      guard case .success(.count(let value)) = result else {
        XCTFail("typed count")
        return
      }
      counts.append(value)
      if value == 0 { initial.fulfill() }
      if value == 1 { changed.fulfill() }
    }
    await fulfillment(of: [initial], timeout: 2)
    _ = try await db.insert("notes", fields: ["body": .string("one")])
    await fulfillment(of: [deadline], timeout: 2)
    XCTAssertEqual(timing.delays, [16], "public injected timer must own the count deadline")
    XCTAssertEqual(counts.values, [0])
    timing.set(16)
    timing.fire()
    await fulfillment(of: [changed], timeout: 2)
    XCTAssertEqual(counts.values, [0, 1])
    sub.cancel()
    try await db.close()
  }

  func testST02RealMemoryFactoryIDExhaustionAndRetainedMigration() async throws {
    let clock = ManualClock(1234)
    let ids = IDs()
    let store = try await MemoryStore.create()
    let v1 = Schema(
      version: 1,
      tables: ["notes": Table(columns: ["body": Column(.string), "at": Column(.timestamp)])])
    let db = try await MemoryTesting.open(
      schema: v1, namespace: "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2", store: store,
      idGenerator: { ids.next() })
    let first = try await db.insert(
      "notes", fields: ["body": .string("retained"), "at": .timestamp(clock.now())])
    XCTAssertEqual(first.id, "test-fixed-id-001")
    XCTAssertEqual(first.fields["at"], .timestamp(1234))
    do {
      _ = try await db.insert("notes", fields: first.fields)
      XCTFail("exhaustion must reject")
    } catch { XCTAssertEqual(error as? HoyaError, .invalidRequest) }
    let before = try await db.fetch(Query("notes"))
    XCTAssertEqual(before, [first])
    try await db.close()
    let v2 = Schema(
      version: 2,
      tables: [
        "notes": Table(columns: [
          "body": Column(.string), "at": Column(.timestamp), "tag": Column(.string, optional: true),
        ])
      ])
    let migration = Migration(
      fromVersion: 1, to: v2,
      steps: [
        .addColumn(
          table: "notes", name: "tag", column: Column(.string, optional: true), defaultValue: nil)
      ])
    let reopened = try await MemoryTesting.open(
      schema: v2, namespace: "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2", store: store,
      migrations: [migration])
    let after = try await reopened.fetch(Query("notes"))
    XCTAssertEqual(after, [first])
    let actualSchema = try await reopened.schema()
    XCTAssertEqual(actualSchema, v2)
    let protection = try await reopened.protection()
    XCTAssertFalse(protection.encrypted)
    XCTAssertEqual(protection.cipher, "none")
    try await reopened.close()
    try await store.dispose()
  }

  func testST03ManualClocksOverflowAndTimerCallbacksOutsideLock() throws {
    let utc = ManualClock(Int64.max)
    XCTAssertThrowsError(try utc.advance(by: 1))
    XCTAssertEqual(utc.now(), Int64.max)
    utc.set(10)
    try utc.advance(by: 5)
    XCTAssertEqual(utc.now(), 15)
    let uptime = ManualUptime(UInt64.max)
    XCTAssertThrowsError(try uptime.advance(by: 1))
    XCTAssertEqual(uptime.now(), UInt64.max)
    uptime.set(12)
    try uptime.advance(by: 4)
    XCTAssertEqual(uptime.now(), 16)
    let timer = ManualTimer()
    let values = Counts()
    let cancelled = timer.schedule(7) { values.append(99) }
    cancelled.cancel()
    cancelled.cancel()
    _ = timer.schedule(16) {
      values.append(1)
      // Reentrant scheduling would deadlock if the timer still held its lock.
      _ = timer.schedule(3) { values.append(2) }
    }
    XCTAssertEqual(timer.pendingDelays, [16])
    XCTAssertTrue(timer.fireNext())
    XCTAssertEqual(values.values, [1])
    XCTAssertEqual(timer.pendingDelays, [3])
    XCTAssertTrue(timer.fireNext())
    XCTAssertEqual(values.values, [1, 2])
    XCTAssertFalse(timer.fireNext())
  }

  func testST04TypedCommandResultsReverseByExactKey() async throws {
    let db = try await MemoryTesting.open(
      schema: schema, namespace: "b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4")
    let payload = JSONValue.object([
      "large": .integer(Int64.max), "ratio": .float(42), "label": .string("東京\0🌿"),
    ])
    let definition: [String: CommandDefinition.Argument] = [
      "large": .init(.integer), "ratio": .init(.float), "label": .init(.string),
    ]
    let context = CapturedContext(identity: .string("trusted"), nowMilliseconds: 1000)
    let first = try await db.command(
      CommandDefinition("accept-v1", arguments: definition), value: payload, context: context
    ) { _ in payload }
    let second = try await db.command(
      CommandDefinition("reject-v1", arguments: definition), value: payload, context: context
    ) { _ in .null }
    let fake = FakeTransport(
      handlers: [
        "accept-v1": { envelope in
          guard case .object(let fields) = envelope else {
            throw FakeTransportError.invalidEnvelope
          }
          XCTAssertEqual(fields["args"], payload)
          return .applied(payload)
        },
        "reject-v1": { _ in .rejected("fixture refusal") },
      ], reverseReplies: true)
    let batch = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    XCTAssertEqual(batch.envelopes.count, 2)
    let response = try await fake.push("[" + batch.envelopes.joined(separator: ",") + "]")
    guard case .array(let replies) = try await JSONValue.decodeJSON(response), replies.count == 2,
      case .object(let last) = replies[0], case .object(let accepted) = replies[1]
    else {
      XCTFail("keyed replies")
      return
    }
    XCTAssertEqual(last["idempotencyKey"], .string(second.key))
    XCTAssertEqual(accepted["idempotencyKey"], .string(first.key))
    XCTAssertEqual(accepted["result"], payload)
    _ = try await db.receiveWire(response)
    let pending = try await db.pendingReconciliations()
    XCTAssertTrue(pending.contains { $0.key == first.key && $0.outcome == .applied(payload) })
    let status = try await db.queueStatus()
    XCTAssertEqual(status.entries.first { $0.key == second.key }?.state, .rejected)
    try await db.reconcile(first.key, plan: ReconcilePlan())
    let final = try await db.queueStatus()
    XCTAssertEqual(final.entries.first { $0.key == first.key }?.state, .applied)
    try await db.close()
  }

  func testST05EntityRepliesKeepDistinctWriteIDsForSameCanonicalRecord() async throws {
    let clock = ManualClock(1000)
    let db = try await MemoryTesting.open(
      schema: schema, namespace: "b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5")
    try await db.configureEntities(scope: "account", tables: ["notes"]) {
      .init(identity: .null, nowMilliseconds: clock.now())
    }
    let inserted = try await db.entityBatch([
      .insert(table: "notes", fields: ["body": .string("created")])
    ])
    let local = try XCTUnwrap(inserted.first?.after?.id)
    let create = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    XCTAssertEqual(create.envelopes.count, 1)
    let initialStatus = try await db.queueStatus()
    let createKey = try XCTUnwrap(initialStatus.entries.first?.key)
    try await db.receiveEntity(
      table: "notes", recordID: local, writeID: createKey, outcome: .applied(.null))
    try await db.reconcile(createKey, plan: ReconcilePlan(mappings: [local: "server:note"]))
    clock.set(1002)
    _ = try await db.patch("notes", id: "server:note", fields: ["body": .string("first")])
    let one = try await db.preparePush(nowMilliseconds: 1003, jitterBasisPoints: 0)
    let oneStatus = try await db.queueStatus()
    let oneKey = try XCTUnwrap(oneStatus.entries.first { $0.state == .sent }?.key)
    try await db.receiveEntity(
      table: "notes", recordID: "server:note", writeID: oneKey, outcome: .applied(.null))
    try await db.reconcile(oneKey, plan: ReconcilePlan())
    clock.set(1004)
    _ = try await db.patch("notes", id: "server:note", fields: ["body": .string("second")])
    let two = try await db.preparePush(nowMilliseconds: 1005, jitterBasisPoints: 0)
    let twoStatus = try await db.queueStatus()
    let twoKey = try XCTUnwrap(twoStatus.entries.first { $0.state == .sent }?.key)
    XCTAssertNotEqual(oneKey, twoKey)
    let fake = FakeTransport(
      entityHandlers: ["notes": { _ in .applied(.null) }], reverseReplies: true)
    let response = try await fake.push(
      "[" + (one.envelopes + two.envelopes).joined(separator: ",") + "]")
    guard case .array(let replies) = try await JSONValue.decodeJSON(response), replies.count == 2,
      case .object(let newest) = replies[0], case .object(let older) = replies[1]
    else {
      XCTFail("entity replies")
      return
    }
    XCTAssertEqual(newest["writeId"], .string(twoKey))
    XCTAssertEqual(older["writeId"], .string(oneKey))
    XCTAssertEqual(newest["recordId"], .string("server:note"))
    XCTAssertEqual(older["recordId"], .string("server:note"))
    XCTAssertEqual(newest["table"], .string("notes"))
    let received = try await db.receiveWire(response)
    guard case .array(let reports) = received else {
      XCTFail("production correlation reports")
      return
    }
    for report in reports {
      guard case .object(let fields) = report else {
        XCTFail("report")
        return
      }
      XCTAssertEqual(fields["ok"], .boolean(true))
    }
    try await db.reconcile(twoKey, plan: ReconcilePlan())
    try await db.close()
  }

  func testST06DelayedDroppedACKRetriesOriginalFrozenBytesAtCoreDeadline() async throws {
    let db = try await MemoryTesting.open(
      schema: schema, namespace: "b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6")
    let command = try await db.command(
      CommandDefinition("retry-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 1000)
    ) { _ in .null }
    let batch = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
    let body = "[" + batch.envelopes.joined(separator: ",") + "]"
    let delayed = expectation(description: "fake reply held")
    let gate = DelayGate()
    let drop = DropFirst()
    let delayFirst = DropFirst()
    let fake = FakeTransport(
      handlers: ["retry-v1": { _ in .applied(.integer(Int64.max)) }],
      delay: {
        let first = await delayFirst.consume()
        if first {
          delayed.fulfill()
          await gate.wait()
        }
      }, dropAcknowledgement: { _ in await drop.consume() })
    let first = Task { try await fake.push(body) }
    await fulfillment(of: [delayed], timeout: 2)
    let held = try await db.queueStatus()
    XCTAssertEqual(held.entries.first?.state, .sent)
    await gate.release()
    do {
      _ = try await first.value
      XCTFail("first ACK is dropped")
    } catch { XCTAssertEqual(error as? FakeTransportError, .droppedAcknowledgement) }
    let sent = try await db.queueStatus()
    let due = try XCTUnwrap(sent.entries.first?.nextDueMilliseconds)
    let retry = try await db.preparePush(nowMilliseconds: due, jitterBasisPoints: 0)
    XCTAssertEqual(retry.envelopes, batch.envelopes)
    let response = try await fake.push("[" + retry.envelopes.joined(separator: ",") + "]")
    _ = try await db.receiveWire(response)
    try await db.reconcile(command.key, plan: ReconcilePlan())
    let terminal = try await db.queueStatus()
    XCTAssertEqual(terminal.live, 0)
    let pushes = await fake.pushes
    XCTAssertEqual(pushes.count, 2)
    XCTAssertEqual(pushes[0], pushes[1])
    try await db.close()
  }
}
