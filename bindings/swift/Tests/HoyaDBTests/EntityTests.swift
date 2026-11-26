// TC-0036 G08/G09: real entity queue and guarded owned async merge session.
import Foundation
import XCTest

@testable import HoyaDB

private actor EntityGate {
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

final class EntityTests: XCTestCase, @unchecked Sendable {
  private var schema: Schema {
    Schema(
      version: 1,
      tables: [
        "notes": Table(columns: ["body": Column(.string), "extra": Column(.string, optional: true)])
      ])
  }

  func testEntityQueuePullProgressAndResetUseRealOwner() async throws {
    let db = Database()
    try await db.openMemory(schema: schema, namespace: "79797979797979797979797979797979")
    try await db.configureEntities(scope: "account", tables: ["notes"]) {
      .init(identity: .string("trusted"), nowMilliseconds: 1000)
    }
    let result = try await db.pullOnce { since, scope in
      XCTAssertNil(since)
      XCTAssertEqual(scope, "account")
      return PullPage(
        checkpoint: "first", complete: true,
        changes: [
          EntityChange(
            table: "notes", recordID: "remote", operation: .update,
            fields: ["body": .string("server")], timestamp: 10, fieldTimestamps: ["body": 10])
        ])
    }
    XCTAssertTrue(result.ready)
    XCTAssertEqual(result.pages, 1)
    XCTAssertEqual(result.changes, 1)
    let changes = try await db.entityBatch([
      .insert(table: "notes", fields: ["body": .string("offline")])
    ])
    let local = try XCTUnwrap(changes.first?.after)
    XCTAssertTrue(local.id.hasPrefix("entity:79797979797979797979797979797979:"))
    let status = try await db.execute("queue_status")
    XCTAssertEqual(status.object?["pending"], .string("1"))
    let rows = try await db.fetch(Query("notes"))
    XCTAssertEqual(rows.count, 2)
    _ = try await db.execute("reset", arguments: ["mode": .string("discard_recovery")])
    let cleared = try await db.fetch(Query("notes"))
    XCTAssertEqual(cleared, [])
    let resetProgress = try await db.syncProgress()
    XCTAssertFalse(resetProgress.ready)
    try await db.close()
  }

  func testAsyncMergeReleasesNativeLockAndStalePageIsFetchedAgain() async throws {
    let db = Database()
    try await db.openMemory(schema: schema, namespace: "80808080808080808080808080808080")
    try await db.configureEntities(scope: "account", tables: []) {
      .init(identity: .null, nowMilliseconds: 100)
    }
    let offered = expectation(description: "owned hook awaits host")
    let gate = EntityGate()
    let counts = HookCounts()
    await db.setMergeHooks([
      "notes": { input in
        await counts.hooked()
        XCTAssertEqual(input.incoming.recordID, "remote")
        XCTAssertEqual(input.defaultResolution.row?.fields["body"], .string("server"))
        XCTAssertTrue(input.defaultResolution.metadata.fields["body"]?.authoritative == true)
        XCTAssertEqual(input.schema.version, 1)
        if await counts.hooks == 1 {
          offered.fulfill()
          await gate.wait()
        }
        return input.defaultResolution.row
      }
    ])
    let pull = Task {
      try await db.pullOnce { since, _ in
        await counts.fetched(since)
        return PullPage(
          checkpoint: "fresh", complete: true,
          changes: [
            EntityChange(
              table: "notes", recordID: "remote", operation: .update,
              fields: ["body": .string("server")], timestamp: 10, fieldTimestamps: ["body": 10])
          ])
      }
    }
    await fulfillment(of: [offered], timeout: 2)
    _ = try await db.insert("notes", fields: ["body": .string("local")], proposedID: "unrelated")
    await db.setMergeHooks(["notes": { _ in throw HoyaError.invalidRequest }])
    await gate.release()
    let result = try await pull.value
    XCTAssertTrue(result.ready)
    let fetched = await counts.checkpoints
    XCTAssertEqual(fetched, [nil, nil])
    let hooks = await counts.hooks
    XCTAssertEqual(hooks, 2)
    let rows = try await db.fetch(Query("notes"))
    XCTAssertEqual(rows.map(\.id), ["remote", "unrelated"])
    try await db.close()
  }
}

private actor HookCounts {
  var hooks = 0
  var checkpoints: [String?] = []
  func hooked() { hooks += 1 }
  func fetched(_ checkpoint: String?) { checkpoints.append(checkpoint) }
}

private actor AbortTrackingRuntime: NativeCalling {
  private let aborted: @Sendable () -> Void
  init(aborted: @escaping @Sendable () -> Void) { self.aborted = aborted }
  func open(_ input: Data, key: Data) async throws -> Wire {
    try await NativeRuntime.shared.open(input, key: key)
  }
  func call(_ input: Data) async throws -> Wire {
    let result = try await NativeRuntime.shared.call(input)
    if try Wire.decode(input).object?["op"] == .string("abort_pull") { aborted() }
    return result
  }
}

extension EntityTests {
  func testCancellationDisposesMergeWhileNonCooperativeHookRemainsAwaiting() async throws {
    let aborted = expectation(description: "cancel aborts actual core ticket before hook resumes")
    let offered = expectation(description: "merge hook awaiting host")
    let runtime = AbortTrackingRuntime(aborted: { aborted.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(schema: schema, namespace: "81818181818181818181818181818181")
    try await db.configureEntities(scope: "account", tables: []) {
      .init(identity: .null, nowMilliseconds: 100)
    }
    let gate = EntityGate()
    await db.setMergeHooks([
      "notes": { input in
        offered.fulfill()
        await gate.wait()
        return input.defaultResolution.row
      }
    ])
    let task = Task {
      try await db.pullOnce { _, _ in
        PullPage(
          checkpoint: "cancelled", complete: true,
          changes: [
            EntityChange(
              table: "notes", recordID: "remote", operation: .update,
              fields: ["body": .string("server")], timestamp: 10, fieldTimestamps: ["body": 10])
          ])
      }
    }
    await fulfillment(of: [offered], timeout: 2)
    task.cancel()
    await fulfillment(of: [aborted], timeout: 2)
    let rows = try await db.fetch(Query("notes"))
    XCTAssertEqual(rows, [])
    await gate.release()
    do {
      _ = try await task.value
      XCTFail("cancelled merge succeeded")
    } catch { XCTAssertTrue(error is CancellationError) }
    let progress = try await db.syncProgress()
    XCTAssertEqual(progress.pages, 0)
    try await db.close()
  }
}
