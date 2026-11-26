// TC-0036: default namespaces use the supplied platform CSPRNG capability.
import Foundation
import XCTest

@testable import HoyaDB

final class NamespaceTests: XCTestCase, @unchecked Sendable {
  func testDefaultPlatformEntropyCreatesValidNamespaceAndRealMemoryOwner() async throws {
    let db = Database()
    let schema = Schema(version: 1, tables: ["electrons": Table(columns: ["spin": Column(.float)])])
    try await db.openMemory(schema: schema)
    let actual = try await db.schema()
    XCTAssertEqual(actual, schema)
    try await db.close()
  }
}

extension NamespaceTests {
  func testUniformJitterRejectsIncompleteEntropyBucketAndPropagatesFailure() throws {
    var values: [UInt16] = [65535, 0]
    var samples = 0
    let value = try NamespaceDefaults.jitter {
      samples += 1
      return values.removeFirst()
    }
    XCTAssertEqual(value, 0)
    XCTAssertEqual(samples, 2)
    XCTAssertThrowsError(
      try NamespaceDefaults.jitter { throw HoyaError.core("EntropyUnavailable") }
    ) { error in
      XCTAssertEqual(error as? HoyaError, .core("EntropyUnavailable"))
    }
  }
}
