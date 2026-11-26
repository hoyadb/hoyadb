// TC-0036 G01/G03/G04: actual Rust types/queries and atomic failure, no host evaluator.
import Foundation
import XCTest

@testable import HoyaDB

final class TypedValuesTests: XCTestCase, @unchecked Sendable {
  func testLosslessCellsQueryOrderingAndAtomicBatchFailure() async throws {
    let db = Database()
    try await db.openMemory(
      schema: Schema(
        version: 1,
        tables: [
          "particles": Table(columns: [
            "name": Column(.string), "large": Column(.integer), "ratio": Column(.float),
            "time": Column(.timestamp), "enabled": Column(.boolean),
            "ref": Column(.reference, optional: true),
          ])
        ]), namespace: "66666666666666666666666666666666")
    let fields: [String: Cell] = [
      "name": .string("λ🪴"), "large": .integer(Int64.max),
      "ratio": .float(42.0), "time": .timestamp(Int64.min), "enabled": .boolean(true), "ref": .null,
    ]
    let first = try await db.insert("particles", fields: fields, proposedID: "caller-one")
    XCTAssertEqual(first, Row(id: "caller-one", fields: fields))
    var next = fields
    next["large"] = .integer(Int64.min)
    next["ratio"] = .float(1.25)
    next.removeValue(forKey: "ref")
    let second = try await db.insert("particles", fields: next, proposedID: "caller-two")
    let query = Query(
      "particles",
      predicates: [.init("large", .oneOf, [.integer(Int64.min), .integer(Int64.max)])],
      order: .init("large", .descending), limit: 2)
    let rows = try await db.fetch(query)
    XCTAssertEqual(rows, [first, second])
    XCTAssertNil(second.fields["ref"])
    do {
      _ = try await db.batch([
        .remove(table: "particles", id: first.id),
        .insert(table: "particles", fields: ["name": .boolean(false)], proposedID: "bad"),
      ])
      XCTFail("invalid atomic batch committed")
    } catch { XCTAssertEqual(error as? HoyaError, .core("InvalidRow")) }
    let preserved = try await db.fetch(Query("particles", order: .init("large", .descending)))
    XCTAssertEqual(preserved, [first, second])
    let replacement = try await db.replace("particles", id: first.id, fields: next)
    XCTAssertEqual(replacement.fields, next)
    let removed = try await db.remove("particles", id: second.id)
    XCTAssertTrue(removed)
    try await db.close()
  }
}

private final class IDs: @unchecked Sendable {
  private let lock = NSLock()
  private var values: [String?]
  private(set) var count = 0
  init(_ values: [String?]) { self.values = values }
  func next() -> String? {
    lock.lock()
    defer { lock.unlock() }
    count += 1
    return values.isEmpty ? nil : values.removeFirst()
  }
}

extension TypedValuesTests {
  func testInjectedGeneratorIsSampledBeforeAtomicBatchAndCommandKeepsPrefixContract() async throws {
    let ids = IDs(["first", nil])
    let db = Database(idGenerator: { ids.next() })
    let schema = Schema(version: 1, tables: ["items": Table(columns: ["name": Column(.string)])])
    try await db.openMemory(schema: schema, namespace: "85858585858585858585858585858585")
    do {
      _ = try await db.batch([
        .insert(table: "items", fields: ["name": .string("one")]),
        .insert(table: "items", fields: ["name": .string("two")]),
      ])
      XCTFail("exhausted external generator silently fell back to core IDs")
    } catch { XCTAssertEqual(error as? HoyaError, .invalidRequest) }
    let empty = try await db.fetch(Query("items"))
    XCTAssertEqual(empty, [])
    XCTAssertEqual(ids.count, 2)
    try await db.close()
    let ordinary = Database(idGenerator: { "test-fixed-id-001" })
    try await ordinary.openMemory(schema: schema, namespace: "86868686868686868686868686868686")
    let row = try await ordinary.insert("items", fields: ["name": .string("ordinary")])
    XCTAssertEqual(row.id, "test-fixed-id-001")
    try await ordinary.close()
    let commandDB = Database(idGenerator: { "local:test-fixed-id-001" })
    try await commandDB.openMemory(schema: schema, namespace: "87878787878787878787878787878787")
    _ = try await commandDB.command(
      CommandDefinition("fixed-create-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 100)
    ) { scope in
      let row = try await scope.insert("items", fields: ["name": .string("command")])
      XCTAssertEqual(row.id, "local:test-fixed-id-001")
      return .null
    }
    try await commandDB.close()
  }
}
