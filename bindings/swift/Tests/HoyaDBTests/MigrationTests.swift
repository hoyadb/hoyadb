// TC-0036 G02/G14: all authored step variants through the actual retained core store.
import Foundation
import XCTest

@testable import HoyaDB

final class MigrationTests: XCTestCase, @unchecked Sendable {
  func testAllAuthoredStepsReopenMemoryAndFailedUpgradePreservesOriginal() async throws {
    let store = try await MemoryStore.create()
    let db = Database()
    let v1 = Schema(
      version: 1,
      tables: [
        "notes": Table(columns: ["name": Column(.string), "legacy": Column(.string)]),
        "obsolete": Table(columns: ["value": Column(.string)]),
      ])
    let v2 = Schema(
      version: 2,
      tables: [
        "notes": Table(columns: [
          "title": Column(.string), "extra": Column(.integer, optional: true),
          "active": Column(.boolean),
        ]),
        "settings": Table(columns: ["enabled": Column(.boolean)]),
      ])
    let namespace = "75757575757575757575757575757575"
    try await db.openMemory(schema: v1, namespace: namespace, store: store)
    _ = try await db.insert(
      "notes", fields: ["name": .string("kept"), "legacy": .string("drop")], proposedID: "note-one")
    try await db.close()
    let upgrade = Migration(
      fromVersion: 1, to: v2,
      steps: [
        .addTable("settings", Table(columns: ["enabled": Column(.boolean)])),
        .dropTable("obsolete"),
        .addColumn(
          table: "notes", name: "extra", column: Column(.integer, optional: true),
          defaultValue: .null),
        .renameColumn(table: "notes", from: "name", to: "title"),
        .dropColumn(table: "notes", name: "legacy"),
        .addColumn(
          table: "notes", name: "active", column: Column(.boolean), defaultValue: .boolean(false)),
        .backfill(table: "notes", column: "active", value: .boolean(true)),
        .setRelations(table: "notes", relations: [:]),
      ])
    try await db.openMemory(schema: v2, namespace: namespace, store: store, migrations: [upgrade])
    let actualSchema = try await db.schema()
    XCTAssertEqual(actualSchema, v2)
    let expected = Row(
      id: "note-one", fields: ["title": .string("kept"), "extra": .null, "active": .boolean(true)])
    let upgraded = try await db.fetch(Query("notes"))
    XCTAssertEqual(upgraded, [expected])
    try await db.close()
    let v3 = Schema(
      version: 3,
      tables: [
        "notes": Table(columns: [
          "caption": Column(.string), "extra": Column(.integer, optional: true),
          "active": Column(.boolean),
        ]),
        "settings": Table(columns: ["enabled": Column(.boolean)]),
      ])
    do {
      try await db.openMemory(
        schema: v3, namespace: namespace, store: store,
        migrations: [
          Migration(
            fromVersion: 2, to: v3,
            steps: [.renameColumn(table: "notes", from: "missing", to: "caption")])
        ])
      XCTFail("invalid authored upgrade succeeded")
    } catch { XCTAssertEqual(error as? HoyaError, .core("MigrationFailure")) }
    try await db.openMemory(schema: v2, namespace: namespace, store: store)
    let restored = try await db.fetch(Query("notes"))
    XCTAssertEqual(restored, [expected])
    try await db.close()
    try await store.dispose()
  }
}

extension MigrationTests {
  func testProtectedAuthoredUpgradeAndConfiguredCapacityReachCore() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      "hoya-sdk-migration-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    guard let bytes = realpath(root.path, nil) else { throw HoyaError.invalidRequest }
    let canonical = URL(fileURLWithPath: String(cString: bytes))
    free(bytes)
    let db = Database(options: DatabaseOptions(capacity: 1))
    let key = Data(repeating: 0x49, count: 32)
    let namespace = "90909090909090909090909090909090"
    let v1 = Schema(version: 1, tables: ["notes": Table(columns: ["name": Column(.string)])])
    try await db.openProtected(
      directory: canonical, name: "migration.sqlite", key: key, schema: v1, namespace: namespace)
    _ = try await db.insert("notes", fields: ["name": .string("kept")], proposedID: "note")
    try await db.close()
    let v2 = Schema(version: 2, tables: ["notes": Table(columns: ["title": Column(.string)])])
    try await db.openProtected(
      directory: canonical, name: "migration.sqlite", key: key, schema: v2, namespace: namespace,
      migrations: [
        Migration(
          fromVersion: 1, to: v2, steps: [.renameColumn(table: "notes", from: "name", to: "title")])
      ])
    let upgraded = try await db.fetch(Query("notes"))
    XCTAssertEqual(upgraded, [Row(id: "note", fields: ["title": .string("kept")])])
    _ = try await db.command(
      CommandDefinition("capacity-first-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 100)
    ) { _ in .null }
    do {
      _ = try await db.command(
        CommandDefinition("capacity-second-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 101)
      ) { _ in .null }
      XCTFail("configured capacity ignored")
    } catch { XCTAssertEqual(error as? HoyaError, .core("Capacity")) }
    try await db.close()
  }
}
