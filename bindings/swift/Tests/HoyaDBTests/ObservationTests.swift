// TC-0036 G05/G10: actual Rust initial event, no manual drain or application write.
import Foundation
import XCTest

@testable import HoyaDB

final class ObservationTests: XCTestCase, @unchecked Sendable {
  func testInitialSnapshotRunsOnSelectedExecutorWithoutWriteOrManualDrain() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])]),
      namespace: "58585858585858585858585858585858")
    let query: ProtocolValue = .object([
      "table": .string("comets"), "predicates": .array([]), "order": .null,
      "limit": .null, "source": .null,
    ])
    let initial = expectation(description: "selected executor initial count")
    let queue = DispatchQueue(label: "dev.hoyadb.test.selected-executor")
    let subscription = try await db.observe(
      query, mode: .count, executor: { work in queue.async(execute: work) },
      callback: { value in
        dispatchPrecondition(condition: .onQueue(queue))
        guard case .success(let value) = value else {
          XCTFail("observation failed: \(value)")
          initial.fulfill()
          return
        }
        XCTAssertEqual(value.object?["kind"], .string("data"))
        XCTAssertEqual(value.object?["value"]?.object?["kind"], .string("count"))
        XCTAssertEqual(value.object?["value"]?.object?["count"], .string("0"))
        initial.fulfill()
      })
    await fulfillment(of: [initial], timeout: 2)
    subscription.cancel()
    try await db.close()
  }
}

private final class ManualTiming: @unchecked Sendable {
  private let lock = NSLock()
  private var milliseconds: UInt64 = 0
  private var next = 0
  private var jobs: [Int: @Sendable () -> Void] = [:]
  private var delays: [UInt64] = []

  var now: UInt64 {
    lock.lock()
    defer { lock.unlock() }
    return milliseconds
  }

  func set(_ value: UInt64) {
    lock.lock()
    milliseconds = value
    lock.unlock()
  }

  var scheduled: [UInt64] {
    lock.lock()
    defer { lock.unlock() }
    return delays
  }

  func schedule(_ delay: UInt64, _ work: @escaping @Sendable () -> Void) -> any TimerCancellation {
    lock.lock()
    next += 1
    let id = next
    jobs[id] = work
    delays.append(delay)
    lock.unlock()
    return ManualTicket { [weak self] in self?.remove(id) }
  }

  func fireFirst() {
    lock.lock()
    let id = jobs.keys.min()
    let work = id.flatMap { jobs.removeValue(forKey: $0) }
    lock.unlock()
    work?()
  }

  private func remove(_ id: Int) {
    lock.lock()
    jobs.removeValue(forKey: id)
    lock.unlock()
  }
}

private struct ManualTicket: TimerCancellation {
  let action: @Sendable () -> Void

  init(_ action: @escaping @Sendable () -> Void) { self.action = action }

  func cancel() { action() }
}

extension ObservationTests {
  func testOneDeadlineTimerIsNotPostponedByLaterWrites() async throws {
    let timing = ManualTiming()
    let db = Database(
      runtime: NativeRuntime.shared, monotonicClock: { timing.now },
      timerScheduler: { delay, work in timing.schedule(delay, work) })
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])]),
      namespace: "59595959595959595959595959595959")
    let query: ProtocolValue = .object([
      "table": .string("comets"), "predicates": .array([]), "order": .null,
      "limit": .null, "source": .null,
    ])
    let initial = expectation(description: "initial count at zero")
    let due = expectation(description: "current count at original deadline")
    let subscription = try await db.observe(query, mode: .count) { result in
      guard case .success(let event) = result else {
        XCTFail("delivery failure")
        return
      }
      switch event.object?["value"]?.object?["count"] {
      case .string("0"): initial.fulfill()
      case .string("3"): due.fulfill()
      default: XCTFail("count delivered before core deadline: \(event)")
      }
    }
    await fulfillment(of: [initial], timeout: 2)
    for step in 1...3 {
      timing.set(UInt64(step))
      _ = try await db.execute(
        "insert",
        arguments: [
          "table": .string("comets"),
          "fields": .object([
            "label": .object(["kind": .string("string"), "value": .string("comet")])
          ]),
          "proposed_id": .string("comet-\(step)"),
        ])
      await db.drainObserved()
    }
    XCTAssertEqual(timing.scheduled, [16])
    timing.set(16)
    timing.fireFirst()
    await fulfillment(of: [due], timeout: 2)
    XCTAssertEqual(timing.scheduled, [16])
    subscription.cancel()
    try await db.close()
  }
}

