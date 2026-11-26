// TC-0036 G06/G07: real staged owner isolation through a suspended host handler.
import Foundation
import XCTest

@testable import HoyaDB

private actor CommandGate {
  private var released = false
  private var continuation: CheckedContinuation<Void, Never>?

  func wait() async {
    if released { return }
    await withCheckedContinuation { continuation = $0 }
  }

  func release() {
    released = true
    continuation?.resume()
    continuation = nil
  }
}

final class CommandTests: XCTestCase, @unchecked Sendable {
  // ADR-0037 / SPEC-0037 replaces cross-owner FIFO with independent native owners.
  // Keep scoped isolation and same-owner FIFO assertions under TC-0036 G06.
  func testSuspendedHandlerIsolationSameOwnerFifoAndIndependentOwnerProgress() async throws {
    let schema = Schema(version: 1, tables: ["scores": Table(columns: ["title": Column(.string)])])
    let first = Database()
    let second = Database()
    try await first.openMemory(schema: schema, namespace: "67676767676767676767676767676767")
    try await second.openMemory(schema: schema, namespace: "68686868686868686868686868686868")
    let staged = expectation(description: "first actual scoped write suspended")
    let independent = expectation(description: "independent owner progresses while first is held")
    let sameOwner = expectation(description: "same-owner handler waits for FIFO admission")
    sameOwner.isInverted = true
    let gate = CommandGate()
    let definition = CommandDefinition("score-create-v1", arguments: ["large": .init(.integer)])
    let context = CapturedContext(identity: .string("trusted-application"), nowMilliseconds: 123)
    let job = Task {
      try await first.command(
        definition, value: .object(["large": .integer(Int64.max)]), context: context
      ) { scope in
        let args = try await scope.arguments()
        XCTAssertEqual(args, .object(["large": .integer(Int64.max)]))
        let captured = try await scope.context()
        XCTAssertEqual(captured, context)
        _ = try await scope.insert(
          "scores", fields: ["title": .string("first")], proposedID: "local:first")
        let rows = try await scope.fetch(Query("scores"))
        XCTAssertEqual(rows.map(\.id), ["local:first"])
        staged.fulfill()
        await gate.wait()
        return .object(["large": .integer(Int64.max), "ratio": .float(1.25)])
      }
    }
    await fulfillment(of: [staged], timeout: 2)
    let outside = try await first.fetch(Query("scores"))
    XCTAssertEqual(outside, [])
    let sameOwnerJob = Task {
      try await first.command(
        CommandDefinition("score-following-v1"), value: .object([:]), context: context
      ) { _ in
        sameOwner.fulfill()
        return .null
      }
    }
    let queued = Task {
      try await second.command(
        CommandDefinition("score-second-v1"), value: .object([:]), context: context
      ) { scope in
        independent.fulfill()
        _ = try await scope.insert(
          "scores", fields: ["title": .string("second")], proposedID: "local:second")
        return .null
      }
    }
    await fulfillment(of: [independent], timeout: 2)
    await fulfillment(of: [sameOwner], timeout: 0.03)
    let independentResult = try await queued.value
    XCTAssertEqual(independentResult.value, .null)
    await gate.release()
    let result = try await job.value
    XCTAssertEqual(result.value, .object(["large": .integer(Int64.max), "ratio": .float(1.25)]))
    _ = try await sameOwnerJob.value
    let committed = try await first.fetch(Query("scores"))
    XCTAssertEqual(committed.map(\.id), ["local:first"])
    let secondRows = try await second.fetch(Query("scores"))
    XCTAssertEqual(secondRows.map(\.id), ["local:second"])
    try await first.close()
    try await second.close()
  }
}

