// TC-0036 G04/G06: actual managed joins, scoped journal and proposed IDs.
import Foundation
import XCTest

@testable import HoyaDB

final class RelationTests: XCTestCase, @unchecked Sendable {
  func testManagedJoinNavigationAndScopedLink() async throws {
    let schema = Schema(
      version: 1,
      tables: [
        "authors": Table(
          columns: ["name": Column(.string)],
          relations: [
            "tags": .manyToMany(
              table: "tags", through: "author_tags", sourceRelation: "author", targetRelation: "tag"
            )
          ]),
        "tags": Table(columns: ["label": Column(.string)]),
        "author_tags": Table(
          columns: [
            "author_id": Column(.reference), "tag_id": Column(.reference),
            "weight": Column(.integer, optional: true),
          ],
          relations: [
            "author": .toOne(table: "authors", foreignKey: "author_id", optional: false),
            "tag": .toOne(table: "tags", foreignKey: "tag_id", optional: false),
          ]),
      ])
    let db = Database(idGenerator: { "local:edge" })
    try await db.openMemory(schema: schema, namespace: "88888888888888888888888888888888")
    _ = try await db.insert("authors", fields: ["name": .string("Ada")], proposedID: "author")
    _ = try await db.insert("tags", fields: ["label": .string("DB")], proposedID: "tag")
    let edge = try await db.link(
      "authors", id: "author", relation: "tags", target: "tag", fields: ["weight": .integer(7)])
    XCTAssertEqual(edge.id, "local:edge")
    let related = Query("tags", source: .init(table: "authors", id: "author", relation: "tags"))
    let rows = try await db.fetch(related)
    XCTAssertEqual(rows.map(\.id), ["tag"])
    let removed = try await db.unlink("authors", id: "author", relation: "tags", target: "tag")
    XCTAssertEqual(removed, 1)
    _ = try await db.command(
      CommandDefinition("join-create-v1"), value: .object([:]),
      context: .init(identity: .null, nowMilliseconds: 100)
    ) { scope in
      let edge = try await scope.link("authors", id: "author", relation: "tags", target: "tag")
      XCTAssertEqual(edge.id, "local:edge")
      let staged = try await scope.fetch(related)
      XCTAssertEqual(staged.map(\.id), ["tag"])
      return .null
    }
    let committed = try await db.fetch(related)
    XCTAssertEqual(committed.map(\.id), ["tag"])
    try await db.close()
  }
}
