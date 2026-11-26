// TC-0036: compiled behavioral RED first; actual general native facade on GREEN.
import Darwin
import Foundation
import XCTest

@testable import HoyaDB

final class NativeOwnerTests: XCTestCase, @unchecked Sendable {
  func testTwoUnrelatedMemorySchemasAndClosedOwner() async throws {
    for (table, field, namespace) in [
      ("poets", "display", "51515151515151515151515151515151"),
      ("instruments", "label", "52525252525252525252525252525252"),
    ] {
      let expected = Schema(version: 1, tables: [table: Table(columns: [field: Column(.string)])])
      let db = Database()
      try await db.openMemory(schema: expected, namespace: namespace)
      let actual = try await db.schema()
      XCTAssertEqual(actual, expected)
      let protection = try await db.protection()
      XCTAssertEqual(protection.encrypted, false)
      XCTAssertEqual(protection.cipher, "none")
      XCTAssertEqual(protection.backup, "not_applicable")
      XCTAssertNil(protection.verifiedBackupExcluded)
      try await db.close()
      do {
        _ = try await db.schema()
        XCTFail("closed owner was usable")
      } catch {
        XCTAssertEqual(error as? HoyaError, .closed)
      }
    }
  }

  func testProtectedFoundationReadbackCorrectKeyReopenAndWrongKeyPreservesFile() async throws {
    let root = try canonicalTemporaryDirectory()
      .appendingPathComponent("hoya-sdk-swift-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    let expected = Schema(version: 1, tables: ["orbits": Table(columns: ["name": Column(.string)])])
    let key = Data((0..<32).map { UInt8($0 * 7) })
    let namespace = "53535353535353535353535353535353"
    let db = Database()
    try await db.openProtected(
      directory: root, name: "orbit.sqlite", key: key, schema: expected, namespace: namespace)
    let protection = try await db.protection()
    XCTAssertTrue(protection.encrypted)
    XCTAssertEqual(protection.cipher, "SQLCipher")
    XCTAssertEqual(protection.version, "4.19.0")
    XCTAssertEqual(protection.backup, "host_managed")
    XCTAssertEqual(protection.verifiedBackupExcluded, true)
    XCTAssertEqual(
      try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, true)
    try await db.close()
    let file = root.appendingPathComponent("orbit.sqlite")
    let before = try Data(contentsOf: file)
    XCTAssertFalse(before.starts(with: Data("SQLite format 3\0".utf8)))
    do {
      try await db.openProtected(
        directory: root, name: "orbit.sqlite", key: Data(repeating: 0x55, count: 32),
        schema: expected, namespace: namespace)
      XCTFail("wrong key opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .core("BackendFailure"))
    }
    XCTAssertEqual(try Data(contentsOf: file), before)
    // SPEC-0036 item 5 / TC-0040 BK02: the reserved policy is fixed by location.
    do {
      try await db.openProtected(
        directory: root, name: "orbit.sqlite", key: key, schema: expected, namespace: namespace,
        backupExcluded: false)
      XCTFail("changed backup policy opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .backupPolicyFailure)
    }
    XCTAssertEqual(try Data(contentsOf: file), before)
    XCTAssertEqual(
      try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, true)
    try await db.openProtected(
      directory: root, name: "orbit.sqlite", key: key, schema: expected, namespace: namespace)
    let actual = try await db.schema()
    XCTAssertEqual(actual, expected)
    try await db.close()
    let included = try canonicalTemporaryDirectory()
      .appendingPathComponent("hoya-sdk-swift-included-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: included, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: included) }
    try await db.openProtected(
      directory: included, name: "orbit.sqlite", key: key, schema: expected, namespace: namespace,
      backupExcluded: false)
    let optedIn = try await db.protection()
    XCTAssertEqual(optedIn.verifiedBackupExcluded, false)
    XCTAssertEqual(
      try included.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, false)
    try await db.close()
  }

  func testMalformedKeyAndSymlinkRejectBeforeBackupPolicyOrFileCreation() async throws {
    let root = try canonicalTemporaryDirectory()
      .appendingPathComponent("hoya-sdk-swift-invalid-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    var directory = root
    var attributes = URLResourceValues()
    attributes.isExcludedFromBackup = false
    try directory.setResourceValues(attributes)
    let schema = Schema(
      version: 1, tables: ["measurements": Table(columns: ["name": Column(.string)])])
    let db = Database()
    do {
      try await db.openProtected(
        directory: root, name: "store.sqlite", key: Data(repeating: 0x44, count: 31),
        schema: schema, namespace: "54545454545454545454545454545454")
      XCTFail("malformed key opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .invalidRequest)
    }
    XCTAssertEqual(
      try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, false)
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
    let alias = root.appendingPathExtension("alias")
    try FileManager.default.createSymbolicLink(at: alias, withDestinationURL: root)
    defer { try? FileManager.default.removeItem(at: alias) }
    do {
      try await db.openProtected(
        directory: alias, name: "store.sqlite", key: Data(repeating: 0x44, count: 32),
        schema: schema, namespace: "54545454545454545454545454545454")
      XCTFail("symlink root opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .invalidRequest)
    }
    XCTAssertEqual(
      try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, false)
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
  }

  func testPosixNoncanonicalAliasRejectsBeforeBackupMutation() async throws {
    let root = try canonicalTemporaryDirectory()
      .appendingPathComponent("hoya-sdk-swift-alias-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    guard root.path.hasPrefix("/private/var/") else {
      throw XCTSkip("This platform does not expose the macOS /var alias")
    }
    var directory = root
    var attributes = URLResourceValues()
    attributes.isExcludedFromBackup = false
    try directory.setResourceValues(attributes)
    let alias = URL(fileURLWithPath: String(root.path.dropFirst("/private".count)))
    let db = Database()
    do {
      try await db.openProtected(
        directory: alias, name: "store.sqlite", key: Data(repeating: 0x44, count: 32),
        schema: Schema(version: 1, tables: ["samples": Table(columns: ["text": Column(.string)])]),
        namespace: "55555555555555555555555555555555")
      XCTFail("noncanonical alias opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .invalidRequest)
    }
    XCTAssertEqual(
      try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, false)
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
  }

  private func canonicalTemporaryDirectory() throws -> URL {
    guard let path = realpath(FileManager.default.temporaryDirectory.path, nil) else {
      throw HoyaError.invalidRequest
    }
    defer { free(path) }
    return URL(fileURLWithPath: String(cString: path))
  }
}
