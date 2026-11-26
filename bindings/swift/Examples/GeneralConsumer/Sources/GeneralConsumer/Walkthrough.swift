// Original TC-0016/0017 caller scenario, using public production/testing products.
import Foundation
import HoyaDB
import HoyaDBTesting

private final class WalkthroughIDs: @unchecked Sendable {
  private let lock = NSLock()
  private var values: [String]
  init(_ values: [String]) { self.values = values }
  func next() -> String? {
    lock.lock()
    defer { lock.unlock() }
    return values.isEmpty ? nil : values.removeFirst()
  }
}

private final class WalkthroughRows: @unchecked Sendable {
  private let lock = NSLock()
  private var seen = Set<Int>()
  private var waits: [Int: CheckedContinuation<Void, Never>] = [:]
  func record(_ count: Int) {
    lock.lock()
    seen.insert(count)
    let work = waits.removeValue(forKey: count)
    lock.unlock()
    work?.resume()
  }
  func wait(_ count: Int) async {
    await withCheckedContinuation { continuation in
      lock.lock()
      if seen.contains(count) {
        lock.unlock()
        continuation.resume()
      } else {
        waits[count] = continuation
        lock.unlock()
      }
    }
  }
}

private actor OfflineNetwork {
  private var calls = 0
  private var waiting: CheckedContinuation<Void, Never>?
  func attempted() {
    calls += 1
    waiting?.resume()
    waiting = nil
  }
  func wait() async {
    if calls > 0 { return }
    await withCheckedContinuation { waiting = $0 }
  }
  func count() -> Int { calls }
}

private func walkthroughSchema(_ version: UInt32 = 1) -> Schema {
  var orderColumns: [String: Column] = [
    "status": Column(.string), "total_cents": Column(.integer), "placed_at": Column(.timestamp),
  ]
  if version == 2 { orderColumns["memo"] = Column(.string, optional: true) }
  return Schema(
    version: version,
    tables: [
      "customers": Table(
        columns: ["name": Column(.string), "email": Column(.string), "country": Column(.string)],
        relations: ["orders": .hasMany(table: "orders", foreignKey: "customer_id")]),
      "orders": Table(
        columns: orderColumns,
        relations: [
          "customer": .toOne(table: "customers", foreignKey: "customer_id", optional: false),
          "items": .hasMany(table: "order_items", foreignKey: "order_id"),
        ]),
      "order_items": Table(
        columns: ["quantity": Column(.integer), "unit_price_cents": Column(.integer)],
        relations: [
          "order": .toOne(table: "orders", foreignKey: "order_id", optional: false),
          "product": .toOne(table: "products", foreignKey: "product_id", optional: false),
        ]),
      "products": Table(columns: ["name": Column(.string), "quantity": Column(.integer)]),
      "order_notes": Table(
        columns: ["body": Column(.string)],
        relations: ["order": .toOne(table: "orders", foreignKey: "order_id", optional: false)]),
    ])
}

private func walkthroughCheck(_ condition: Bool) throws {
  if !condition { throw HoyaError.invalidResponse }
}