private actor SuspendedDrainRuntime: NativeCalling {
  private let started: @Sendable () -> Void
  private var suspended = false
  private var release: CheckedContinuation<Void, Never>?

  init(started: @escaping @Sendable () -> Void) { self.started = started }

  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }

  func call(_ input: Data) async throws -> Wire {
    let operation = try Wire.decode(input).object?["op"]?.string
    let value = try await NativeRuntime.shared.call(input)
    if operation == "drain" && !suspended {
      suspended = true
      await withCheckedContinuation { continuation in
        release = continuation
        started()
      }
    }
    return value
  }

  func resume() {
    release?.resume()
    release = nil
  }
}

extension ObservationTests {
  func testWriteDuringOwnedInitialDrainSchedulesFollowupWithoutAnotherWrite() async throws {
    let started = expectation(description: "actual native initial snapshot held by host transport")
    let runtime = SuspendedDrainRuntime(started: { started.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])]),
      namespace: "60606060606060606060606060606060")
    let query: ProtocolValue = .object([
      "table": .string("comets"), "predicates": .array([]), "order": .null,
      "limit": .null, "source": .null,
    ])
    let initial = expectation(description: "owned initial empty snapshot")
    let update = expectation(description: "followup complete current snapshot")
    let subscription = try await db.observe(query) { result in
      guard case .success(let event) = result,
        case .array(let rows) = event.object?["value"]?.object?["rows"]
      else {
        XCTFail("membership delivery failure")
        return
      }
      if rows.isEmpty {
        initial.fulfill()
      } else {
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows.first?.object?["id"], .string("after-initial"))
        update.fulfill()
      }
    }
    await fulfillment(of: [started], timeout: 2)
    _ = try await db.execute(
      "insert",
      arguments: [
        "table": .string("comets"),
        "fields": .object([
          "label": .object(["kind": .string("string"), "value": .string("comet")])
        ]),
        "proposed_id": .string("after-initial"),
      ])
    // This is the managed drain continuation, while the earlier owned reply is
    // deliberately suspended. It must retain one followup instead of losing it.
    await db.drainObserved()
    await runtime.resume()
    await fulfillment(of: [initial, update], timeout: 2)
    subscription.cancel()
    try await db.close()
  }
}

private actor HeldWriteRuntime: NativeCalling {
  private let committed: @Sendable () -> Void
  private var held: CheckedContinuation<Void, Never>?

  init(committed: @escaping @Sendable () -> Void) { self.committed = committed }

  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }

  func call(_ input: Data) async throws -> Wire {
    let operation = try Wire.decode(input).object?["op"]?.string
    let value = try await NativeRuntime.shared.call(input)
    if operation == "insert" {
      await withCheckedContinuation { continuation in
        held = continuation
        committed()
      }
    }
    return value
  }

  func resume() {
    held?.resume()
    held = nil
  }
}

extension ObservationTests {
  func testRawDrainCannotStealManagedEventsAndExplicitManagedDrainDispatchesThem() async throws {
    let committed = expectation(description: "actual write committed; SDK response held")
    let runtime = HeldWriteRuntime(committed: { committed.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])]),
      namespace: "76767676767676767676767676767676")
    let initial = expectation(description: "initial managed delivery")
    let updated = expectation(description: "explicit managed drain delivered current row")
    let query: ProtocolValue = .object([
      "table": .string("comets"), "predicates": .array([]), "order": .null, "limit": .null,
      "source": .null,
    ])
    let subscription = try await db.observe(query) { result in
      guard case .success(let event) = result,
        case .array(let rows) = event.object?["value"]?.object?["rows"]
      else {
        XCTFail("invalid managed event")
        return
      }
      if rows.isEmpty {
        initial.fulfill()
      } else {
        XCTAssertEqual(rows.first?.object?["id"], .string("owned"))
        updated.fulfill()
      }
    }
    await fulfillment(of: [initial], timeout: 2)
    let write = Task {
      try await db.insert("comets", fields: ["label": .string("new")], proposedID: "owned")
    }
    await fulfillment(of: [committed], timeout: 2)
    do {
      _ = try await db.execute("drain", arguments: ["now_ms": .string("0")])
      XCTFail("raw drain stole an SDK-managed snapshot")
    } catch { XCTAssertEqual(error as? HoyaError, .busy) }
    do { try await db.drainNotifications() } catch {
      XCTFail("explicit managed drain failed: \(error)")
    }
    await runtime.resume()
    _ = try await write.value
    await fulfillment(of: [updated], timeout: 2)
    subscription.cancel()
    try await db.close()
  }
}

private actor HeldRegistrationRuntime: NativeCalling {
  private let registered: @Sendable () -> Void
  private var first = true
  private var release: CheckedContinuation<Void, Never>?

  init(registered: @escaping @Sendable () -> Void) { self.registered = registered }

  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }

  func call(_ input: Data) async throws -> Wire {
    let operation = try Wire.decode(input).object?["op"]?.string
    let result = try await NativeRuntime.shared.call(input)
    if operation == "observe", first {
      first = false
      await withCheckedContinuation { continuation in
        release = continuation
        registered()
      }
    }
    return result
  }

  func resume() {
    release?.resume()
    release = nil
  }
}

