// TC-0036 G09/G14: run writer and reader as independently launched processes.
import Darwin
import Foundation
import HoyaDB

func consumerResultsDirectory() throws -> URL {
  if let root = ProcessInfo.processInfo.environment["HOYA_CONSUMER_ROOT"] {
    return URL(fileURLWithPath: root, isDirectory: true)
  }
  #if canImport(UIKit)
    return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
  #else
    throw HoyaError.invalidRequest  // CLI tests supply their recorded owned root.
  #endif
}

private func recoveryDirectory(create: Bool) throws -> URL {
  let root = try consumerResultsDirectory().appendingPathComponent("hoya-sdk-public-recovery")
  if create {
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
  }
  guard let bytes = realpath(root.path, nil) else { throw HoyaError.invalidRequest }
  defer { free(bytes) }
  return URL(fileURLWithPath: String(cString: bytes))
}

private let recoverySchema = Schema(
  version: 1,
  tables: [
    "durable_notes": Table(columns: [
      "body": Column(.string), "large": Column(.integer), "ratio": Column(.float),
    ])
  ])
private let recoveryKey = Data((0..<32).map { UInt8($0 * 7) })
private let recoveryNamespace = "93939393939393939393939393939393"

func recoveryWriter() async throws {
  let directory = try recoveryDirectory(create: true)
  let db = Database()
  try await db.openProtected(
    directory: directory, name: "recovery.sqlite", key: recoveryKey, schema: recoverySchema,
    namespace: recoveryNamespace)
  try await db.configureEntities(scope: "durable-account", tables: []) {
    .init(identity: .string("trusted"), nowMilliseconds: 1000)
  }
  _ = try await db.pullOnce { since, _ in
    guard since == nil else { throw HoyaError.invalidResponse }
    return PullPage(checkpoint: "persisted-checkpoint", complete: true, changes: [])
  }
  _ = try await db.command(
    CommandDefinition("durable-create-v1", arguments: ["large": .init(.integer)]),
    value: .object(["large": .integer(Int64.max)]),
    context: .init(identity: .null, nowMilliseconds: 1000)
  ) { scope in
    _ = try await scope.insert(
      "durable_notes",
      fields: [
        "body": .string("protected-restart-canary-λ"), "large": .integer(Int64.max),
        "ratio": .float(42),
      ], proposedID: "local:restart")
    return .float(42)
  }
  let batch = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
  guard batch.envelopes.count == 1 else { throw HoyaError.invalidResponse }
  try batch.envelopes[0].write(
    to: directory.appendingPathComponent("frozen.txt"), atomically: true, encoding: .utf8)
  try await db.close()
  let bytes = try Data(contentsOf: directory.appendingPathComponent("recovery.sqlite"))
  guard !bytes.starts(with: Data("SQLite format 3\0".utf8)),
    bytes.range(of: Data("protected-restart-canary-λ".utf8)) == nil
  else { throw HoyaError.invalidResponse }
}

func recoveryReader() async throws {
  let directory = try recoveryDirectory(create: false)
  let before = try Data(contentsOf: directory.appendingPathComponent("recovery.sqlite"))
  let db = Database()
  do {
    try await db.openProtected(
      directory: directory, name: "recovery.sqlite", key: Data(repeating: 0x55, count: 32),
      schema: recoverySchema, namespace: recoveryNamespace)
    throw HoyaError.invalidResponse
  } catch HoyaError.core("BackendFailure") {}
  guard try Data(contentsOf: directory.appendingPathComponent("recovery.sqlite")) == before else {
    throw HoyaError.invalidResponse
  }
  try await db.openProtected(
    directory: directory, name: "recovery.sqlite", key: recoveryKey, schema: recoverySchema,
    namespace: recoveryNamespace)
  let rows = try await db.fetch(Query("durable_notes"))
  guard
    rows == [
      Row(
        id: "local:restart",
        fields: [
          "body": .string("protected-restart-canary-λ"), "large": .integer(Int64.max),
          "ratio": .float(42),
        ])
    ]
  else { throw HoyaError.invalidResponse }
  let progress = try await db.syncProgress()
  guard progress.ready, progress.pages == 1 else { throw HoyaError.invalidResponse }
  let status = try await db.queueStatus()
  guard status.sending == 1, let entry = status.entries.first, let due = entry.nextDueMilliseconds
  else { throw HoyaError.invalidResponse }
  let retry = try await db.preparePush(nowMilliseconds: due, jitterBasisPoints: 0)
  guard
    retry.envelopes == [
      try String(contentsOf: directory.appendingPathComponent("frozen.txt"), encoding: .utf8)
    ]
  else { throw HoyaError.invalidResponse }
  try await db.receive(entry.key, outcome: .applied(.object(["id": .string("server:restart")])))
  try await db.reconcile(
    entry.key, plan: ReconcilePlan(mappings: ["local:restart": "server:restart"]))
  guard try await db.fetch(Query("durable_notes")).first?.id == "server:restart" else {
    throw HoyaError.invalidResponse
  }
  try await db.close()
  // This root belongs only to the synthetic ordinary consumer; no import API.
  try FileManager.default.removeItem(at: directory)
}