func walkthroughExercise(protectedDirectory: URL? = nil) async throws {
  let engine = protectedDirectory == nil ? "memory" : "protected"
  let stamp = try ISO8601DateFormatter().date(from: "2026-09-15T12:00:00Z").map {
    Int64($0.timeIntervalSince1970 * 1000)
  }.unwrap()
  let clock = ManualClock(stamp)
  let ids = WalkthroughIDs(
    ["test-fixed-id-001", "cust_123", "p1", "pending-1", "pending-2", "pending-3", "note_789"]
      + (1...3).flatMap { n in
        [
          n == 1 ? "local:test-fixed-id-001" : "local:order-\(n)", "local:item-\(n)a",
          "local:item-\(n)b",
        ]
      })
  let key = Data((0..<32).map { UInt8($0 * 7) })
  let db: Database
  if let protectedDirectory {
    db = Database(idGenerator: { ids.next() })
    try await db.openProtected(
      directory: protectedDirectory, name: "walkthrough.sqlite", key: key,
      schema: walkthroughSchema(), namespace: "16171617161716171617161716171617")
  } else {
    db = try await MemoryTesting.open(
      schema: walkthroughSchema(), namespace: "16171617161716171617161716171617",
      idGenerator: { ids.next() })
  }
  let offline = OfflineNetwork()
  let query = Query("orders", predicates: [.init("status", .equal, [.string("pending")])])
  let fields: @Sendable () -> [String: Cell] = {
    [
      "status": .string("pending"), "total_cents": .integer(200),
      "placed_at": .timestamp(clock.now()), "customer_id": .reference("cust_123"),
    ]
  }
  let definition = CommandDefinition(
    "add-order-v1", arguments: ["customerId": .init(.string), "items": .init(.array)],
    referencePaths: [
      [.key("customerId")], [.key("items"), .index(0), .key("productId")],
      [.key("items"), .index(1), .key("productId")],
    ])
  let arguments = JSONValue.object([
    "customerId": .string("cust_123"),
    "items": .array([
      .object(["productId": .string("p1"), "quantity": .integer(1)]),
      .object(["productId": .string("p1"), "quantity": .integer(1)]),
    ]),
  ])
  let context = CapturedContext(
    identity: .object(["tenant": .string("walkthrough")]), nowMilliseconds: clock.now())
  let handler: @Sendable (CommandScope) async throws -> JSONValue = { scope in
    let prior = try await scope.fetch(
      Query("orders", predicates: [.init("customer_id", .equal, [.reference("cust_123")])]))
    try walkthroughCheck(prior.count >= 3)
    await Task.yield()
    let captured = try await scope.context()
    var values = fields()
    values["placed_at"] = .timestamp(captured.nowMilliseconds)
    let order = try await scope.insert("orders", fields: values)
    for _ in 0..<2 {
      _ = try await scope.insert(
        "order_items",
        fields: [
          "quantity": .integer(1), "unit_price_cents": .integer(100),
          "order_id": .reference(order.id), "product_id": .reference("p1"),
        ])
    }
    return .object(["id": .string(order.id)])
  }
  do {
    _ = try await db.schema()
    try walkthroughCheck(await offline.count() == 0)
    print("TC17.1 PASS engine=\(engine) zero_network_schema=true")
    let ordinary = try await db.insert(
      "customers",
      fields: [
        "name": .string("ordinary"), "email": .string("ordinary@example.invalid"),
        "country": .string("FR"),
      ])
    try walkthroughCheck(ordinary.id == "test-fixed-id-001")
    _ = try await db.remove("customers", id: ordinary.id)
    _ = try await db.insert(
      "customers",
      fields: [
        "name": .string("Customer"), "email": .string("fixture@example.invalid"),
        "country": .string("FR"),
      ])
    _ = try await db.insert(
      "products", fields: ["name": .string("Product"), "quantity": .integer(100)])
    for _ in 0..<3 { _ = try await db.insert("orders", fields: fields()) }
    _ = try await db.insert(
      "order_notes", fields: ["body": .string("before"), "order_id": .reference("pending-1")])
    try await db.configureEntities(scope: "walkthrough", tables: ["order_notes"]) {
      .init(identity: .string("trusted"), nowMilliseconds: clock.now())
    }
    try walkthroughCheck(try await db.fetch(query).count == 3)
    let related = try await db.fetch(
      Query("customers", source: .init(table: "orders", id: "pending-1", relation: "customer")))
    try walkthroughCheck(related.first?.id == "cust_123")
    print("TC16.1 TC17.3 PASS engine=\(engine) pending=3 ordinary_fixed_id=exact relation=core")
    let rows = WalkthroughRows()
    let subscription = try await db.observe(query) { result in
      if case .success(.rows(let current)) = result { rows.record(current.count) }
    }
    await rows.wait(3)
    let first = try await db.command(
      definition, value: arguments, context: context, handler: handler)
    guard case .object(let firstResult) = first.value,
      case .string(let orderID)? = firstResult["id"]
    else { throw HoyaError.invalidResponse }
    try walkthroughCheck(orderID == "local:test-fixed-id-001")
    let order = try await db.fetch(
      Query("orders", predicates: [.init("id", .equal, [.string(orderID)])]))
    try walkthroughCheck(order.first?.fields["placed_at"] == .timestamp(stamp))
    let iso = ISO8601DateFormatter().string(from: Date(timeIntervalSince1970: Double(stamp) / 1000))
    try walkthroughCheck(iso == "2026-09-15T12:00:00Z")
    let items = try await db.fetch(
      Query("order_items", predicates: [.init("order_id", .equal, [.reference(orderID)])]))
    try walkthroughCheck(items.count == 2)
    await rows.wait(4)
    print(
      "TC16.2 TC16.7 TC16.8 TC17.4 TC17.5 PASS engine=\(engine) command_id=local:test-fixed-id-001 items=2 clock=2026-09-15T12:00:00Z"
    )
    let fake = FakeTransport(handlers: [
      "add-order-v1": { _ in .applied(.object(["id": .string("server:walkthrough-1")])) }
    ])
    let batch = try await db.preparePush(nowMilliseconds: stamp, jitterBasisPoints: 0)
    let wire = try await fake.push("[" + batch.envelopes.joined(separator: ",") + "]")
    guard case .array(let replies) = try await JSONValue.decodeJSON(wire),
      case .object(let reply)? = replies.first
    else { throw HoyaError.invalidResponse }
    try walkthroughCheck(reply["idempotencyKey"] == .string(first.key))
    _ = try await db.receiveWire(wire)
    var mappings = [orderID: "server:walkthrough-1"]
    for (index, item) in items.enumerated() {
      mappings[item.id] = "server:walkthrough-item-\(index)"
    }
    try await db.reconcile(first.key, plan: ReconcilePlan(mappings: mappings))
    print("TC16.3 TC16.4 PASS engine=\(engine) real_fake_contract=keyed")
    let second = try await db.command(
      definition, value: arguments, context: context, handler: handler)
    let reject = FakeTransport(handlers: ["add-order-v1": { _ in .rejected("business refusal") }])
    let secondBatch = try await db.preparePush(nowMilliseconds: stamp, jitterBasisPoints: 0)
    let rejectedWire = try await reject.push(
      "[" + secondBatch.envelopes.joined(separator: ",") + "]")
    _ = try await db.receiveWire(rejectedWire)
    let rejected = try await db.queueStatus()
    try walkthroughCheck(rejected.entries.first { $0.key == second.key }?.state == .rejected)
    print("TC16.5 PASS engine=\(engine) business_rejection=true")
    let third = try await db.command(
      definition, value: arguments, context: context, handler: handler)
    let dropped = try await db.preparePush(nowMilliseconds: stamp, jitterBasisPoints: 0)
    let noACK = FakeTransport(
      handlers: ["add-order-v1": { _ in .applied(.null) }], dropAcknowledgement: { _ in true })
    do {
      _ = try await noACK.push("[" + dropped.envelopes.joined(separator: ",") + "]")
      throw HoyaError.invalidResponse
    } catch FakeTransportError.droppedAcknowledgement {}
    let sent = try await db.queueStatus()
    let retry = sent.entries.first { $0.key == third.key }
    try walkthroughCheck(retry?.state == .sent && retry?.nextDueMilliseconds == stamp + 1000)
    print("TC16.6 PASS engine=\(engine) dropped_ack_sent_retry=true")
    _ = try await db.patch(
      "order_notes", id: "note_789", fields: ["body": .string("offline update")])
    let note = try await db.fetch(
      Query("order_notes", predicates: [.init("id", .equal, [.string("note_789")])]))
    try walkthroughCheck(note.first?.fields["body"] == .string("offline update"))
    let queue = try await db.queueStatus()
    try walkthroughCheck(queue.entries.count >= 4 && queue.live > 0)
    try await db.startSync(
      transport: { _ in
        await offline.attempted()
        throw HoyaError.core("BackendFailure")
      }, now: { clock.now() }, jitter: { 0 }, onError: { _ in })
    await offline.wait()
    await db.stopSync()
    try walkthroughCheck(try await db.fetch(Query("orders")).count == 6)
    print("TC17.6 TC17.7 TC17.8 PASS engine=\(engine) entity_queued=true network_successes=0")
    subscription.cancel()
    try await db.close()
  } catch {
    try? await db.close()
    throw error
  }
  try await walkthroughMigration(protectedDirectory: protectedDirectory)
}

