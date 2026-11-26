// TC-0036: public SDK only; the same executable runs on macOS and iOS.
import Foundation
import HoyaDB

private actor Gate {
  private var released = false
  private var pending: CheckedContinuation<Void, Never>?

  func wait() async {
    if released { return }
    await withCheckedContinuation { pending = $0 }
  }

  func release() {
    released = true
    pending?.resume()
    pending = nil
  }
}

private actor Counts {
  var hooks = 0
  var fetches = 0
  func hook() -> Int {
    hooks += 1
    return hooks
  }
  func fetch() { fetches += 1 }
}

func advancedExercise() async throws {
  let schema = Schema(
    version: 1,
    tables: [
      "notes": Table(columns: ["body": Column(.string), "extra": Column(.string, optional: true)])
    ])
  let db = Database(idGenerator: { "local:test-fixed-id-001" })
  try await db.openMemory(schema: schema, namespace: "91919191919191919191919191919191")
  let entered = Gate()
  let handler = Gate()
  let cancelled = Task {
    try await db.command(
      CommandDefinition("public-cancel-v1", arguments: ["large": .init(.integer)]),
      value: .object(["large": .integer(Int64.max)]),
      context: .init(identity: .string("trusted-app"), nowMilliseconds: 1000)
    ) { scope in
      guard try await scope.arguments() == .object(["large": .integer(Int64.max)]) else {
        throw HoyaError.invalidResponse
      }
      let inserted = try await scope.insert("notes", fields: ["body": .string("staged")])
      guard inserted.id == "local:test-fixed-id-001",
        try await scope.fetch(Query("notes")).count == 1
      else { throw HoyaError.invalidResponse }
      await entered.release()
      await handler.wait()
      return .null
    }
  }
  await entered.wait()
  guard try await db.fetch(Query("notes")).isEmpty else { throw HoyaError.invalidResponse }
  cancelled.cancel()
  // This admitted followup completes before the cancelled app handler resumes.
  let command = try await db.command(
    CommandDefinition("public-followup-v1"), value: .object([:]),
    context: .init(identity: .null, nowMilliseconds: 1000)
  ) { _ in .float(42) }
  await handler.release()
  do {
    _ = try await cancelled.value
    throw HoyaError.invalidResponse
  } catch is CancellationError {}
  guard try await db.fetch(Query("notes")).isEmpty else { throw HoyaError.invalidResponse }
  let first = try await db.preparePush(nowMilliseconds: 1001, jitterBasisPoints: 0)
  let early = try await db.preparePush(nowMilliseconds: 1002, jitterBasisPoints: 0)
  guard first.envelopes.count == 1, early.envelopes.isEmpty, let due = early.nextDueMilliseconds
  else { throw HoyaError.invalidResponse }
  let retry = try await db.preparePush(nowMilliseconds: due, jitterBasisPoints: 0)
  guard retry.envelopes == first.envelopes else { throw HoyaError.invalidResponse }
  try await db.receive(
    command.key, outcome: .applied(.object(["large": .integer(Int64.max), "ratio": .float(42)])))
  let pending = try await db.pendingReconciliations()
  guard pending.count == 1, pending.first?.key == command.key else {
    throw HoyaError.invalidResponse
  }
  try await db.reconcile(command.key, plan: ReconcilePlan())
  guard try await db.queueStatus().live == 0 else { throw HoyaError.invalidResponse }
  try await db.configureEntities(scope: "public-account", tables: []) {
    .init(identity: .null, nowMilliseconds: 2000)
  }
  let offered = Gate()
  let merge = Gate()
  let counts = Counts()
  await db.setMergeHooks([
    "notes": { input in
      guard input.schema == schema, input.incoming.recordID == "remote",
        input.defaultResolution.row?.fields["body"] == .string("server")
      else { throw HoyaError.invalidResponse }
      if await counts.hook() == 1 {
        await offered.release()
        await merge.wait()
      }
      return input.defaultResolution.row
    }
  ])
  let pull = Task {
    try await db.pullOnce { since, scope in
      guard since == nil, scope == "public-account" else { throw HoyaError.invalidResponse }
      await counts.fetch()
      return PullPage(
        checkpoint: "public-checkpoint", complete: true,
        changes: [
          EntityChange(
            table: "notes", recordID: "remote", operation: .update,
            fields: ["body": .string("server")], timestamp: 10, fieldTimestamps: ["body": 10])
        ])
    }
  }
  await offered.wait()
  _ = try await db.insert("notes", fields: ["body": .string("local")], proposedID: "unrelated")
  await db.setMergeHooks(["notes": { _ in throw HoyaError.invalidRequest }])
  await merge.release()
  let progress = try await pull.value
  let hooks = await counts.hooks
  let fetches = await counts.fetches
  guard progress.ready, hooks == 2, fetches == 2 else { throw HoyaError.invalidResponse }
  guard try await db.fetch(Query("notes")).map(\.id) == ["remote", "unrelated"] else {
    throw HoyaError.invalidResponse
  }
  _ = try await db.execute("reset", arguments: ["mode": .string("discard_recovery")])
  await db.setMergeHooks([:])
  try await db.reconfigureEntities(scope: "public-new-account", tables: ["notes"]) {
    .init(identity: .string("new-trusted-app"), nowMilliseconds: 3000)
  }
  let automatic = try await db.entityBatch([
    .insert(table: "notes", fields: ["body": .string("automatic")])
  ])
  guard automatic.first?.after?.id.hasPrefix("entity:91919191919191919191919191919191:") == true,
    try await db.queueStatus().pending == 1
  else { throw HoyaError.invalidResponse }
  try await db.close()
  try await authoredReopen()
}

private func authoredReopen() async throws {
  let store = try await MemoryStore.create()
  let db = Database()
  let namespace = "92929292929292929292929292929292"
  let v1 = Schema(version: 1, tables: ["drafts": Table(columns: ["name": Column(.string)])])
  try await db.openMemory(schema: v1, namespace: namespace, store: store)
  _ = try await db.insert("drafts", fields: ["name": .string("preserved")], proposedID: "draft")
  try await db.close()
  let v2 = Schema(
    version: 2,
    tables: ["drafts": Table(columns: ["title": Column(.string), "ready": Column(.boolean)])])
  try await db.openMemory(
    schema: v2, namespace: namespace, store: store,
    migrations: [
      Migration(
        fromVersion: 1, to: v2,
        steps: [
          .renameColumn(table: "drafts", from: "name", to: "title"),
          .addColumn(
            table: "drafts", name: "ready", column: Column(.boolean), defaultValue: .boolean(true)),
        ])
    ])
  guard
    try await db.fetch(Query("drafts")) == [
      Row(id: "draft", fields: ["title": .string("preserved"), "ready": .boolean(true)])
    ]
  else { throw HoyaError.invalidResponse }
  try await db.close()
  try await store.dispose()
}