extension ObservationTests {
  func testRetiredRegistrationCannotBlockReplacementOwnerInitialDelivery() async throws {
    let registered = expectation(description: "old native subscription created; reply held")
    let runtime = HeldRegistrationRuntime(registered: { registered.fulfill() })
    let db = Database(runtime: runtime)
    let schema = Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])])
    let query: ProtocolValue = .object([
      "table": .string("comets"), "predicates": .array([]), "order": .null, "limit": .null,
      "source": .null,
    ])
    try await db.openMemory(schema: schema, namespace: "77777777777777777777777777777777")
    let old = Task {
      try await db.observe(query) { _ in XCTFail("retired observer received callback") }
    }
    await fulfillment(of: [registered], timeout: 2)
    try await db.close()
    try await db.openMemory(schema: schema, namespace: "78787878787878787878787878787878")
    let delivered = expectation(
      description: "replacement initial event before retired reply returns")
    let replacement = try await db.observe(query) { result in
      guard case .success(let event) = result else {
        XCTFail("replacement observer failed")
        return
      }
      XCTAssertEqual(event.object?["value"]?.object?["rows"], .array([]))
      delivered.fulfill()
    }
    await fulfillment(of: [delivered], timeout: 2)
    await runtime.resume()
    do {
      _ = try await old.value
      XCTFail("retired registration succeeded")
    } catch { XCTAssertEqual(error as? HoyaError, .closed) }
    replacement.cancel()
    try await db.close()
  }
}

extension ObservationTests {
  func testTypedDataStatusAndProgressDeliverWithoutManualDrain() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])]),
      namespace: "83838383838383838383838383838383")
    let data = expectation(description: "typed initial rows")
    let status = expectation(description: "typed initial queue status")
    let progress = expectation(description: "typed initial sync progress")
    let rows = try await db.observe(Query("comets")) { result in
      XCTAssertEqual(try? result.get(), .rows([]))
      data.fulfill()
    }
    let queue = try await db.observeQueueStatus { result in
      XCTAssertEqual(try? result.get().live, 0)
      status.fulfill()
    }
    let sync = try await db.observeProgress { result in
      XCTAssertEqual(try? result.get().phase, .notStarted)
      progress.fulfill()
    }
    await fulfillment(of: [data, status, progress], timeout: 2)
    rows.cancel()
    queue.cancel()
    sync.cancel()
    try await db.close()
  }
}

extension ObservationTests {
  /// A deadline timer armed before the drain that stamped the last delivered count can fire before
  /// the core's 16 ms window ends. The pending count must not be stranded until another write.
  func testDeadlineDrainBeforeCoreWindowArmsOneFollowUpTimer() async throws {
    let timing = ManualTiming()
    timing.set(100)
    let db = Database(
      runtime: NativeRuntime.shared, monotonicClock: { timing.now },
      timerScheduler: { delay, work in timing.schedule(delay, work) })
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["comets": Table(columns: ["label": Column(.string)])]),
      namespace: "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a")
    let query: ProtocolValue = .object([
      "table": .string("comets"), "predicates": .array([]), "order": .null,
      "limit": .null, "source": .null,
    ])
    let initial = expectation(description: "initial count stamped at 100")
    let current = expectation(description: "pending count delivered by the follow-up timer")
    let subscription = try await db.observe(query, mode: .count) { result in
      guard case .success(let event) = result else {
        XCTFail("delivery failure")
        return
      }
      switch event.object?["value"]?.object?["count"] {
      case .string("0"): initial.fulfill()
      case .string("1"): current.fulfill()
      default: XCTFail("unexpected count: \(event)")
      }
    }
    await fulfillment(of: [initial], timeout: 2)
    timing.set(105)
    _ = try await db.execute(
      "insert",
      arguments: [
        "table": .string("comets"),
        "fields": .object([
          "label": .object(["kind": .string("string"), "value": .string("comet")])
        ]),
        "proposed_id": .string("comet-1"),
      ])
    await db.drainObserved()
    XCTAssertEqual(timing.scheduled, [16])
    // The host timer fires 5 ms after the write, only 10 ms after the stamped delivery.
    timing.set(110)
    timing.fireFirst()
    for _ in 0..<200 where timing.scheduled.count < 2 {
      try await Task.sleep(nanoseconds: 5_000_000)
    }
    XCTAssertEqual(timing.scheduled, [16, 16], "the undelivered count needs one follow-up timer")
    timing.set(130)
    timing.fireFirst()
    await fulfillment(of: [current], timeout: 2)
    // The follow-up delivered the count: no further timer, no polling loop.
    try await Task.sleep(nanoseconds: 50_000_000)
    XCTAssertEqual(timing.scheduled, [16, 16])
    subscription.cancel()
    try await db.close()
  }
}
