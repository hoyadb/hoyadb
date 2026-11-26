// TC-0044 CP01/04/05/06/13: public Swift APIs against the actual native facade.
// The release verification records the linked native artifact's current-source provenance.
import Foundation
import HoyaDB
import XCTest

private final class NativeResolutions: @unchecked Sendable {
  private let lock = NSLock()
  private var items: [(String, Resolution)] = []
  func append(_ key: String, _ result: Resolution) {
    lock.lock()
    items.append((key, result))
    lock.unlock()
  }
  var count: Int {
    lock.lock()
    defer { lock.unlock() }
    return items.count
  }
}

final class ConflictPolicyNativeTests: XCTestCase, @unchecked Sendable {
  private let schema = Schema(
    version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
  private let namespace = "45454545454545454545454545454545"

  private func command(_ db: Database, id: String) async throws -> String {
    let receipt = try await db.command(
      CommandDefinition("create-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 100)
    ) { scope in
      _ = try await scope.insert("notes", fields: ["body": .string("optimistic")], proposedID: id)
      return .null
    }
    return receipt.key
  }

  private func rejectAndEdit(_ db: Database, id: String) async throws -> String {
    let key = try await command(db, id: id)
    _ = try await db.preparePush(nowMilliseconds: 100, jitterBasisPoints: 0)
    try await db.receive(key, outcome: .rejected("denied"))
    // Rollback applies to a processed rejection; receiving alone leaves mapper work pending.
    try await db.reconcile(key, plan: .init())
    _ = try await db.replace("notes", id: id, fields: ["body": .string("kept")])
    return key
  }

  func testDefaultStopExplicitPolicyAndOwnerReopenPreserveTruthfulDiscardedState() async throws {
    let store = try await MemoryStore.create()
    let reports = NativeResolutions()
    let db = Database(onResolution: { reports.append($0, $1) })
    try await db.openMemory(schema: schema, namespace: namespace, store: store)
    let key = try await rejectAndEdit(db, id: "local:first")
    do {
      try await db.rollback(key)
      XCTFail("default Stop discarded a conflict")
    } catch { XCTAssertEqual(error as? HoyaError, .core("RollbackConflict")) }
    do {
      try await db.setConflictPolicy(.init(default: .discard, commands: ["bad": .discard]))
      XCTFail("invalid command name replaced the policy")
    } catch { XCTAssertEqual(error as? HoyaError, .core("InvalidCommand")) }
    do {
      try await db.rollback(key)
      XCTFail("failed policy update changed the previous Stop behavior")
    } catch { XCTAssertEqual(error as? HoyaError, .core("RollbackConflict")) }
    try await db.setConflictPolicy(.init(default: .discard))
    let resolution = try await db.rollbackResolving(key)
    XCTAssertEqual(
      resolution, .keptCurrent(discardedKeys: [key], unmapped: ["local:first"], skippedWrites: 0))
    let status = try await db.queueStatus()
    XCTAssertEqual(status.entries.first?.state, .discarded)
    XCTAssertEqual(status.entries.first?.rolledBack, false)
    XCTAssertEqual(reports.count, 1)
    try await db.close()
    try await db.openMemory(schema: schema, namespace: namespace, store: store)
    let reopened = try await db.queueStatus()
    XCTAssertEqual(reopened.entries.first?.state, .discarded)
    let rows = try await db.fetch(Query("notes"))
    XCTAssertEqual(rows, [Row(id: "local:first", fields: ["body": .string("kept")])])
    let next = try await rejectAndEdit(db, id: "local:second")
    do {
      try await db.rollback(next)
      XCTFail("owner-only policy survived reopen")
    } catch { XCTAssertEqual(error as? HoyaError, .core("RollbackConflict")) }
    XCTAssertEqual(reports.count, 1)
    try await db.close()
    try await store.dispose()
  }

  func testConstructorDiscardAndLegacyRollbackReportKeptCurrentOnce() async throws {
    let reports = NativeResolutions()
    let db = Database(conflict: .init(default: .discard), onResolution: { reports.append($0, $1) })
    try await db.openMemory(schema: schema, namespace: namespace)
    let key = try await rejectAndEdit(db, id: "local:legacy")
    try await db.rollback(key)
    let status = try await db.queueStatus()
    XCTAssertEqual(status.entries.first?.state, .discarded)
    XCTAssertEqual(status.entries.first?.rolledBack, false)
    XCTAssertEqual(reports.count, 1)
    let rows = try await db.fetch(Query("notes"))
    XCTAssertEqual(rows.first?.fields["body"], .string("kept"))
    try await db.close()
  }

  func testManualUnsentDiscardKeepsRowsAndReturnsUnmappedIDs() async throws {
    let db = Database()
    try await db.openMemory(schema: schema, namespace: namespace)
    let key = try await command(db, id: "local:manual")
    let before = try await db.fetch(Query("notes"))
    let report = try await db.discard(key)
    XCTAssertEqual(
      report, .keptCurrent(discardedKeys: [key], unmapped: ["local:manual"], skippedWrites: 0))
    let after = try await db.fetch(Query("notes"))
    XCTAssertEqual(after, before)
    let status = try await db.queueStatus()
    XCTAssertEqual(status.entries.first?.state, .discarded)
    XCTAssertEqual(status.live, 0)
    try await db.close()
  }

  func testCorrectionKeepsCurrentRowAppliesMappingAndReportsSkippedWrites() async throws {
    let reports = NativeResolutions()
    let db = Database(conflict: .init(default: .discard), onResolution: { reports.append($0, $1) })
    try await db.openMemory(schema: schema, namespace: namespace)
    let key = try await command(db, id: "local:corrected")
    _ = try await db.preparePush(nowMilliseconds: 100, jitterBasisPoints: 0)
    try await db.receive(key, outcome: .corrected(.null))
    _ = try await db.replace("notes", id: "local:corrected", fields: ["body": .string("kept")])
    let report = try await db.reconcileResolving(
      key,
      plan: ReconcilePlan(
        writes: [.put(table: "notes", row: Row(id: "remote", fields: ["body": .string("server")]))],
        mappings: ["local:corrected": "remote"]))
    XCTAssertEqual(report, .keptCurrent(discardedKeys: [], unmapped: [], skippedWrites: 1))
    let rows = try await db.fetch(Query("notes"))
    XCTAssertEqual(rows, [Row(id: "remote", fields: ["body": .string("kept")])])
    let status = try await db.queueStatus()
    XCTAssertEqual(status.entries.first?.state, .corrected)
    XCTAssertEqual(status.live, 0)
    XCTAssertEqual(reports.count, 1)
    try await db.close()
  }
}