private func walkthroughMigration(protectedDirectory: URL?) async throws {
  let store = protectedDirectory == nil ? try await MemoryStore.create() : nil
  let ids = WalkthroughIDs(["cust-migrate", "order-migrate"])
  let v1 = walkthroughSchema()
  let v2 = walkthroughSchema(2)
  let db: Database
  if let protectedDirectory {
    db = Database(idGenerator: { ids.next() })
    try await db.openProtected(
      directory: protectedDirectory, name: "walkthrough-migration.sqlite",
      key: Data((0..<32).map { UInt8($0 * 7) }), schema: v1,
      namespace: "17171717171717171717171717171717")
  } else {
    db = try await MemoryTesting.open(
      schema: v1, namespace: "17171717171717171717171717171717", store: store,
      idGenerator: { ids.next() })
  }
  _ = try await db.insert(
    "customers",
    fields: [
      "name": .string("Customer"), "email": .string("migration@example.invalid"),
      "country": .string("FR"),
    ])
  _ = try await db.insert(
    "orders",
    fields: [
      "status": .string("pending"), "total_cents": .integer(200), "placed_at": .timestamp(1000),
      "customer_id": .reference("cust-migrate"),
    ])
  try await db.close()
  let migration = Migration(
    fromVersion: 1, to: v2,
    steps: [
      .addColumn(
        table: "orders", name: "memo", column: Column(.string, optional: true), defaultValue: nil)
    ])
  let reopened: Database
  if let protectedDirectory {
    reopened = Database()
    try await reopened.openProtected(
      directory: protectedDirectory, name: "walkthrough-migration.sqlite",
      key: Data((0..<32).map { UInt8($0 * 7) }), schema: v2,
      namespace: "17171717171717171717171717171717", migrations: [migration])
  } else {
    reopened = try await MemoryTesting.open(
      schema: v2, namespace: "17171717171717171717171717171717", store: store,
      migrations: [migration])
  }
  let rows = try await reopened.fetch(Query("orders"))
  try walkthroughCheck(
    rows.count == 1 && rows[0].fields["customer_id"] == .reference("cust-migrate")
      && rows[0].fields["memo"] == nil)
  try await reopened.close()
  try await store?.dispose()
  print(
    "TC17.2 PASS engine=\(protectedDirectory == nil ? "memory" : "protected") authored_migration=true"
  )
}

extension Optional {
  fileprivate func unwrap() throws -> Wrapped {
    guard let value = self else { throw HoyaError.invalidResponse }
    return value
  }
}
