// TC-0036 G03/G04: exact typed protocol reaches actual Rust, never Foundation numbers.
import Foundation
import XCTest

@testable import HoyaDB

final class ProtocolTests: XCTestCase, @unchecked Sendable {
  func testExactAdvancedProtocolUsesCallerSchemaAndLosslessCells() async throws {
    let db = Database()
    let schema = Schema(
      version: 1,
      tables: [
        "spectra": Table(columns: [
          "label": Column(.string), "ordinal": Column(.integer), "ratio": Column(.float),
        ])
      ])
    try await db.openMemory(schema: schema, namespace: "57575757575757575757575757575757")
    let fields: [String: ProtocolValue] = [
      "label": .object(["kind": .string("string"), "value": .string("λ🪴")]),
      "ordinal": .object(["kind": .string("integer"), "value": .string(String(Int64.max))]),
      "ratio": .object(["kind": .string("float"), "value": .string("1.25")]),
    ]
    let inserted = try await db.execute(
      "insert",
      arguments: [
        "table": .string("spectra"), "fields": .object(fields),
        "proposed_id": .string("caller-chosen"),
      ])
    XCTAssertEqual(inserted.object?["id"], .string("caller-chosen"))
    XCTAssertEqual(inserted.object?["fields"]?.object?["ordinal"], fields["ordinal"])
    let query: ProtocolValue = .object([
      "table": .string("spectra"), "predicates": .array([]), "order": .null,
      "limit": .null, "source": .null,
    ])
    let fetched = try await db.execute("fetch", arguments: ["query": query])
    XCTAssertEqual(fetched, .array([inserted]))
    do {
      _ = try await db.execute("open", arguments: [:])
      XCTFail("advanced ordinary operation routed binary open")
    } catch {
      XCTAssertEqual(error as? HoyaError, .invalidRequest)
    }
    try await db.close()
    do {
      _ = try await db.execute("fetch", arguments: ["query": query])
      XCTFail("closed advanced operation succeeded")
    } catch {
      XCTAssertEqual(error as? HoyaError, .closed)
    }
  }
}

extension ProtocolTests {
  func testEncodingRejectsNodeAndDepthOverflowBeforeFoundationTraversal() throws {
    XCTAssertThrowsError(try ProtocolValue.array(Array(repeating: .null, count: 100_001)).encoded())
    { error in
      XCTAssertEqual(error as? HoyaError, .core("LimitExceeded"))
    }
    var nested = ProtocolValue.null
    for _ in 0..<65 { nested = .array([nested]) }
    XCTAssertThrowsError(try nested.encoded()) { error in
      XCTAssertEqual(error as? HoyaError, .core("LimitExceeded"))
    }
    let valid = try ProtocolValue.array(Array(repeating: .null, count: 99_999)).encoded()
    XCTAssertEqual(try ProtocolValue.decode(valid), .array(Array(repeating: .null, count: 99_999)))
  }
}
