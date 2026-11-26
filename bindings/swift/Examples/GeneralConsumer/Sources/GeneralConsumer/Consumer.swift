// TC-0036: ordinary consumer imports the packaged SDK, not @testable/internal APIs.
import Darwin
import Foundation
import HoyaDB

#if canImport(UIKit)
  import UIKit
#endif

private final class Event: @unchecked Sendable {
  private let lock = NSLock()
  private var result: Result<ProtocolValue, HoyaError>?
  private var continuation: CheckedContinuation<ProtocolValue, any Error>?

  func accept(_ value: Result<ProtocolValue, HoyaError>) {
    lock.lock()
    guard result == nil else {
      lock.unlock()
      return
    }
    result = value
    let continuation = continuation
    self.continuation = nil
    lock.unlock()
    continuation?.resume(with: value.mapError { $0 as any Error })
  }

  func wait() async throws -> ProtocolValue {
    try await withCheckedThrowingContinuation { continuation in
      lock.lock()
      if let result {
        lock.unlock()
        continuation.resume(with: result.mapError { $0 as any Error })
      } else {
        self.continuation = continuation
        lock.unlock()
      }
    }
  }
}

@MainActor
private enum Exercise {
  static func run() async throws {
    for (table, column, namespace) in [
      ("telescopes", "aperture", "61616161616161616161616161616161"),
      ("sonatas", "title", "62626262626262626262626262626262"),
    ] {
      let db = Database()
      let schema = Schema(version: 1, tables: [table: Table(columns: [column: Column(.string)])])
      try await db.openMemory(schema: schema, namespace: namespace)
      guard try await db.schema() == schema else { throw HoyaError.invalidResponse }
      let inserted = try await db.execute(
        "insert",
        arguments: [
          "table": .string(table),
          "fields": .object([column: .object(["kind": .string("string"), "value": .string("λ🪴")])]),
          "proposed_id": .string("consumer-selected"),
        ])
      guard case .object(let row) = inserted, row["id"] == .string("consumer-selected") else {
        throw HoyaError.invalidResponse
      }
      let query: ProtocolValue = .object([
        "table": .string(table), "predicates": .array([]), "order": .null, "limit": .null,
        "source": .null,
      ])
      guard try await db.execute("fetch", arguments: ["query": query]) == .array([inserted]) else {
        throw HoyaError.invalidResponse
      }
      let event = Event()
      let subscription = try await db.observe(query) { result in
        precondition(Thread.isMainThread)
        event.accept(result)
      }
      guard case .object(let initial) = try await event.wait(), initial["kind"] == .string("data")
      else { throw HoyaError.invalidResponse }
      subscription.cancel()
      try await db.close()
    }
    try await walkthroughExercise()
    let documents = try consumerResultsDirectory()
    let root = documents.appendingPathComponent("hoyadb-consumer-owned-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    guard let canonical = realpath(root.path, nil) else { throw HoyaError.invalidRequest }
    let directory = URL(fileURLWithPath: String(cString: canonical))
    free(canonical)
    let db = Database()
    let schema = Schema(
      version: 1, tables: ["orbital_periods": Table(columns: ["period": Column(.integer)])])
    try await walkthroughExercise(protectedDirectory: directory)
    let key = Data((0..<32).map { UInt8($0 * 7) })
    try await db.openProtected(
      directory: directory, name: "encrypted.sqlite", key: key, schema: schema,
      namespace: "63636363636363636363636363636363")
    let protection = try await db.protection()
    guard protection.encrypted, protection.version == "4.19.0",
      protection.verifiedBackupExcluded == true
    else { throw HoyaError.invalidResponse }
    try await db.close()
    let bytes = try Data(contentsOf: directory.appendingPathComponent("encrypted.sqlite"))
    guard !bytes.starts(with: Data("SQLite format 3\0".utf8)) else {
      throw HoyaError.invalidResponse
    }
    try await db.openProtected(
      directory: directory, name: "encrypted.sqlite", key: key, schema: schema,
      namespace: "63636363636363636363636363636363")
    guard try await db.schema() == schema else { throw HoyaError.invalidResponse }
    try await db.close()
  }

  static func report() async -> Int32 {
    do {
      let mode = ProcessInfo.processInfo.environment["HOYA_CONSUMER_MODE"] ?? "all"
      guard ["all", "writer", "reader"].contains(mode) else { throw HoyaError.invalidRequest }
      if mode == "reader" {
        try await recoveryReader()
      } else {
        try await run()
        try await advancedExercise()
        if mode == "writer" { try await recoveryWriter() }
      }
      let details =
        mode == "reader"
        ? "protected_restart=independent wrong_key=unchanged rows=lossless-i64-float queue=frozen-retry mapping=guarded"
        : "schemas=2 protected=SQLCipher4.19.0 automatic_observers=2 commands=cancel-fifo queue=frozen-retry entity=automatic pull=stale-refetch hook=async migration=authored"
      let message = "HOYA_SWIFT_GENERAL_CONSUMER_GREEN mode=\(mode) \(details)\n"
      let root = try consumerResultsDirectory()
      try message.write(
        to: root.appendingPathComponent("hoya-consumer-\(mode).txt"), atomically: true,
        encoding: .utf8)
      print(message)
      return 0
    } catch {
      print("HOYA_SWIFT_GENERAL_CONSUMER_FAILED \(error)")
      return 1
    }
  }
}

#if canImport(UIKit)
  @main
  final class ConsumerApplication: UIResponder, UIApplicationDelegate {
    func application(
      _ application: UIApplication,
      didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
      Task { @MainActor in exit(await Exercise.report()) }
      return true
    }
  }
#else
  @main
  enum ConsumerApplication {
    static func main() async { exit(await Exercise.report()) }
  }
#endif