extension CommandTests {
  func testOverLimitFinishBodyAbortsScopeAndLeavesAdmissionUsable() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["scores": Table(columns: ["title": Column(.string)])]),
      namespace: "69696969696969696969696969696969")
    do {
      _ = try await db.command(
        CommandDefinition("over-budget-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 10)
      ) { scope in
        _ = try await scope.insert(
          "scores", fields: ["title": .string("must abort")], proposedID: "local:over-budget")
        return .string(String(repeating: "x", count: 1_048_576))
      }
      XCTFail("over-limit command result committed")
    } catch { XCTAssertEqual(error as? HoyaError, .core("LimitExceeded")) }
    let persisted = try await db.fetch(Query("scores"))
    XCTAssertEqual(persisted, [])
    let ticket = try await db.execute("request")
    let admitted = try await db.execute(
      "begin",
      arguments: [
        "request": ticket, "definition": CommandDefinition("after-budget-v1").wire,
        "value": JSONValue.object([:]).wire,
        "context": CapturedContext(identity: .null, nowMilliseconds: 10).wire,
      ])
    XCTAssertEqual(admitted, .boolean(true), "serialization rejection leaked scope admission")
    try await db.close()
  }
}

extension CommandTests {
  func testTaskCancellationReleasesAdmissionWhileHandlerRemainsSuspended() async throws {
    let schema = Schema(version: 1, tables: ["scores": Table(columns: ["title": Column(.string)])])
    let db = Database()
    let nextOwner = Database()
    try await db.openMemory(schema: schema, namespace: "70707070707070707070707070707070")
    try await nextOwner.openMemory(schema: schema, namespace: "71717171717171717171717171717171")
    let suspended = expectation(description: "staged write before cancellation")
    let released = expectation(description: "other owner runs before cancelled handler resumes")
    let gate = CommandGate()
    let job = Task {
      try await db.command(
        CommandDefinition("cancel-before-finish-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 10)
      ) { scope in
        _ = try await scope.insert(
          "scores", fields: ["title": .string("aborted")], proposedID: "local:abort")
        suspended.fulfill()
        await gate.wait()
        do {
          _ = try await scope.fetch(Query("scores"))
          XCTFail("cancelled scope remained usable")
        } catch is CancellationError {}
        return .null
      }
    }
    await fulfillment(of: [suspended], timeout: 2)
    job.cancel()
    let following = Task {
      let result = try await nextOwner.command(
        CommandDefinition("after-cancel-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 10)
      ) { _ in .null }
      released.fulfill()
      return result
    }
    await fulfillment(of: [released], timeout: 2)
    await gate.release()
    do {
      _ = try await job.value
      XCTFail("cancelled command committed")
    } catch is CancellationError {}
    _ = try await following.value
    let rows = try await db.fetch(Query("scores"))
    XCTAssertEqual(rows, [])
    try await db.close()
    try await nextOwner.close()
  }
}

private actor HeldFinishRuntime: NativeCalling {
  private let committed: @Sendable () -> Void
  private var held: CheckedContinuation<Void, Never>?
  private var replacementDrain: (@Sendable () -> Void)?

  init(committed: @escaping @Sendable () -> Void) { self.committed = committed }

  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }

  func call(_ input: Data) async throws -> Wire {
    let operation = try Wire.decode(input).object?["op"]?.string
    let value = try await NativeRuntime.shared.call(input)
    if operation == "drain" { replacementDrain?() }
    if operation == "finish" {
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

  func watchReplacementDrain(_ callback: @escaping @Sendable () -> Void) {
    replacementDrain = callback
  }
}

extension CommandTests {
  func testCancellationAfterSubmittedFinishPreservesDurableResult() async throws {
    let committed = expectation(description: "real finish committed before host response")
    let runtime = HeldFinishRuntime(committed: { committed.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["scores": Table(columns: ["title": Column(.string)])]),
      namespace: "72727272727272727272727272727272")
    let job = Task {
      try await db.command(
        CommandDefinition("submitted-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 10)
      ) { scope in
        _ = try await scope.insert(
          "scores", fields: ["title": .string("committed")], proposedID: "local:submitted")
        return .integer(Int64.max)
      }
    }
    await fulfillment(of: [committed], timeout: 2)
    job.cancel()
    let status = try await db.execute("queue_status")
    XCTAssertEqual(status.object?["pending"], .string("1"))
    await runtime.resume()
    let result = try await job.value
    XCTAssertEqual(result.value, .integer(Int64.max))
    let rows = try await db.fetch(Query("scores"))
    XCTAssertEqual(rows.map(\.id), ["local:submitted"])
    try await db.close()
  }

  func testCommittedFinishSurvivesCloseAndReplacementWithoutStaleDelivery() async throws {
    let committed = expectation(description: "real finish committed before close")
    let runtime = HeldFinishRuntime(committed: { committed.fulfill() })
    let store = try await MemoryStore.create()
    let schema = Schema(
      version: 1, tables: ["scores": Table(columns: ["title": Column(.string)])])
    let db = Database(runtime: runtime)
    try await db.openMemory(
      schema: schema, namespace: "98989898989898989898989898989898", store: store)
    let payload = JSONValue.object([
      "large": .integer(Int64.max), "ratio": .float(42), "text": .string("durable 日本語"),
    ])
    let job = Task {
      try await db.command(
        CommandDefinition("submitted-close-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 10)
      ) { scope in
        _ = try await scope.insert(
          "scores", fields: ["title": .string("committed")], proposedID: "local:closed-finish")
        return payload
      }
    }
    await fulfillment(of: [committed], timeout: 2)
    let beforeClose = try await db.queueStatus()
    let key = try XCTUnwrap(beforeClose.entries.first?.key)
    try await db.close()
    // A fresh owner reads the retained actual store before the old host reply.
    let reopened = Database()
    try await reopened.openMemory(
      schema: schema, namespace: "98989898989898989898989898989898", store: store)
    let committedRows = try await reopened.fetch(Query("scores"))
    XCTAssertEqual(committedRows.map(\.id), ["local:closed-finish"])
    let recovery = try await reopened.queueStatus()
    XCTAssertEqual(recovery.entries.first?.key, key)
    XCTAssertEqual(recovery.pending, 1)
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["stars": Table(columns: ["name": Column(.string)])]),
      namespace: "99999999999999999999999999999999")
    _ = try await db.insert("stars", fields: ["name": .string("replacement")], proposedID: "new")
    let initial = expectation(description: "replacement subscription initial snapshot")
    let subscription = try await db.observe(Query("stars"), mode: .membership) { result in
      if case .success = result { initial.fulfill() }
    }
    await fulfillment(of: [initial], timeout: 2)
    let staleDrain = expectation(description: "old finish must not drain replacement")
    staleDrain.isInverted = true
    await runtime.watchReplacementDrain { staleDrain.fulfill() }
    await runtime.resume()
    do {
      let result = try await job.value
      XCTAssertEqual(result, CommandResult(key: key, value: payload))
    } catch {
      XCTFail("submitted committed finish was hidden after close: \(error)")
    }
    await fulfillment(of: [staleDrain], timeout: 0.15)
    let replacement = try await db.fetch(Query("stars"))
    XCTAssertEqual(replacement, [Row(id: "new", fields: ["name": .string("replacement")])])
    let replacementQueue = try await db.queueStatus()
    XCTAssertEqual(replacementQueue.live, 0)
    subscription.cancel()
    try await db.close()
    try await reopened.close()
    try await store.dispose()
  }

  func testCloseInvalidatesSuspendedScopeWithoutTouchingReplacementOwner() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["scores": Table(columns: ["title": Column(.string)])]),
      namespace: "73737373737373737373737373737373")
    let suspended = expectation(description: "old handler is outside native call")
    let gate = CommandGate()
    let job = Task {
      try await db.command(
        CommandDefinition("close-admitted-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 10)
      ) { scope in
        _ = try await scope.insert(
          "scores", fields: ["title": .string("old")], proposedID: "local:old")
        suspended.fulfill()
        await gate.wait()
        do {
          _ = try await scope.fetch(Query("scores"))
          XCTFail("stale scope reached replacement owner")
        } catch is CancellationError {}
        return .null
      }
    }
    await fulfillment(of: [suspended], timeout: 2)
    try await db.close()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["stars": Table(columns: ["name": Column(.string)])]),
      namespace: "74747474747474747474747474747474")
    _ = try await db.insert(
      "stars", fields: ["name": .string("replacement")], proposedID: "current")
    await gate.release()
    do {
      _ = try await job.value
      XCTFail("closed handler published into replacement owner")
    } catch is CancellationError {}
    let rows = try await db.fetch(Query("stars"))
    XCTAssertEqual(rows, [Row(id: "current", fields: ["name": .string("replacement")])])
    try await db.close()
  }
}
