import CoreFoundation
// API-0008 / TC-0032 H01–H06. Experimental host wrapper, not a packaged SDK.
import Darwin
import Dispatch
import Foundation
import Security

enum HostFailure: Error, CustomStringConvertible {
  case check(String)
  case rpc(String)
  case cabi(Int32)
  case handler, transport, closed, unimplemented
  var description: String {
    switch self {
    case .check(let name): return name
    case .rpc(let kind): return "RPC " + kind
    case .cabi(let code): return "C ABI status " + String(code)
    case .handler: return "synthetic handler failure"
    case .transport: return "synthetic transport unavailable"
    case .closed: return "owner closed"
    case .unimplemented: return "host dispatcher not implemented"
    }
  }
}
func require(_ value: Bool, _ message: String) throws {
  if !value { throw HostFailure.check(message) }
}
func dictionary(_ value: Any) throws -> [String: Any] {
  guard let result = value as? [String: Any] else { throw HostFailure.check("expected Object") }
  return result
}
func list(_ value: Any) throws -> [Any] {
  guard let result = value as? [Any] else { throw HostFailure.check("expected Array") }
  return result
}
func text(_ value: Any) throws -> String {
  guard let result = value as? String else { throw HostFailure.check("expected String") }
  return result
}
func physicalPath(_ path: String) throws -> String {
  guard let value = realpath(path, nil) else {
    throw HostFailure.check("canonical temporary path unavailable")
  }
  defer { free(value) }
  return String(cString: value)
}
func threadID() -> UInt64 {
  var result: UInt64 = 0
  pthread_threadid_np(nil, &result)
  return result
}

indirect enum Tagged: Equatable {
  case null
  case boolean(Bool)
  case string(String)
  case integer(String)
  case float(String)
  case timestamp(String)
  case reference(String)
  case array([Tagged])
  case object([String: Tagged])
  static func int(_ value: Int64) -> Tagged { .integer(String(value)) }
  static func exactInteger(_ value: String) throws -> Tagged {
    guard let parsed = Int64(value), String(parsed) == value else {
      throw HostFailure.rpc("InvalidRequest")
    }
    return .integer(value)
  }
  static func finite(_ value: Double) throws -> Tagged {
    guard value.isFinite else { throw HostFailure.rpc("InvalidRequest") }
    return .float(String(value))
  }
  static func == (lhs: Tagged, rhs: Tagged) -> Bool {
    switch (lhs, rhs) {
    case (.null, .null): return true
    case (.boolean(let a), .boolean(let b)): return a == b
    case (.string(let a), .string(let b)), (.integer(let a), .integer(let b)),
      (.timestamp(let a), .timestamp(let b)), (.reference(let a), .reference(let b)):
      return a == b
    case (.float(let a), .float(let b)): return Double(a) == Double(b)
    case (.array(let a), .array(let b)): return a == b
    case (.object(let a), .object(let b)): return a == b
    default: return false
    }
  }
  var wire: [String: Any] {
    switch self {
    case .null: return ["kind": "null"]
    case .boolean(let value): return ["kind": "boolean", "value": value]
    case .string(let value): return ["kind": "string", "value": value]
    case .integer(let value): return ["kind": "integer", "value": value]
    case .float(let value): return ["kind": "float", "value": value]
    case .timestamp(let value): return ["kind": "timestamp", "value": value]
    case .reference(let value): return ["kind": "reference", "value": value]
    case .array(let values): return ["kind": "array", "value": values.map(\.wire)]
    case .object(let values): return ["kind": "object", "value": values.mapValues(\.wire)]
    }
  }
  init(wire: Any) throws {
    let value = try dictionary(wire)
    let kind = try text(value["kind"] as Any)
    if kind == "null" {
      try require(value.count == 1, "exact Null tag")
      self = .null
      return
    }
    try require(value.count == 2 && value["value"] != nil, "exact tagged shape")
    let payload = value["value"]!
    switch kind {
    case "boolean":
      guard let boolean = payload as? Bool else { throw HostFailure.check("Boolean tag") }
      self = .boolean(boolean)
    case "string": self = .string(try text(payload))
    case "integer": self = try .exactInteger(text(payload))
    case "float":
      let string = try text(payload)
      guard let number = Double(string), number.isFinite else {
        throw HostFailure.check("finite Float tag")
      }
      self = .float(string)
    case "timestamp": self = .timestamp(try text(payload))
    case "reference": self = .reference(try text(payload))
    case "array": self = .array(try list(payload).map { try Tagged(wire: $0) })
    case "object": self = .object(try dictionary(payload).mapValues { try Tagged(wire: $0) })
    default: throw HostFailure.check("known tag")
    }
  }
}
func fields(_ values: [String: Tagged]) -> [String: Any] { values.mapValues(\.wire) }
func orderFields(_ label: String, total: Int64 = Int64.min) -> [String: Tagged] {
  [
    "label": .string(label), "total": .int(total), "ratio": .float("42.0"),
    "active": .boolean(true), "placed_at": .timestamp(String(Int64.max)), "notes": .null,
  ]
}
func query(_ table: String, source: [String: String]? = nil) -> [String: Any] {
  var value: [String: Any] = [
    "table": table, "predicates": [], "order": NSNull(), "limit": NSNull(),
  ]
  if let source { value["source"] = source }
  return value
}
func definition(_ arguments: [String: String] = ["label": "String"], references: [[Any]] = [])
  -> [String: Any]
{
  [
    "name": "native-command-v1",
    "arguments": arguments.mapValues { ["kind": $0, "optional": false] as [String: Any] },
    "reference_paths": references,
  ]
}
func entropy() throws -> String {
  var bytes = [UInt8](repeating: 0, count: 16)
  guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
    throw HostFailure.check("platform entropy unavailable")
  }
  return bytes.map { String(format: "%02x", $0) }.joined()
}
func outerOnly(_ value: Any) throws {
  if value is NSNull || value is String { return }
  if let number = value as? NSNumber {
    guard CFGetTypeID(number) == CFBooleanGetTypeID() else {
      throw HostFailure.rpc("InvalidRequest")
    }
    return
  }
  if let values = value as? [Any] {
    for value in values { try outerOnly(value) }
    return
  }
  if let values = value as? [String: Any] {
    for value in values.values { try outerOnly(value) }
    return
  }
  throw HostFailure.rpc("InvalidRequest")
}
final class ReplyBox {
  var data = Data()
  var thread: UInt64 = 0
  var reenter = false
  var reentered = false
}
let replyVisitor: hoya_command_reply = { context, bytes, count in
  let result = Unmanaged<ReplyBox>.fromOpaque(context!).takeUnretainedValue()
  result.data = Data(bytes: bytes!, count: count)
  result.thread = threadID()
  if result.reenter {
    let nested = ReplyBox()
    let request = Data("{\"op\":\"unknown\",\"owner\":null,\"args\":{}}".utf8)
    let code = request.withUnsafeBytes { raw in
      hoya_command_rpc(
        raw.bindMemory(to: UInt8.self).baseAddress, raw.count, replyVisitor,
        Unmanaged.passUnretained(nested).toOpaque())
    }
    result.reentered =
      code == 0 && String(data: nested.data, encoding: .utf8)?.contains("InvalidRequest") == true
  }
}
enum RPC {
  static func raw(_ data: Data, reenter: Bool = false) throws -> (Any, UInt64) {
    let reply = ReplyBox()
    reply.reenter = reenter
    let code = data.withUnsafeBytes { raw in
      hoya_command_rpc(
        raw.bindMemory(to: UInt8.self).baseAddress, raw.count, replyVisitor,
        Unmanaged.passUnretained(reply).toOpaque())
    }
    guard code == 0 else { throw HostFailure.cabi(code) }
    if reenter { try require(reply.reentered, "C reply callback reenters after registry unlock") }
    let value = try JSONSerialization.jsonObject(with: reply.data)
    try outerOnly(value)
    return (value, reply.thread)
  }
  static func call(_ op: String, owner: String? = nil, _ args: [String: Any] = [:]) throws -> Any {
    let request: [String: Any] = ["op": op, "owner": owner as Any? ?? NSNull(), "args": args]
    try outerOnly(request)
    let data = try JSONSerialization.data(withJSONObject: request, options: [.sortedKeys])
    let (value, callbackThread) = try raw(data)
    try require(callbackThread == threadID(), "C reply stays on caller thread")
    let response = try dictionary(value)
    guard response["ok"] as? Bool == true else {
      throw HostFailure.rpc(try text(dictionary(response["error"] as Any)["kind"] as Any))
    }
    return response["value"] ?? NSNull()
  }
}

@MainActor final class Signal {
  private var fired = false
  private var waiters: [CheckedContinuation<Void, Never>] = []
  func fire() {
    fired = true
    let values = waiters
    waiters.removeAll()
    for value in values { value.resume() }
  }
  func wait() async {
    if fired { return }
    await withCheckedContinuation { waiters.append($0) }
  }
}
@MainActor final class Gate {
  let entered = Signal(), released = Signal()
  func pause() async {
    entered.fire()
    await released.wait()
  }
  func release() { released.fire() }
}
@MainActor final class Owner {
  let handle: String
  let host: CommandHost
  var open = true
  var generation: UInt64 = 1
  var callbacks: [String: ([String: Any]) throws -> Void] = [:]
  var draining = false
  var callbackFailures = 0
  var onTransition: (() -> Void)?
  var onClose: (() -> Void)?
  var context: (() throws -> Tagged)?
  var clock: () -> Int64
  private var protection: NativeStoreProtection?

  convenience init(
    unprotected path: String, host: CommandHost, version: String = "1", authored: Bool = false,
    options: [String: String]? = nil
  ) throws {
    try self.init(
      path: path, host: host, version: version, authored: authored,
      options: options, operation: "open", keyHex: nil)
  }

  convenience init(
    protectedKey key: [UInt8], root: String, name: String, host: CommandHost,
    excludedFromBackup: Bool = true, version: String = "1", authored: Bool = false,
    options: [String: String]? = nil
  ) throws {
    let policy = try NativeStoreProtection(
      root: root, name: name, key: key,
      excluded: excludedFromBackup)
    guard ProcessInfo.processInfo.environment["HOYADB_PHASE4_ROOT"] == root else {
      throw HostFailure.rpc("InvalidRequest")
    }
    try policy.apply()
    let hex = key.map { String(format: "%02x", $0) }.joined()
    try self.init(
      path: policy.path, host: host, version: version, authored: authored,
      options: options, operation: "open_secure", keyHex: hex)
    protection = policy
  }

  func revalidateAfterOwnedReplacement() throws {
    guard let policy = protection else { throw HostFailure.rpc("InvalidRequest") }
    try policy.afterOwnedReplacement(ownerIsOpen: open)
  }

  private init(
    path: String, host: CommandHost, version: String, authored: Bool,
    options: [String: String]?, operation: String, keyHex: String?
  ) throws {
    self.host = host
    self.clock = { 1000 }
    self.context = {
      .object(["user": .string("synthetic trusted identity"), "min": .int(Int64.min)])
    }
    var args: [String: Any] = [
      "path": path, "namespace": try entropy(), "version": version, "authored": authored,
      "options": options ?? [
        "capacity": "1000", "history": "100", "initial_delay_ms": "1000", "multiplier": "2",
        "max_delay_ms": "60000",
      ],
    ]
    if let keyHex { args["key_hex"] = keyHex }
    self.handle = try text(RPC.call(operation, args))
  }
  func rpc(_ op: String, _ args: [String: Any] = [:]) throws -> Any {
    guard open else { throw HostFailure.closed }
    let value = try RPC.call(op, owner: handle, args)
    if [
      "finish", "receive", "receive_wire", "reconcile", "rollback", "cancel", "forget",
      "entity_batch", "entity_patch", "apply_pull",
    ].contains(
      op)
    {
      onTransition?()
    }
    return value
  }
  func fetch(_ table: String) throws -> [[String: Any]] {
    try list(rpc("fetch", ["query": query(table)])).map(dictionary)
  }
  func close() throws {
    guard open else { return }
    _ = try rpc("close")
    open = false
    generation += 1
    callbacks.removeAll()
    onClose?()
    host.closed(self)
  }
  @discardableResult func observe(
    _ op: String, args: [String: Any] = [:], callback: @escaping ([String: Any]) throws -> Void
  ) throws -> String {
    let token = try text(rpc(op, args))
    callbacks[token] = callback
    return token
  }
  func unsubscribe(_ token: String) throws {
    callbacks.removeValue(forKey: token)
    _ = try rpc("unsubscribe", ["subscription": token])
  }
  func drain(_ now: Int64 = 1000) throws {
    guard !draining else { throw HostFailure.rpc("Busy") }
    draining = true
    defer { draining = false }
    let expectedGeneration = generation
    var more = true
    while more {
      guard open && generation == expectedGeneration else { return }
      let page = try dictionary(rpc("drain", ["now_ms": String(now)]))
      guard let hasMore = page["has_more"] as? Bool else {
        throw HostFailure.check("drain has_more Boolean")
      }
      more = hasMore
      for value in try list(page["events"] as Any) {
        let event = try dictionary(value)
        let token = try text(event["subscription"] as Any)
        guard open && generation == expectedGeneration, let callback = callbacks[token] else {
          continue
        }
        do { try callback(event) } catch { callbackFailures += 1 }
      }
    }
  }
}
// Scenario factory only: mode is selected explicitly by the test runner at
// process startup and inherited by its independent writer/reader children.
// The two owner constructors themselves never downgrade or choose fallback.
@MainActor private func fixtureOwner(
  _ path: String, host: CommandHost, version: String = "1", authored: Bool = false,
  options: [String: String]? = nil
) throws -> Owner {
  if ProcessInfo.processInfo.environment["HOYADB_PHASE6_PROTECTED"] == "1" {
    let file = URL(fileURLWithPath: path)
    return try Owner(
      protectedKey: securityFixtureKey, root: file.deletingLastPathComponent().path,
      name: file.lastPathComponent, host: host, version: version,
      authored: authored, options: options)
  }
  return try Owner(
    unprotected: path, host: host, version: version, authored: authored, options: options)
}

@MainActor final class Scope {
  private let owner: Owner, request: String
  private let alive: () -> Bool
  init(owner: Owner, request: String, alive: @escaping () -> Bool) {
    self.owner = owner
    self.request = request
    self.alive = alive
  }
  private func rpc(_ op: String, _ args: [String: Any] = [:]) throws -> Any {
    guard alive() && owner.open else { throw HostFailure.rpc("InvalidHandle") }
    var args = args
    args["request"] = request
    return try owner.rpc(op, args)
  }
  func insert(_ table: String, _ values: [String: Tagged]) throws -> [String: Any] {
    try dictionary(rpc("scope_insert", ["table": table, "fields": fields(values)]))
  }
  func fetch(_ table: String, source: [String: String]? = nil) throws -> [[String: Any]] {
    try list(rpc("scope_fetch", ["query": query(table, source: source)])).map(dictionary)
  }
}
@MainActor final class CommandTask {
  let owner: Owner
  let request: String
  var admission: Result<Void, Error>?
  var result: Result<[String: Any], Error>?
  var admissionWaiters: [CheckedContinuation<Void, Error>] = []
  var resultWaiters: [CheckedContinuation<[String: Any], Error>] = []
  var handlerTask: Task<Void, Never>?
  var committed: [String: Any]?
  let generation: UInt64
  var handler: (Scope, Tagged, [String: Any]) async throws -> Tagged
  var definition: [String: Any], args: Tagged, identity: Tagged, now: Int64
  init(
    owner: Owner, request: String, definition: [String: Any], args: Tagged, identity: Tagged,
    now: Int64, handler: @escaping (Scope, Tagged, [String: Any]) async throws -> Tagged
  ) {
    self.owner = owner
    self.generation = owner.generation
    self.request = request
    self.definition = definition
    self.args = args
    self.identity = identity
    self.now = now
    self.handler = handler
  }
  func admitted() async throws {
    if let admission { return try admission.get() }
    return try await withCheckedThrowingContinuation { admissionWaiters.append($0) }
  }
  func value() async throws -> [String: Any] {
    if let result { return try result.get() }
    return try await withCheckedThrowingContinuation { resultWaiters.append($0) }
  }
  func cancel() { owner.host.cancel(self) }
  func admitted(_ value: Result<Void, Error>) {
    guard admission == nil else { return }
    admission = value
    let waiters = admissionWaiters
    admissionWaiters.removeAll()
    for waiter in waiters { waiter.resume(with: value) }
  }
  func complete(_ value: Result<[String: Any], Error>) {
    guard result == nil else { return }
    result = value
    let waiters = resultWaiters
    resultWaiters.removeAll()
    for waiter in waiters { waiter.resume(with: value) }
  }
}
enum HandlerContext { @TaskLocal static var inside = false }
@MainActor final class CommandHost {
  private var queue: [CommandTask] = []
  private var active: CommandTask?
  private struct IdleOperation {
    let id: UUID, owner: Owner, generation: UInt64
    let action: () -> Void, fail: (Error) -> Void
  }
  private var idleWork: [IdleOperation] = []
  private var flushingIdle = false
  // A test barrier models an already copied durable finish reply delayed by host work.
  var afterFinish: (() async -> Void)?
  var hasCommandWork: Bool { active != nil || !queue.isEmpty }
  func submit(
    _ owner: Owner, definition: [String: Any] = definition(),
    args: Tagged = .object(["label": .string("command")]),
    handler: @escaping (Scope, Tagged, [String: Any]) async throws -> Tagged
  ) throws -> CommandTask {
    guard !HandlerContext.inside else { throw HostFailure.rpc("Busy") }
    guard owner.open else { throw HostFailure.closed }
    guard let context = owner.context else { throw HostFailure.rpc("InvalidCommand") }
    let identity = try context()
    let now = owner.clock()
    let request = try text(owner.rpc("request"))
    let job = CommandTask(
      owner: owner, request: request, definition: definition, args: args, identity: identity,
      now: now, handler: handler)
    queue.append(job)
    pump()
    return job
  }
  func execute(
    _ owner: Owner, handler: @escaping (Scope, Tagged, [String: Any]) async throws -> Tagged
  ) async throws -> [String: Any] {
    try Task.checkCancellation()
    let job = try submit(owner, handler: handler)
    return try await withTaskCancellationHandler {
      try await job.value()
    } onCancel: {
      Task { @MainActor in job.cancel() }
    }
  }
  private func pump() {
    guard active == nil else { return }
    while !queue.isEmpty {
      let job = queue[0]
      do {
        let began = try job.owner.rpc(
          "begin",
          [
            "request": job.request, "definition": job.definition, "value": job.args.wire,
            "identity": job.identity.wire, "now_ms": String(job.now),
          ])
        guard began as? Bool == true else { return }
        queue.removeFirst()
        active = job
        job.admitted(.success(()))
        job.handlerTask = Task { @MainActor in await self.run(job) }
        return
      } catch {
        queue.removeFirst()
        job.admitted(.failure(error))
        job.complete(.failure(error))
        _ = try? job.owner.rpc("cancel_request", ["request": job.request])
      }
    }
    flushIdle()
  }
  private func alive(_ job: CommandTask) -> Bool {
    active === job && job.result == nil && job.owner.open && job.owner.generation == job.generation
  }
  private func run(_ job: CommandTask) async {
    do {
      let args = try Tagged(wire: job.owner.rpc("scope_args", ["request": job.request]))
      let context = try dictionary(job.owner.rpc("scope_context", ["request": job.request]))
      let scope = Scope(
        owner: job.owner, request: job.request,
        alive: { [weak self, weak job] in
          guard let self, let job else { return false }
          return self.alive(job)
        })
      let result = try await HandlerContext.$inside.withValue(true) {
        try await job.handler(scope, args, context)
      }
      guard alive(job) else { return }
      try Task.checkCancellation()
      let reply = try dictionary(
        job.owner.rpc("finish", ["request": job.request, "result": result.wire]))
      job.committed = reply
      if let afterFinish { await afterFinish() }
      guard job.result == nil else { return }
      finish(job, .success(reply))
    } catch {
      guard alive(job) else { return }
      _ = try? job.owner.rpc("abort", ["request": job.request])
      finish(job, .failure(error))
    }
  }
  private func finish(_ job: CommandTask, _ result: Result<[String: Any], Error>) {
    if active === job { active = nil }
    job.complete(result)
    // Finish is durable; callback exceptions cannot change its result.
    if job.owner.open { try? job.owner.drain(job.owner.clock()) }
    pump()
  }
  func cancel(_ job: CommandTask) {
    guard job.result == nil else { return }
    if let committed = job.committed {
      finish(job, .success(committed))
      return
    }
    if active === job {
      _ = try? job.owner.rpc("abort", ["request": job.request])
      active = nil
    } else if let index = queue.firstIndex(where: { $0 === job }) {
      queue.remove(at: index)
      _ = try? job.owner.rpc("cancel_request", ["request": job.request])
    }
    job.handlerTask?.cancel()
    job.admitted(.failure(CancellationError()))
    job.complete(.failure(CancellationError()))
    pump()
  }
  func atIdle<T>(_ owner: Owner, _ operation: @escaping () throws -> T) async throws -> T {
    guard !HandlerContext.inside else { throw HostFailure.rpc("Busy") }
    try Task.checkCancellation()
    let id = UUID()
    return try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { continuation in
        whenIdle(owner, id: id, fail: { continuation.resume(throwing: $0) }) {
          do { continuation.resume(returning: try operation()) } catch {
            continuation.resume(throwing: error)
          }
        }
      }
    } onCancel: {
      Task { @MainActor in self.cancelIdle(id) }
    }
  }
  private func cancelIdle(_ id: UUID) {
    guard let index = idleWork.firstIndex(where: { $0.id == id }) else { return }
    let work = idleWork.remove(at: index)
    work.fail(CancellationError())
  }
  func whenIdle(
    _ owner: Owner, id: UUID = UUID(), fail: @escaping (Error) -> Void = { _ in },
    _ action: @escaping () -> Void
  ) {
    guard owner.open else {
      fail(HostFailure.closed)
      return
    }
    idleWork.append(
      IdleOperation(id: id, owner: owner, generation: owner.generation, action: action, fail: fail))
    flushIdle()
  }
  private func flushIdle() {
    guard !flushingIdle && active == nil && queue.isEmpty else { return }
    flushingIdle = true
    defer { flushingIdle = false }
    while active == nil && queue.isEmpty && !idleWork.isEmpty {
      let work = idleWork.removeFirst()
      if work.owner.open && work.owner.generation == work.generation {
        work.action()
      } else {
        work.fail(HostFailure.closed)
      }
    }
  }
  func closed(_ owner: Owner) {
    let invalidated = idleWork.filter { $0.owner === owner }
    idleWork.removeAll { $0.owner === owner }
    for work in invalidated { work.fail(HostFailure.closed) }
    let removed = queue.filter { $0.owner === owner }
    queue.removeAll { $0.owner === owner }
    for job in removed {
      job.admitted(.failure(HostFailure.closed))
      job.complete(.failure(HostFailure.closed))
    }
    if let job = active, job.owner === owner {
      active = nil
      job.handlerTask?.cancel()
      if let committed = job.committed {
        job.complete(.success(committed))
      } else {
        job.complete(.failure(HostFailure.closed))
      }
    }
    pump()
  }
}

@MainActor func h01(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-h01.sqlite", host: host)
  defer { try? owner.close() }
  let gate = Gate()
  var secondBegan = false
  var snapshots: [Int] = []
  var deliverySample: UInt64 = 0
  let started = DispatchTime.now().uptimeNanoseconds
  _ = try owner.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
  ) { event in
    let value = try dictionary(event["value"] as Any)
    snapshots.append(try list(value["value"] as Any).count)
    if snapshots.last == 1 {
      try require(
        try owner.fetch("items").count == 2, "H01 callback sees complete awaited relation batch")
      deliverySample = DispatchTime.now().uptimeNanoseconds - started
    }
  }
  try owner.drain()
  let first = try host.submit(owner) { scope, _, _ in
    let order = try scope.insert("orders", orderFields("suspended 注文🌿\0"))
    let id = try text(order["id"] as Any)
    await gate.pause()
    _ = try scope.insert(
      "items", ["label": .string("one"), "quantity": .int(1), "order_id": .reference(id)])
    _ = try scope.insert(
      "items", ["label": .string("two"), "quantity": .int(2), "order_id": .reference(id)])
    try require(
      try scope.fetch("items", source: ["table": "orders", "id": id, "relation": "items"]).count
        == 2, "H01 staged relations see both awaited items")
    return .object(["id": .string(id)])
  }
  try await first.admitted()
  await gate.entered.wait()
  let second = try host.submit(owner) { scope, _, _ in
    secondBegan = true
    try require(
      try scope.fetch("orders").count == 1 && scope.fetch("items").count == 2,
      "H01 next same-store handler sees entire commit")
    return .null
  }
  try require(
    !secondBegan && owner.fetch("orders").isEmpty && owner.fetch("items").isEmpty,
    "H01 committed readers available while Swift handler suspends")
  gate.release()
  _ = try await first.value()
  _ = try await second.value()
  try require(
    secondBegan && owner.fetch("orders").count == 1 && owner.fetch("items").count == 2,
    "H01 complete atomic commit")
  try require(snapshots == [0, 1] && deliverySample > 0, "H01 no intermediate staged notification")
  print("H01 one bounded handler-to-drain sample ns=" + String(deliverySample))
}
@MainActor func h03(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-h03.sqlite", host: host)
  defer { try? owner.close() }
  var contextCopy: Tagged = .null
  let job = try host.submit(
    owner, definition: definition(["min": "Integer", "max": "Integer", "float": "Float"]),
    args: .object(["min": .int(Int64.min), "max": .int(Int64.max), "float": .float("42.0")])
  ) { scope, args, context in
    guard case .object(let values) = args else { throw HostFailure.check("H03 Object args") }
    try require(
      values["min"] == .int(Int64.min) && values["max"] == .int(Int64.max),
      "H03 exact integer extrema")
    contextCopy = try Tagged(wire: context["identity"] as Any)
    try require(try text(context["now_ms"] as Any) == "1000", "H03 captured time")
    _ = try scope.insert("orders", orderFields("Unicode 🌿\0"))
    return .float("1.25")
  }
  _ = try await job.value()
  contextCopy = .string("mutated host copy")
  let values = try owner.fetch("orders")
  let row = try dictionary(values[0]["fields"] as Any)
  try require(try Tagged(wire: row["total"] as Any) == .int(Int64.min), "H03 typed row exact i64")
  let push = try dictionary(
    owner.rpc("prepare_push", ["now_ms": "1000", "jitter_basis_points": "0"]))
  let envelopes = try list(push["envelopes"] as Any).map(text)
  try require(
    envelopes.count == 1 && envelopes[0].contains("-9223372036854775808")
      && envelopes[0].contains("9223372036854775807") && envelopes[0].contains("42.0")
      && !envelopes[0].contains("mutated host copy"), "H03 frozen envelope/context exact")
  try require(contextCopy == .string("mutated host copy"), "H03 independent copied identity")
  owner.context = nil
  var invoked = false
  do {
    _ = try host.submit(owner) { _, _, _ in
      invoked = true
      return .null
    }
    throw HostFailure.check("H03 missing provider accepted")
  } catch HostFailure.rpc("InvalidCommand") {}
  try require(!invoked, "H03 missing context does not invoke handler")
  do {
    _ = try Tagged.exactInteger("9223372036854775808")
    throw HostFailure.check("H03 overflow accepted")
  } catch HostFailure.rpc("InvalidRequest") {}
  do {
    _ = try Tagged.finite(.infinity)
    throw HostFailure.check("H03 nonfinite accepted")
  } catch HostFailure.rpc("InvalidRequest") {}
  let (_, callbackThread) = try RPC.raw(
    Data("{\"op\":\"unknown\",\"owner\":null,\"args\":{}}".utf8), reenter: true)
  try require(callbackThread == threadID(), "H03 Swift copied C reply caller thread")
  do {
    _ = try RPC.raw(Data([0xff]))
    throw HostFailure.check("H03 invalid UTF8 accepted")
  } catch HostFailure.cabi(1) {}
  let (duplicate, _) = try RPC.raw(
    Data("{\"op\":\"unknown\",\"op\":\"open\",\"owner\":null,\"args\":{}}".utf8))
  try require(
    try text(dictionary(dictionary(duplicate)["error"] as Any)["kind"] as Any) == "InvalidPayload",
    "H03 duplicate outer keys rejected")
  owner.context = { .null }
  invoked = false
  let invalid = try host.submit(
    owner, definition: definition(["value": "Float"]), args: .object(["value": .float("NaN")])
  ) { _, _, _ in
    invoked = true
    return .null
  }
  do {
    _ = try await invalid.value()
    throw HostFailure.check("H03 invalid tagged float accepted")
  } catch HostFailure.rpc("InvalidRequest") {}
  let oversized = try host.submit(
    owner, definition: definition(["value": "String"]),
    args: .object(["value": .string(String(repeating: "x", count: 1_048_577))])
  ) { _, _, _ in
    invoked = true
    return .null
  }
  do {
    _ = try await oversized.value()
    throw HostFailure.check("H03 oversized payload accepted")
  } catch HostFailure.cabi(1) {}
  try require(!invoked, "H03 rejected malformed/oversized values never invoke handler")
}

@MainActor func h02(_ root: String) async throws {
  let host = CommandHost()
  let a = try fixtureOwner(root + "/phase-four-h02-a.sqlite", host: host)
  let b = try fixtureOwner(root + "/phase-four-h02-b.sqlite", host: host)
  defer {
    try? a.close()
    try? b.close()
  }
  let gate = Gate()
  var order: [String] = []
  var cancelledRan = false
  let first = try host.submit(a) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("discard"))
    await gate.pause()
    throw HostFailure.handler
  }
  try await first.admitted()
  await gate.entered.wait()
  do {
    _ = try a.rpc(
      "begin",
      [
        "request": first.request, "definition": definition(),
        "value": Tagged.object(["label": .string("repeat")]).wire, "identity": Tagged.null.wire,
        "now_ms": "1000",
      ])
    throw HostFailure.check("H02 repeated begin accepted")
  } catch HostFailure.rpc("InvalidTicket") {}
  try require(
    try list(a.rpc("scope_fetch", ["request": first.request, "query": query("orders")])).count == 1,
    "H02 rejected repeated begin preserves original active handle")
  let cancelled = try host.submit(b) { _, _, _ in
    cancelledRan = true
    return .null
  }
  let second = try host.submit(b) { _, _, _ in
    order.append("second")
    return .null
  }
  let third = try host.submit(a) { _, _, _ in
    order.append("third")
    return .null
  }
  do {
    _ = try b.rpc("scope_fetch", ["request": first.request, "query": query("orders")])
    throw HostFailure.check("H02 cross-owner request accepted")
  } catch HostFailure.rpc("InvalidHandle") {}
  cancelled.cancel()
  do {
    _ = try await cancelled.value()
    throw HostFailure.check("H02 queued cancellation succeeded")
  } catch is CancellationError {}
  gate.release()
  do {
    _ = try await first.value()
    throw HostFailure.check("H02 handler failure committed")
  } catch HostFailure.handler {}
  _ = try await second.value()
  _ = try await third.value()
  try require(
    !cancelledRan && order == ["second", "third"] && a.fetch("orders").isEmpty,
    "H02 FIFO and abort across distinct owners")
  let activeGate = Gate()
  let late = try host.submit(a) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("cancel"))
    await activeGate.pause()
    _ = try scope.fetch("orders")
    return .null
  }
  try await late.admitted()
  await activeGate.entered.wait()
  let following = try host.submit(b) { _, _, _ in
    order.append("after-cancel")
    return .null
  }
  late.cancel()
  do {
    _ = try await late.value()
    throw HostFailure.check("H02 active cancellation succeeded")
  } catch is CancellationError {}
  _ = try await following.value()
  activeGate.release()
  try require(try a.fetch("orders").isEmpty, "H02 explicit cancellation discarded staging")
  do {
    _ = try a.rpc("finish", ["request": late.request, "result": Tagged.null.wire])
    throw HostFailure.check("H02 stale finish accepted")
  } catch HostFailure.rpc("InvalidHandle") {}
  let closeGate = Gate()
  let closing = try host.submit(a) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("close"))
    await closeGate.pause()
    return .null
  }
  try await closing.admitted()
  await closeGate.entered.wait()
  let survivor = try host.submit(b) { _, _, _ in
    order.append("after-close")
    return .null
  }
  try a.close()
  do {
    _ = try await closing.value()
    throw HostFailure.check("H02 closed handler completed")
  } catch HostFailure.closed {}
  _ = try await survivor.value()
  closeGate.release()
  try require(
    order.suffix(2) == ["after-cancel", "after-close"], "H02 close releases app coordinator")
  let taskGate = Gate()
  let task = Task {
    try await host.execute(b) { scope, _, _ in
      do {
        _ = try host.submit(b) { _, _, _ in .null }
        throw HostFailure.check("H02 nested command accepted")
      } catch HostFailure.rpc("Busy") {}
      do {
        _ = try await host.atIdle(b) { try b.fetch("orders") }
        throw HostFailure.check("H02 nested idle wait accepted")
      } catch HostFailure.rpc("Busy") {}
      _ = try scope.insert("orders", orderFields("Task cancellation"))
      await taskGate.pause()
      return .null
    }
  }
  await taskGate.entered.wait()
  task.cancel()
  do {
    _ = try await task.value
    throw HostFailure.check("H02 Swift Task cancellation succeeded")
  } catch is CancellationError {}
  let afterTask = try host.submit(b) { _, _, _ in .null }
  _ = try await afterTask.value()
  taskGate.release()
  try require(
    try b.fetch("orders").isEmpty,
    "H02 caller Task cancellation aborts without awaiting uncooperative handler")
  let finishGate = Gate()
  host.afterFinish = { await finishGate.pause() }
  let committed = try host.submit(b) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("already committed"))
    return .string("owned committed result")
  }
  await finishGate.entered.wait()
  try require(try b.fetch("orders").count == 1, "H02 delayed finish reply is already durable")
  committed.cancel()
  try require(
    try Tagged(wire: await committed.value()["result"] as Any) == .string("owned committed result"),
    "H02 late cancellation preserves committed success")
  host.afterFinish = nil
  finishGate.release()
}
@MainActor func h04(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-h04.sqlite", host: host)
  defer { try? owner.close() }
  var armed = false
  var secondCalls = 0
  var statusCalls = 0
  var recursiveRejected = false
  var second = ""
  _ = try owner.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
  ) { _ in
    _ = try owner.fetch("orders")
    if armed {
      armed = false
      try owner.unsubscribe(second)
      do {
        try owner.drain()
        throw HostFailure.check("H04 recursive drain accepted")
      } catch HostFailure.rpc("Busy") { recursiveRejected = true }
      throw HostFailure.handler
    }
  }
  second = try owner.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
  ) { _ in secondCalls += 1 }
  _ = try owner.observe("observe_status") { _ in statusCalls += 1 }
  try owner.drain()
  let initialSecond = secondCalls
  let initialStatus = statusCalls
  armed = true
  let job = try host.submit(owner) { scope, _, _ in
    let row = try scope.insert("orders", orderFields("callbacks"))
    return .string(try text(row["id"] as Any))
  }
  let result = try await job.value()
  let key = try text(result["key"] as Any)
  try require(
    recursiveRejected && owner.callbackFailures == 1 && secondCalls == initialSecond
      && statusCalls > initialStatus,
    "H04 callback isolation preserves durable success and suppresses cancelled extracted delivery")
  var outcomes: [String] = []
  _ = try owner.observe("observe_outcome", args: ["key": key]) { event in
    let value = try dictionary(event["value"] as Any)
    try require(
      value["key"] == nil && value["idempotencyKey"] == nil && value["code"] == nil,
      "H04 outcome has no wire routing/status")
    outcomes.append(try text(value["state"] as Any))
  }
  try owner.drain()
  _ = try owner.rpc("prepare_push", ["now_ms": "1000", "jitter_basis_points": "0"])
  try owner.drain()
  _ = try owner.rpc(
    "receive", ["key": key, "outcome": ["state": "applied", "result": Tagged.null.wire]])
  try owner.drain()
  let local = try Tagged(wire: result["result"] as Any)
  guard case .string(let localID) = local else { throw HostFailure.check("H04 handler result ID") }
  _ = try owner.rpc(
    "reconcile", ["key": key, "plan": ["writes": [], "mappings": [localID: "server:callbacks"]]])
  try owner.drain()
  try require(
    outcomes == ["queued", "sent", "acknowledged", "applied"], "H04 owned outcome lifecycle")
  let closing = try fixtureOwner(root + "/phase-four-h04-close.sqlite", host: host)
  var suppressed = 0
  _ = try closing.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
  ) { _ in try closing.close() }
  _ = try closing.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
  ) { _ in suppressed += 1 }
  try closing.drain()
  try require(!closing.open && suppressed == 0, "H04 owner close suppresses extracted callbacks")
}
@MainActor func pushedPage(_ owner: Owner, _ now: Int64) throws -> (
  envelopes: [String], due: Int64?, hasMore: Bool
) {
  let value = try dictionary(
    owner.rpc("prepare_push", ["now_ms": String(now), "jitter_basis_points": "0"]))
  guard let more = value["has_more"] as? Bool else {
    throw HostFailure.check("push has_more Boolean")
  }
  return (
    try list(value["envelopes"] as Any).map(text),
    (value["next_due_ms"] as? String).flatMap(Int64.init), more
  )
}
@MainActor func pushed(_ owner: Owner, _ now: Int64) throws -> ([String], Int64?) {
  var envelopes: [String] = []
  while true {
    let page = try pushedPage(owner, now)
    envelopes += page.envelopes
    if !page.hasMore { return (envelopes, page.due) }
  }
}
@MainActor func createOrder(_ host: CommandHost, _ owner: Owner, _ label: String) async throws -> (
  String, String
) {
  let job = try host.submit(owner) { scope, _, _ in
    .string(try text(scope.insert("orders", orderFields(label))["id"] as Any))
  }
  let reply = try await job.value()
  guard case .string(let id) = try Tagged(wire: reply["result"] as Any) else {
    throw HostFailure.check("created ID result")
  }
  return (try text(reply["key"] as Any), id)
}
@MainActor func restartWriter(_ root: String, after: Bool) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(
    root + (after ? "/phase-four-after.sqlite" : "/phase-four-before.sqlite"), host: host)
  if !after {
    let gate = Gate()
    let job = try host.submit(owner) { scope, _, _ in
      _ = try scope.insert("orders", orderFields("uncommitted"))
      await gate.pause()
      return .null
    }
    try await job.admitted()
    await gate.entered.wait()
    try require(try owner.fetch("orders").isEmpty, "H05 pre-commit ordinary reads empty")
    print("H05 writer pre-commit barrier")
    fflush(stdout)
    kill(getpid(), SIGKILL)
    throw HostFailure.check("SIGKILL unexpectedly returned")
  }
  var keys: [String] = []
  var ids: [String] = []
  let receipt = try await createOrder(host, owner, "receipt")
  keys.append(receipt.0)
  ids.append(receipt.1)
  _ = try pushed(owner, 1000)
  _ = try owner.rpc("receive", ["key": receipt.0, "outcome": NSNull()])
  let pending = try await createOrder(host, owner, "pending local")
  keys.append(pending.0)
  ids.append(pending.1)
  _ = try pushed(owner, 1000)
  let exactResult: Tagged = .object([
    "min": .int(Int64.min), "max": .int(Int64.max), "integral": .float("42.0"),
    "nested": .array([.string("再開🌿\0"), .float("1.25")]),
  ])
  _ = try owner.rpc(
    "receive", ["key": pending.0, "outcome": ["state": "applied", "result": exactResult.wire]])
  let sent = try await createOrder(host, owner, "sent")
  keys.append(sent.0)
  ids.append(sent.1)
  let (frozen, due) = try pushed(owner, 1000)
  try require(frozen.count == 1 && due != nil, "H05 one frozen sent envelope")
  let queued = try await createOrder(host, owner, "queued")
  keys.append(queued.0)
  ids.append(queued.1)
  let saved: [String: Any] = [
    "keys": keys, "ids": ids, "frozen": frozen[0], "due": String(due!), "result": exactResult.wire,
  ]
  try JSONSerialization.data(withJSONObject: saved, options: [.sortedKeys]).write(
    to: URL(fileURLWithPath: root + "/phase-four-restart.json"), options: .atomic)
  print("H05 writer post-commit barrier")
  fflush(stdout)
  kill(getpid(), SIGKILL)
  throw HostFailure.check("SIGKILL unexpectedly returned")
}
@MainActor func restartReader(_ root: String, after: Bool) async throws {
  let host = CommandHost()
  let path = root + (after ? "/phase-four-after.sqlite" : "/phase-four-before.sqlite")
  let owner = try fixtureOwner(path, host: host)
  defer { try? owner.close() }
  if !after {
    try require(
      try owner.fetch("orders").isEmpty, "H05 killed staged rows absent after process reopen")
    let status = try dictionary(owner.rpc("queue_status"))
    try require(try text(status["live"] as Any) == "0", "H05 killed staging has no envelope")
    let job = try host.submit(owner) { scope, _, _ in
      let id = try text(scope.insert("orders", orderFields("counter"))["id"] as Any)
      try require(id == "local:1", "H05 killed staging did not consume durable local counter")
      throw HostFailure.handler
    }
    do {
      _ = try await job.value()
      throw HostFailure.check("H05 aborted counter probe committed")
    } catch HostFailure.handler {}
    return
  }
  let saved = try dictionary(
    JSONSerialization.jsonObject(
      with: Data(contentsOf: URL(fileURLWithPath: root + "/phase-four-restart.json"))))
  let keys = try list(saved["keys"] as Any).map(text)
  let ids = try list(saved["ids"] as Any).map(text)
  let due = Int64(try text(saved["due"] as Any))!
  try require(try owner.fetch("orders").count == 4, "H05 post-commit rows restored")
  let status = try dictionary(owner.rpc("queue_status"))
  try require(
    try text(status["pending"] as Any) == "1" && text(status["sending"] as Any) == "1"
      && text(status["awaiting_outcome"] as Any) == "1" && text(status["live"] as Any) == "4",
    "H05 queue states restored independently")
  let pending = try list(owner.rpc("pending_reconciliations")).map(dictionary)
  try require(
    pending.count == 1 && text(pending[0]["key"] as Any) == keys[1],
    "H05 local reconciliation restored")
  try require(
    try Tagged(wire: dictionary(pending[0]["outcome"] as Any)["result"] as Any)
      == Tagged(wire: saved["result"] as Any), "H05 exact pending result survives process")
  let (newOnly, _) = try pushed(owner, 0)
  try require(
    newOnly.count == 1 && newOnly[0].contains(keys[3]), "H05 backward clock only sends queued work")
  let (early, _) = try pushed(owner, due - 1)
  try require(early.isEmpty, "H05 retry deadline survives reopen")
  let (retry, _) = try pushed(owner, due)
  try require(
    retry.count == 2 && retry.contains(try text(saved["frozen"] as Any)),
    "H05 exact frozen retry after process reopen")
  _ = try owner.rpc(
    "reconcile", ["key": keys[1], "plan": ["writes": [], "mappings": [ids[1]: "server:restart"]]])
  try require(
    try list(owner.rpc("pending_reconciliations")).isEmpty, "H05 local recovery applied once")
  try owner.close()
  do {
    _ = try fixtureOwner(path, host: host, version: "2", authored: true)
    throw HostFailure.check("H05 live work migrated")
  } catch HostFailure.rpc("SchemaConflict") {}
  let reopened = try fixtureOwner(path, host: host)
  defer { try? reopened.close() }
  try require(try reopened.fetch("orders").count == 4, "H05 refused lifecycle preserves rows")
  let again = try dictionary(reopened.rpc("queue_status"))
  try require(
    try text(again["awaiting_outcome"] as Any) == "1",
    "H05 receipt remains awaiting result after another reopen")
}
@MainActor func h05(_ root: String) async throws {
  let executable = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
  for stage in ["before", "after"] {
    for role in ["writer", "reader"] {
      let child = Process()
      child.executableURL = executable
      child.arguments = [role + "-" + stage]
      child.environment = ProcessInfo.processInfo.environment
      try child.run()
      child.waitUntilExit()
      if role == "writer" {
        try require(
          child.terminationReason == .uncaughtSignal && child.terminationStatus == SIGKILL,
          "H05 writer killed only at named barrier")
      } else {
        try require(
          child.terminationReason == .exit && child.terminationStatus == 0,
          "H05 independent reader verified state")
      }
    }
  }
}
enum Transport {
  static func endpoint(_ operation: String) throws -> URL {
    let base = ProcessInfo.processInfo.environment["HOYADB_PHASE5_HTTP"] ?? "http://127.0.0.1:8774"
    guard let url = URL(string: base), url.scheme == "http", url.host == "127.0.0.1" else {
      throw HostFailure.rpc("InvalidRequest")
    }
    return url.appendingPathComponent("hoyadb").appendingPathComponent(operation)
  }
  static func send(_ envelopes: [String], mode: String = "applied") async throws -> String {
    try Task.checkCancellation()
    let bytes = try body(envelopes)
    let configuration = URLSessionConfiguration.ephemeral
    configuration.timeoutIntervalForRequest = 5
    configuration.timeoutIntervalForResource = 5
    let session = URLSession(configuration: configuration)
    defer { session.invalidateAndCancel() }
    var request = URLRequest(url: try endpoint("commands"))
    request.httpMethod = "POST"
    request.httpBody = bytes
    request.timeoutInterval = 5
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.setValue("Bearer synthetic-native-fixture", forHTTPHeaderField: "Authorization")
    request.setValue(mode, forHTTPHeaderField: "X-Hoya-Fixture")
    let (data, response) = try await session.data(for: request)
    try Task.checkCancellation()
    guard let response = response as? HTTPURLResponse, response.statusCode == 200,
      data.count <= 1_048_576, let result = String(data: data, encoding: .utf8)
    else { throw HostFailure.transport }
    return result
  }
  static func body(_ envelopes: [String]) throws -> Data {
    guard !envelopes.isEmpty else { throw HostFailure.rpc("InvalidRequest") }
    // The canonical strings are deliberately never Foundation-decoded/re-encoded.
    let body = Data(("[" + envelopes.joined(separator: ",") + "]").utf8)
    guard body.count <= 1_048_576 else { throw HostFailure.rpc("InvalidRequest") }
    return body
  }
}
@MainActor func h06(_ root: String) async throws {
  let host = CommandHost()
  let path = root + "/phase-four-h06.sqlite"
  let owner = try fixtureOwner(path, host: host)
  defer { try? owner.close() }
  var keys: [String] = []
  var ids: [String] = []
  for index in 0..<3 {
    let job = try host.submit(
      owner,
      definition: definition([
        "label": "String", "min": "Integer", "max": "Integer", "float": "Float",
      ]),
      args: .object([
        "label": .string("HTTP " + String(index)), "min": .int(Int64.min), "max": .int(Int64.max),
        "float": .float("42.0"),
      ])
    ) { scope, _, _ in
      .string(try text(scope.insert("orders", orderFields("HTTP " + String(index)))["id"] as Any))
    }
    let reply = try await job.value()
    keys.append(try text(reply["key"] as Any))
    guard case .string(let id) = try Tagged(wire: reply["result"] as Any) else {
      throw HostFailure.check("H06 local ID")
    }
    ids.append(id)
  }
  let (first, due) = try pushed(owner, 1000)
  try require(first.count == 3 && due == 2000, "H06 batch and backoff")
  do {
    _ = try await Transport.send(first, mode: "unavailable")
    throw HostFailure.check("H06 unavailable succeeded")
  } catch HostFailure.transport {}
  let failed = try dictionary(owner.rpc("queue_status"))
  try require(
    try text(failed["sending"] as Any) == "3", "H06 transport failure leaves sent recovery work")
  try require(try pushed(owner, 1999).0.isEmpty, "H06 no retry before deadline")
  let (retried, _) = try pushed(owner, 2000)
  try require(first == retried, "H06 frozen retry bytes identical")
  let body = try await Transport.send(retried, mode: "mixed")
  let received = try list(owner.rpc("receive_wire", ["body": body])).map(dictionary)
  try require(
    received.count == 3 && received.allSatisfy { $0["ok"] as? Bool == true },
    "H06 reversed keyed results accepted independently")
  let pending = try list(owner.rpc("pending_reconciliations")).map(dictionary)
  try require(
    pending.count == 3 && Set(try pending.map { try text($0["key"] as Any) }) == Set(keys),
    "H06 outcomes correlate by key")
  for value in pending {
    let key = try text(value["key"] as Any)
    let index = keys.firstIndex(of: key)!
    let outcome = try dictionary(value["outcome"] as Any)
    let state = try text(outcome["state"] as Any)
    if state == "rejected" {
      try require(index == 2, "H06 rejected keyed member")
      _ = try owner.rpc(
        "reconcile", ["key": key, "plan": ["writes": [], "mappings": [:] as [String: String]]])
      _ = try owner.rpc("rollback", ["key": key])
    } else {
      try require(
        state == (index == 0 ? "applied" : "corrected"), "H06 applied/corrected keyed members")
      guard case .object(let result) = try Tagged(wire: outcome["result"] as Any),
        case .object(let echo) = result["echo"]
      else { throw HostFailure.check("H06 exact server echo") }
      try require(
        echo["min"] == .int(Int64.min) && echo["max"] == .int(Int64.max)
          && echo["float"] == .float("42.0"), "H06 HTTP echo preserves i64/Float kinds")
      let remote = "server:" + key
      try require(result["id"] == .string(remote), "H06 authoritative key mapping")
      let writes: [[String: Any]] =
        index == 1
        ? [
          [
            "op": "put", "table": "orders", "id": remote,
            "fields": fields(orderFields("corrected authoritative")),
          ]
        ] : []
      _ = try owner.rpc(
        "reconcile", ["key": key, "plan": ["writes": writes, "mappings": [ids[index]: remote]]])
    }
  }
  let rows = try owner.fetch("orders")
  try require(
    rows.count == 2 && rows.allSatisfy { ($0["id"] as? String)?.hasPrefix("server:") == true },
    "H06 mapping and rejected inverse update local store")
  try require(
    try rows.contains {
      try Tagged(wire: dictionary($0["fields"] as Any)["label"] as Any)
        == .string("corrected authoritative")
    }, "H06 application-defined correction committed")
  try require(
    try list(owner.rpc("pending_reconciliations")).isEmpty, "H06 corrections processed once")
  try await h06Matrix(root, host)
  try await h06Guard(root, host)
  try await h06Timer(root, host)
  try await h06Overlap(root, host)
  try await h06Reports(root, host)
  try await h06Pages(root)
  try await h06Plan(root)
}
@MainActor func h06Matrix(_ root: String, _ host: CommandHost) async throws {
  let owner = try fixtureOwner(root + "/phase-four-h06-matrix.sqlite", host: host)
  defer { try? owner.close() }
  var keys: [String] = []
  for _ in 0..<3 {
    let job = try host.submit(owner) { _, _, _ in .null }
    keys.append(try text(await job.value()["key"] as Any))
  }
  let first = try pushed(owner, 1000).0
  let partial = try await Transport.send(first, mode: "partial")
  let partialResults = try list(owner.rpc("receive_wire", ["body": partial]))
  try require(
    partialResults.count == 1 && list(owner.rpc("pending_reconciliations")).count == 1,
    "H06 omitted results remain independently retryable")
  let retry = try pushed(owner, 2000).0
  try require(
    retry.count == 2 && retry.allSatisfy(first.contains),
    "H06 partial retry retains only missing frozen members")
  let duplicate = try await Transport.send(retry, mode: "duplicate")
  let reports = try list(owner.rpc("receive_wire", ["body": duplicate])).map(dictionary)
  try require(
    reports.count == 4 && reports.allSatisfy { $0["ok"] as? Bool == true },
    "H06 equal duplicate outcomes idempotent")
  try require(
    try list(owner.rpc("pending_reconciliations")).count == 3,
    "H06 duplicates do not duplicate pending work")
  for key in keys {
    _ = try owner.rpc(
      "reconcile", ["key": key, "plan": ["writes": [], "mappings": [:] as [String: String]]])
  }
  for _ in 0..<2 {
    let job = try host.submit(owner) { _, _, _ in .null }
    _ = try await job.value()
  }
  let new = try pushed(owner, 2000).0
  let conflict = try await Transport.send(new, mode: "conflict")
  let conflictReports = try list(owner.rpc("receive_wire", ["body": conflict])).map(dictionary)
  try require(
    conflictReports.count == 3 && conflictReports.filter { $0["ok"] as? Bool == false }.count == 1
      && list(owner.rpc("pending_reconciliations")).count == 2,
    "H06 conflicting duplicate reports protocol error and preserves independent finals")
  let protocolError = try dictionary(
    conflictReports.first { $0["ok"] as? Bool == false }!["error"] as Any)
  try require(
    try text(protocolError["kind"] as Any) == "Protocol", "H06 conflicting duplicate typed Protocol"
  )
  do {
    _ = try owner.rpc("receive_wire", ["body": "["])
    throw HostFailure.check("H06 malformed wire accepted")
  } catch HostFailure.rpc("InvalidPayload") {}
  try require(
    try list(owner.rpc("pending_reconciliations")).count == 2,
    "H06 malformed body has no partial side effects")
  let emptySend = Task { try await Transport.send([]) }
  do {
    _ = try await emptySend.value
    throw HostFailure.check("H06 empty body accepted")
  } catch HostFailure.rpc("InvalidRequest") {}
  let cancelled = Task {
    try Task.checkCancellation()
    return try await Transport.send(new)
  }
  cancelled.cancel()
  do {
    _ = try await cancelled.value
    throw HostFailure.check("H06 transport cancellation ignored")
  } catch is CancellationError {} catch let error as URLError where error.code == .cancelled {}
}
@MainActor func h06Guard(_ root: String, _ host: CommandHost) async throws {
  let path = root + "/phase-four-h06-guard.sqlite"
  let owner = try fixtureOwner(path, host: host)
  let (key, id) = try await createOrder(host, owner, "guarded")
  _ = try owner.rpc(
    "direct_batch",
    [
      "writes": [
        [
          "op": "replace", "table": "orders", "id": id,
          "fields": fields(orderFields("unrelated later edit")),
        ]
      ]
    ])
  do {
    _ = try owner.rpc("cancel", ["key": key])
    throw HostFailure.check("H06 conflicting inverse succeeded")
  } catch HostFailure.rpc("RollbackConflict") {}
  let unchanged = try owner.fetch("orders")
  let status = try dictionary(owner.rpc("queue_status"))
  let commands = try list(status["commands"] as Any).map(dictionary)
  try require(
    unchanged.count == 1
      && Tagged(wire: dictionary(unchanged[0]["fields"] as Any)["label"] as Any)
        == .string("unrelated later edit")
      && commands[0]["state"] as? String == "cancelled"
      && commands[0]["rolled_back"] as? Bool == false,
    "H06 cancellation durable while conflicting inverse preserves unrelated edit")
  try owner.close()
  do {
    _ = try fixtureOwner(path, host: host, version: "2", authored: true)
    throw HostFailure.check("H06 unresolved cancellation migrated")
  } catch HostFailure.rpc("SchemaConflict") {}
  let reopened = try fixtureOwner(path, host: host)
  _ = try reopened.rpc(
    "direct_batch",
    [
      "writes": [
        ["op": "replace", "table": "orders", "id": id, "fields": fields(orderFields("guarded"))]
      ]
    ])
  _ = try reopened.rpc("rollback", ["key": key])
  try require(try reopened.fetch("orders").isEmpty, "H06 restored guard permits explicit rollback")
  try reopened.close()
  let migrated = try fixtureOwner(path, host: host, version: "2", authored: true)
  defer { try? migrated.close() }
  try require(try migrated.fetch("orders").isEmpty, "H06 recovery-free authored lifecycle accepted")
}
@MainActor final class DeadlineSignal {
  private var fired = false
  private var waiters: [UUID: CheckedContinuation<Void, Error>] = [:]
  func fire() {
    fired = true
    let values = waiters.values
    waiters.removeAll()
    for waiter in values { waiter.resume() }
  }
  func wait(milliseconds: Int64 = 5000, name: String = "host wake deadline exceeded") async throws {
    if fired { return }
    let id = UUID()
    try await withCheckedThrowingContinuation { continuation in
      waiters[id] = continuation
      Task { @MainActor in
        try? await Task.sleep(for: .milliseconds(milliseconds))
        if let waiter = waiters.removeValue(forKey: id) {
          waiter.resume(throwing: HostFailure.check(name))
        }
      }
    }
  }
}
@MainActor final class TransportPump {
  let owner: Owner
  let transport: ([String]) async throws -> String
  private var task: Task<Void, Never>?
  private var timer: Task<Void, Never>?
  private var running = false, closed = false
  private var pendingResponse: String?
  var onError: ((Error) -> Void)?
  init(_ owner: Owner, transport: @escaping ([String]) async throws -> String) {
    self.owner = owner
    self.transport = transport
    owner.onTransition = { [weak self] in self?.wake() }
    owner.onClose = { [weak self] in self?.close() }
  }
  func wake() {
    guard !closed && owner.open else { return }
    timer?.cancel()
    timer = nil
    guard !running else { return }
    owner.host.whenIdle(owner) { [weak self] in
      guard let self else { return }
      if let response = self.pendingResponse { self.accept(response) } else { self.start() }
    }
  }
  private func start() {
    guard !closed && owner.open && !running else { return }
    do {
      let page = try pushedPage(owner, owner.clock())
      let envelopes = page.envelopes
      guard !envelopes.isEmpty else {
        if page.hasMore { wake() } else { schedule(page.due) }
        return
      }
      running = true
      task = Task { @MainActor in
        do {
          let response = try await transport(envelopes)
          guard !closed && owner.open else { return }
          pendingResponse = response
          owner.host.whenIdle(owner) { [weak self] in self?.accept(response) }
        } catch {
          guard !closed && owner.open else { return }
          running = false
          onError?(error)
          owner.host.whenIdle(owner) { [weak self] in self?.wake() }
        }
      }
    } catch { onError?(error) }
  }
  private func accept(_ response: String) {
    guard !closed && owner.open else { return }
    // An explicit retained-response retry must also suppress transition wakes
    // until its per-key reports have been inspected.
    running = true
    do {
      let reports = try list(owner.rpc("receive_wire", ["body": response])).map(dictionary)
      if let failed = reports.first(where: { $0["ok"] as? Bool != true }) {
        throw HostFailure.rpc(try text(dictionary(failed["error"] as Any)["kind"] as Any))
      }
      pendingResponse = nil
      try owner.drain(owner.clock())
    } catch {
      // Retain owned response for an explicit wake; never replace a failed
      // durable receipt with a blind retransmission of its envelope.
      running = false
      onError?(error)
      return
    }
    running = false
    wake()
  }
  private func schedule(_ due: Int64?) {
    guard let due, !closed && owner.open else { return }
    let now = owner.clock()
    let delta = due > now ? due - now : 0
    timer = Task { @MainActor in
      do {
        try await Task.sleep(for: .milliseconds(delta))
        try Task.checkCancellation()
        wake()
      } catch {}
    }
  }
  func close() {
    closed = true
    timer?.cancel()
    task?.cancel()
    timer = nil
    task = nil
    pendingResponse = nil
  }
}
@MainActor func h06Timer(_ root: String, _ host: CommandHost) async throws {
  let owner = try fixtureOwner(
    root + "/phase-four-h06-timer.sqlite", host: host,
    options: [
      "capacity": "1000", "history": "100", "initial_delay_ms": "30", "multiplier": "2",
      "max_delay_ms": "1000",
    ])
  defer { try? owner.close() }
  owner.clock = { Int64(DispatchTime.now().uptimeNanoseconds / 1_000_000) }
  let completed = DeadlineSignal()
  var attempts = 0
  var firstBytes = Data()
  var laterBytes = Data()
  let pump = TransportPump(owner) { envelopes in
    attempts += 1
    let bytes = try Transport.body(envelopes)
    if attempts == 1 {
      firstBytes = bytes
      throw HostFailure.transport
    }
    laterBytes = bytes
    let response = try await Transport.send(envelopes)
    completed.fire()
    return response
  }
  let job = try host.submit(owner) { _, _, _ in .null }
  _ = try await job.value()
  try await completed.wait(name: "H06 retry timer did not fire")
  try require(
    attempts == 2 && firstBytes == laterBytes,
    "H06 timer wakes frozen retry after fake transport failure using actual one-command URLSession")
  // Await the callback transition into received state, without adding a polling loop.
  let settled = DeadlineSignal()
  _ = try owner.observe("observe_outcome", args: ["key": text(job.result!.get()["key"] as Any)]) {
    event in
    if try text(dictionary(event["value"] as Any)["state"] as Any) == "acknowledged" {
      settled.fire()
    }
  }
  try owner.drain(owner.clock())
  try await settled.wait(name: "H06 timer final was not delivered")
  pump.close()
}
@MainActor func h06Overlap(_ root: String, _ host: CommandHost) async throws {
  let owner = try fixtureOwner(root + "/phase-four-h06-overlap.sqlite", host: host)
  defer { try? owner.close() }
  let network = Gate()
  let bodyOwned = Signal()
  let accepted = DeadlineSignal()
  let pump = TransportPump(owner) { envelopes in
    await network.pause()
    let body = try await Transport.send(envelopes)
    bodyOwned.fire()
    return body
  }
  let first = try host.submit(owner) { _, _, _ in .null }
  let key = try text(await first.value()["key"] as Any)
  await network.entered.wait()
  _ = try owner.observe("observe_outcome", args: ["key": key]) { event in
    if try text(dictionary(event["value"] as Any)["state"] as Any) == "acknowledged" {
      accepted.fire()
      pump.close()
    }
  }
  try owner.drain()
  let handler = Gate()
  let second = try host.submit(owner) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("HTTP overlap"))
    await handler.pause()
    return .null
  }
  try await second.admitted()
  await handler.entered.wait()
  network.release()
  await bodyOwned.wait()
  try require(
    try list(owner.rpc("pending_reconciliations")).isEmpty,
    "H06 in-flight receipt does not mutate while handler holds permit")
  handler.release()
  _ = try await second.value()
  try await accepted.wait(name: "H06 overlapping HTTP final was dropped")
  try require(
    try list(owner.rpc("pending_reconciliations")).count == 1,
    "H06 owned HTTP response deferred until shared dispatcher idle")
  pump.close()
}
@MainActor func h06Reports(_ root: String, _ host: CommandHost) async throws {
  let owner = try fixtureOwner(root + "/phase-four-h06-reports.sqlite", host: host)
  defer { try? owner.close() }
  var calls = 0
  var errors = 0
  let firstError = DeadlineSignal()
  let secondError = DeadlineSignal()
  let pump = TransportPump(owner) { envelopes in
    calls += 1
    return try await Transport.send(envelopes, mode: "conflict")
  }
  pump.onError = { error in
    if case HostFailure.rpc("Protocol") = error {
      errors += 1
      if errors == 1 { firstError.fire() } else { secondError.fire() }
    }
  }
  let job = try host.submit(owner) { _, _, _ in .null }
  _ = try await job.value()
  try await firstError.wait(name: "H06 per-member receipt error was hidden")
  try require(
    calls == 1 && list(owner.rpc("pending_reconciliations")).count == 1,
    "H06 successful member retained when another report conflicts")
  pump.wake()
  try await secondError.wait(name: "H06 retained response was not retried explicitly")
  try require(
    calls == 1 && errors == 2,
    "H06 explicit receipt retry reuses owned response without HTTP retransmit")
  pump.close()
}
@MainActor func h03Bound(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-h03-bound.sqlite", host: host)
  defer { try? owner.close() }
  var job: CommandTask?
  let limit = 1_048_576
  job = try host.submit(owner) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("must not commit"))
    let empty: [String: Any] = [
      "op": "finish", "owner": owner.handle,
      "args": ["request": job!.request, "result": Tagged.string("").wire],
    ]
    let overhead = try JSONSerialization.data(withJSONObject: empty, options: [.sortedKeys]).count
    let count = limit - overhead
    let result = Tagged.string(String(repeating: "x", count: count))
    let frame: [String: Any] = [
      "ok": true, "value": ["key": String(repeating: "a", count: 66), "result": result.wire],
    ]
    try require(
      try JSONSerialization.data(withJSONObject: frame).count > limit,
      "H03 near-bound request actually predicts oversized success")
    return result
  }
  do {
    _ = try await job!.value()
    throw HostFailure.check("H03 oversized finish success accepted")
  } catch HostFailure.rpc("LimitExceeded") {}
  try require(
    try owner.fetch("orders").isEmpty
      && text(dictionary(owner.rpc("queue_status"))["live"] as Any) == "0",
    "H03 failed oversized finish committed rows or envelope")
  let next = try host.submit(owner) { scope, _, _ in
    try require(
      try text(scope.insert("orders", orderFields("counter"))["id"] as Any) == "local:1",
      "H03 failed oversized finish consumed counter")
    throw HostFailure.handler
  }
  do { _ = try await next.value() } catch HostFailure.handler {}
}
@MainActor func h04Pages(_ root: String) async throws {
  let owner = try fixtureOwner(root + "/phase-four-h04-pages.sqlite", host: CommandHost())
  defer { try? owner.close() }
  let large = String(repeating: "x", count: 400_000)
  _ = try owner.rpc(
    "direct_batch",
    ["writes": [["op": "insert", "table": "orders", "fields": fields(orderFields(large))]]])
  let id = try text(owner.fetch("orders")[0]["id"] as Any)
  var tokens: [String] = []
  var sizes: [Int] = []
  var armed = true
  var suppressed = 0
  for index in 0..<4 {
    let token = try owner.observe(
      "observe_data", args: ["query": query("orders"), "mode": "columns", "columns": ["label"]]
    ) { event in
      if index == 1 { suppressed += 1 }
      let snapshot = try list(dictionary(event["value"] as Any)["value"] as Any).map(dictionary)
      guard
        case .string(let label) = try Tagged(
          wire: dictionary(snapshot[0]["fields"] as Any)["label"] as Any)
      else { throw HostFailure.check("H04 page String") }
      sizes.append(label.utf8.count)
      if index == 0 && armed {
        armed = false
        try owner.unsubscribe(tokens[1])
        _ = try owner.rpc(
          "direct_batch",
          [
            "writes": [
              [
                "op": "replace", "table": "orders", "id": id,
                "fields": fields(orderFields("fresh")),
              ]
            ]
          ])
        do {
          try owner.drain()
          throw HostFailure.check("H04 recursive paged drain accepted")
        } catch HostFailure.rpc("Busy") {}
      }
    }
    tokens.append(token)
  }
  try owner.drain()
  try require(
    sizes == [400_000, 400_000, 400_000] && suppressed == 0,
    "H04 paged frozen snapshot lost or pulled callback-generated events")
  try owner.drain()
  try require(
    sizes == [400_000, 400_000, 400_000, 5, 5, 5],
    "H04 callback-generated events wait for later explicit drain: sizes="
      + String(describing: sizes) + " failures=" + String(owner.callbackFailures))
  let closing = try fixtureOwner(root + "/phase-four-h04-pages-close.sqlite", host: owner.host)
  _ = try closing.rpc(
    "direct_batch",
    ["writes": [["op": "insert", "table": "orders", "fields": fields(orderFields(large))]]])
  var calls = 0
  for _ in 0..<4 {
    _ = try closing.observe(
      "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
    ) { _ in
      calls += 1
      try closing.close()
    }
  }
  try closing.drain()
  try require(calls == 1, "H04 close suppresses remaining page callbacks")
  let oversized = try fixtureOwner(
    root + "/phase-four-h04-oversized-event.sqlite", host: owner.host)
  defer { try? oversized.close() }
  for _ in 0..<2 {
    _ = try oversized.rpc(
      "direct_batch",
      [
        "writes": [
          [
            "op": "insert", "table": "orders",
            "fields": fields(orderFields(String(repeating: "x", count: 600_000))),
          ]
        ]
      ])
  }
  var boundedCalls = 0
  let unbounded = try oversized.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []]
  ) { _ in throw HostFailure.check("H04 oversized snapshot delivered") }
  for _ in 0..<2 {
    do {
      try oversized.drain()
      throw HostFailure.check("H04 oversized snapshot consumed silently")
    } catch HostFailure.rpc("LimitExceeded") {}
  }
  try oversized.unsubscribe(unbounded)
  var bounded = query("orders")
  bounded["limit"] = "1"
  _ = try oversized.observe(
    "observe_data", args: ["query": bounded, "mode": "rows", "columns": []]
  ) { _ in boundedCalls += 1 }
  try oversized.drain()
  try require(boundedCalls == 0, "H04 cancelled empty cached snapshot pulled fresh core events")
  try oversized.drain()
  try require(
    boundedCalls == 1, "H04 cancellation releases oversized snapshot for later bounded query")
}
@MainActor func h06Pages(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-h06-pages.sqlite", host: host)
  defer { try? owner.close() }
  var keys: [String] = []
  for _ in 0..<4 {
    let job = try host.submit(
      owner, definition: definition(["value": "String"]),
      args: .object(["value": .string(String(repeating: "x", count: 300_000))])
    ) { _, _, _ in .null }
    keys.append(try text(await job.value()["key"] as Any))
  }
  var transportError: String?
  var arrays: [Int] = []
  var received = Set<String>()
  let done = DeadlineSignal()
  let pump = TransportPump(owner) { envelopes in
    let bytes = try Transport.body(envelopes)
    try require(
      bytes.count <= 1_048_576 && !envelopes.isEmpty,
      "H06 each page is a bounded nonempty HTTP array")
    arrays.append(envelopes.count)
    print("H06 bounded HTTP array members=" + String(envelopes.count))
    return try await Transport.send(envelopes)
  }
  pump.onError = { error in
    transportError = String(describing: error)
    done.fire()
  }
  for key in keys {
    _ = try owner.observe("observe_outcome", args: ["key": key]) { event in
      if try text(dictionary(event["value"] as Any)["state"] as Any) == "acknowledged" {
        received.insert(key)
        if received.count == 4 {
          done.fire()
          pump.close()
        }
      }
    }
  }
  pump.wake()
  try await done.wait(milliseconds: 900_000, name: "H06 push prefix continuation did not complete")
  if let transportError { throw HostFailure.check("H06 bounded push error: " + transportError) }
  try require(
    arrays.count >= 2 && arrays.reduce(0, +) == 4 && received == Set(keys),
    "H06 aggregate push failed after Sent persistence or skipped cached prefixes")
  pump.close()
}
@MainActor func h06Plan(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-h06-plan.sqlite", host: host)
  defer { try? owner.close() }
  let (key, local) = try await createOrder(host, owner, "application plan")
  let body = try await Transport.send(pushed(owner, 1000).0)
  _ = try owner.rpc("receive_wire", ["body": body])
  let mapper = Gate()
  let handler = Gate()
  let attempted = Signal()
  var completed = false
  let planTask = Task { () -> Result<Void, Error> in
    await mapper.pause()
    attempted.fire()
    do {
      _ = try await host.atIdle(owner) {
        try owner.rpc(
          "reconcile",
          ["key": key, "plan": ["writes": [], "mappings": [local: "server:application-plan"]]])
      }
      completed = true
      return .success(())
    } catch {
      completed = true
      return .failure(error)
    }
  }
  await mapper.entered.wait()
  let command = try host.submit(owner) { scope, _, _ in
    _ = try scope.insert("orders", orderFields("separate command"))
    await handler.pause()
    return .null
  }
  defer { handler.release() }
  try await command.admitted()
  await handler.entered.wait()
  mapper.release()
  await attempted.wait()
  try require(!completed, "H06 async application plan was not retained during active handler")
  handler.release()
  _ = try await command.value()
  try await planTask.value.get()
  try require(
    try owner.fetch("orders").contains { $0["id"] as? String == "server:application-plan" },
    "H06 retained application plan applied after idle")
  let closeGate = Gate()
  let blocking = try host.submit(owner) { _, _, _ in
    await closeGate.pause()
    return .null
  }
  try await blocking.admitted()
  await closeGate.entered.wait()
  let cancelled = Task { try await host.atIdle(owner) { try owner.fetch("orders") } }
  await Task.yield()
  cancelled.cancel()
  do {
    _ = try await cancelled.value
    throw HostFailure.check("H06 cancelled deferred plan/read applied")
  } catch is CancellationError {}
  let waiting = Task { try await host.atIdle(owner) { try owner.fetch("orders") } }
  await Task.yield()
  try owner.close()
  do {
    _ = try await waiting.value
    throw HostFailure.check("H06 closed deferred plan/read applied")
  } catch HostFailure.closed {}
  closeGate.release()
}
// API-0009 / TC-0034 E20: additive reference-host methods. Domain bodies stay opaque.
@MainActor extension Owner {
  func configureEntities(scope: String = "native:phase-five", tables: [String] = ["orders"]) throws
  {
    _ = try rpc("entity_configure", ["scope": scope, "tables": tables])
  }
  func entityBatch(_ writes: [[String: Any]]) throws -> [[String: Any]] {
    guard let context else { throw HostFailure.rpc("InvalidCommand") }
    let identity = try context()
    let now = clock()
    return try list(
      rpc("entity_batch", ["writes": writes, "identity": identity.wire, "now_ms": String(now)])
    ).map(dictionary)
  }
  func patchEntity(_ table: String, _ id: String, _ values: [String: Tagged]) throws -> [String:
    Any]
  {
    guard let context else { throw HostFailure.rpc("InvalidCommand") }
    let identity = try context()
    let now = clock()
    return try dictionary(
      rpc(
        "entity_patch",
        [
          "table": table, "id": id, "fields": fields(values), "identity": identity.wire,
          "now_ms": String(now),
        ]))
  }
  func resetEntities(_ mode: String = "preserve") throws {
    guard !host.hasCommandWork else { throw HostFailure.rpc("Busy") }
    _ = try rpc("reset", ["mode": mode])
    // Core failure leaves every host lifetime intact. Only committed reset
    // invalidates callbacks, deferred work and the old transport pump.
    generation += 1
    callbacks.removeAll()
    onClose?()
    onClose = nil
    onTransition = nil
    host.closed(self)
  }
  func uploadPull(_ request: String, body: String) throws -> Int {
    let bytes = Array(body.utf8)
    guard bytes.count <= 1_048_576 else { throw HostFailure.rpc("LimitExceeded") }
    var offset = 0
    var chunks = 0
    while offset < bytes.count {
      // 128KiB still fits the unchanged complete frame even when every input
      // scalar escapes to six ASCII bytes. Never split a UTF-8 code point.
      var end = min(offset + 131072, bytes.count)
      while end < bytes.count && (bytes[end] & 0xc0) == 0x80 { end -= 1 }
      guard let chunk = String(bytes: bytes[offset..<end], encoding: .utf8) else {
        throw HostFailure.rpc("InvalidPayload")
      }
      let next = try text(
        rpc("pull_chunk", ["request": request, "offset": String(offset), "chunk": chunk]))
      try require(next == String(end), "pull chunk acknowledged exact UTF8 byte offset")
      offset = end
      chunks += 1
    }
    return chunks
  }
  func pull(_ transport: @escaping ([String: Any]) async throws -> String) async throws {
    let expectedGeneration = generation
    while open && generation == expectedGeneration {
      let batch = try dictionary(rpc("begin_pull"))
      let request = try text(batch["request"] as Any)
      do {
        let body = try await transport(batch)
        guard open && generation == expectedGeneration else { throw HostFailure.closed }
        _ = try uploadPull(request, body: body)
        _ = try await host.atIdle(self) { try self.rpc("apply_pull", ["request": request]) }
        return
      } catch {
        if open && generation == expectedGeneration {
          _ = try? rpc("abort_pull", ["request": request])
        }
        if case HostFailure.rpc("StalePull") = error, open && generation == expectedGeneration {
          continue
        }
        throw error
      }
    }
    throw HostFailure.closed
  }
  func entries() throws -> [[String: Any]] {
    try list(dictionary(rpc("queue_status"))["commands"] as Any).map(dictionary)
  }
}
extension Transport {
  static func pull(_ batch: [String: Any], mode: String = "normal") async throws -> String {
    try Task.checkCancellation()
    var components = URLComponents(url: try endpoint("changes"), resolvingAgainstBaseURL: false)!
    if let since = batch["since"] as? String {
      components.queryItems = [URLQueryItem(name: "since", value: since)]
    }
    let configuration = URLSessionConfiguration.ephemeral
    configuration.timeoutIntervalForRequest = 5
    configuration.timeoutIntervalForResource = 5
    let session = URLSession(configuration: configuration)
    defer { session.invalidateAndCancel() }
    var request = URLRequest(url: components.url!)
    request.timeoutInterval = 5
    request.setValue(mode, forHTTPHeaderField: "X-Hoya-Fixture")
    request.setValue("Bearer synthetic-native-fixture", forHTTPHeaderField: "Authorization")
    let (data, response) = try await session.data(for: request)
    try Task.checkCancellation()
    guard let response = response as? HTTPURLResponse, response.statusCode == 200,
      data.count <= 1_048_576, let body = String(data: data, encoding: .utf8)
    else { throw HostFailure.transport }
    return body
  }
}
@MainActor func entityInsert(_ owner: Owner, label: String) throws -> [String: Any] {
  let changes = try owner.entityBatch([
    ["op": "insert", "table": "orders", "fields": fields(orderFields(label))]
  ])
  return try dictionary(changes[0]["after"] as Any)
}
@MainActor func eN01(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-entity-writes.sqlite", host: host)
  defer { try? owner.close() }
  try owner.configureEntities()
  var samples = 0
  owner.context = {
    samples += 1
    return .object(["user": .string("native entity"), "max": .int(Int64.max)])
  }
  let created = try entityInsert(owner, label: "create 東京\0🌿")
  let id = try text(created["id"] as Any)
  try require(
    id.hasPrefix("entity:") && samples == 1,
    "E-N01 namespaced create and one trusted context sample")
  let firstKey = try text(owner.entries()[0]["key"] as Any)
  let creation = try pushed(owner, 1000).0
  try require(
    creation.count == 1 && creation[0].contains("\"writeId\"")
      && !creation[0].contains("idempotencyKey"),
    "E-N01 entity creation has correlation but no command key")
  _ = try owner.rpc("receive_wire", ["body": await Transport.send(creation)])
  _ = try owner.rpc("reconcile", ["key": firstKey, "plan": ["writes": [], "mappings": [id: id]]])
  _ = try owner.patchEntity("orders", id, ["label": .string("create 東京\0🌿")])
  var replacement = orderFields("replace")
  replacement.removeValue(forKey: "notes")
  _ = try owner.entityBatch([
    ["op": "replace", "table": "orders", "id": id, "fields": fields(replacement)]
  ])
  var commandID = ""
  let command = try host.submit(owner) { scope, _, _ in
    commandID = try text(scope.insert("orders", orderFields("command-scope"))["id"] as Any)
    return .null
  }
  _ = try await command.value()
  let commandKey = try text(command.result!.get()["key"] as Any)
  let entries = try owner.entries()
  try require(entries.count == 4, "E-N01 command scopes do not generate secondary entity diffs")
  let envelopes = try pushed(owner, 1000).0
  try require(
    envelopes.count == 3 && envelopes[0].contains("\"writeId\"")
      && !envelopes[0].contains("idempotencyKey"),
    "E-N01 mixed queue uses discriminated entity wire")
  try require(
    envelopes[0].contains("\"changes\":{\"label\":") && !envelopes[0].contains("\"total\":"),
    "E-N01 same-value patch contains only supplied field")
  try require(
    envelopes[1].contains("\"removedFields\":[\"notes\"]")
      && envelopes[1].contains("\"notes\":null"),
    "E-N01 replacement omission differs from present Null")
  let body = try await Transport.send(envelopes)
  let reports = try list(owner.rpc("receive_wire", ["body": body])).map(dictionary)
  try require(
    reports.count == 3 && reports.allSatisfy { $0["ok"] as? Bool == true },
    "E-N01 real mixed HTTP correlates reordered members")
  let pending = try list(owner.rpc("pending_reconciliations")).map(dictionary)
  for value in pending {
    let key = try text(value["key"] as Any)
    let mappings = key == commandKey ? [commandID: "server:native-command"] : [:]
    _ = try owner.rpc("reconcile", ["key": key, "plan": ["writes": [], "mappings": mappings]])
  }
  try require(
    try text(dictionary(owner.rpc("queue_status"))["live"] as Any) == "0",
    "E-N01 mixed authoritative identity map settles shared queue")
  _ = try owner.entityBatch([["op": "remove", "table": "orders", "id": id]])
  let delete = try owner.entries().last!
  let deletion = try pushed(owner, 1000).0
  try require(
    deletion.count == 1 && deletion[0].contains("\"op\":\"delete\"")
      && deletion[0].contains("\"changes\":{}"), "E-N01 delete has no changed fields")
  _ = try owner.rpc("cancel", ["key": text(delete["key"] as Any)])
  try require(
    try owner.fetch("orders").count == 2,
    "E-N01 explicit cancellation restores rejected deletion and comparison undo")
  print(
    "E-N01 initial create=1 then mixed envelopes=3 identity preserved context samples=\(samples)")
  try await entityCorrectionClocks(root)
}
func entityChangeFrame(_ id: String, values: [String: Tagged], time: Int64, op: String = "update")
  -> [String: Any]
{
  [
    "table": "orders", "record_id": id, "op": op, "changes": fields(values), "removed_fields": [],
    "timestamp_ms": String(time), "field_timestamps": values.mapValues { _ in String(time) },
  ]
}
func entityPageFrame(_ checkpoint: String, change: [String: Any]) throws -> String {
  let page: [String: Any] = ["checkpoint": checkpoint, "complete": true, "changes": [change]]
  try outerOnly(page)
  return String(
    decoding: try JSONSerialization.data(withJSONObject: page, options: [.sortedKeys]),
    as: UTF8.self)
}
@MainActor func entityCorrectionClocks(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-entity-correction.sqlite", host: host)
  defer { try? owner.close() }
  try owner.configureEntities()
  let id = "server:native-clock"
  let initial = try entityPageFrame(
    "clock:seed",
    change: entityChangeFrame(id, values: orderFields("baseline"), time: 10, op: "create"))
  try await owner.pull { _ in initial }
  let command = try host.submit(owner) { _, _, _ in .null }
  let commandKey = try text(await command.value()["key"] as Any)
  _ = try owner.patchEntity("orders", id, ["label": .string("optimistic1000")])
  let diffKey = try text(owner.entries().last!["key"] as Any)
  let envelopes = try pushed(owner, 1000).0
  let response = try await Transport.send(envelopes, mode: "mixed")
  try require(
    envelopes.count == 2 && response.contains("\"status\":\"corrected\""),
    "E-N01 real backend returns entity correction in mixed array")
  let reports = try list(owner.rpc("receive_wire", ["body": response])).map(dictionary)
  try require(
    reports.allSatisfy { $0["ok"] as? Bool == true }, "E-N01 independent mixed correction receipts")
  _ = try owner.rpc("reconcile", ["key": commandKey, "plan": ["writes": [], "mappings": [:]]])
  let correction = entityChangeFrame(id, values: ["label": .string("authoritative25")], time: 25)
  _ = try owner.rpc(
    "reconcile",
    [
      "key": diffKey,
      "plan": ["writes": [["op": "entity", "change": correction]], "mappings": [:]],
    ])
  func label() throws -> Tagged {
    try Tagged(wire: dictionary(owner.fetch("orders")[0]["fields"] as Any)["label"] as Any)
  }
  try require(try label() == .string("authoritative25"), "E-N01 correction value applied")
  let older = try entityPageFrame(
    "clock:older", change: entityChangeFrame(id, values: ["label": .string("older24")], time: 24))
  try await owner.pull { _ in older }
  try require(try label() == .string("authoritative25"), "E-N01 correction clock25 rejects older24")
  let newer = try entityPageFrame(
    "clock:newer", change: entityChangeFrame(id, values: ["label": .string("newer26")], time: 26))
  try await owner.pull { _ in newer }
  try require(
    try label() == .string("newer26"),
    "E-N01 released optimistic1000 permits authoritative26 after correction25")
  print("E-N01 correction clock1000->25; older24 kept/newer26 applied")
}
@MainActor func eN02(_ root: String) async throws {
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-entity-pull.sqlite", host: host)
  defer { try? owner.close() }
  try owner.configureEntities()
  var progress: [String] = []
  var values: [String] = []
  _ = try owner.observe("observe_progress") { event in
    progress.append(try text(dictionary(event["value"] as Any)["pages"] as Any))
  }
  _ = try owner.observe(
    "observe_data", args: ["query": query("customers"), "mode": "columns", "columns": ["country"]]
  ) { event in
    let rows = try list(dictionary(event["value"] as Any)["value"] as Any).map(dictionary)
    if let row = rows.first {
      values.append(
        String(describing: try Tagged(wire: dictionary(row["fields"] as Any)["country"] as Any)))
    }
  }
  try owner.drain()
  var since: [String] = []
  try await owner.pull { batch in
    since.append(batch["since"] as? String ?? "<null>")
    return try await Transport.pull(batch)
  }
  try owner.drain()
  var state = try dictionary(owner.rpc("sync_progress"))
  try require(
    state["ready"] as? Bool == false && state["pages"] as? String == "1",
    "E-N02 first actual HTTP page is partial")
  try await owner.pull { batch in
    since.append(batch["since"] as? String ?? "<null>")
    return try await Transport.pull(batch)
  }
  try owner.drain()
  state = try dictionary(owner.rpc("sync_progress"))
  try require(
    state["ready"] as? Bool == true && state["pages"] as? String == "2"
      && since == ["<null>", "fixture:page-one"],
    "E-N02 exact opaque continuation and sticky readiness")
  try require(
    progress == ["0", "1", "2"] && values.count == 2, "E-N02 postcommit data and progress callbacks"
  )
  let request = try dictionary(owner.rpc("begin_pull"))
  do {
    _ = try owner.rpc(
      "pull_chunk", ["request": text(request["request"] as Any), "offset": "1", "chunk": "{}"])
    throw HostFailure.check("E-N02 out-of-order chunk accepted")
  } catch HostFailure.rpc("InvalidRequest") {}
  _ = try owner.rpc("abort_pull", ["request": text(request["request"] as Any)])
  let large = try fixtureOwner(root + "/phase-four-entity-large.sqlite", host: host)
  defer { try? large.close() }
  try large.configureEntities()
  let batch = try dictionary(large.rpc("begin_pull"))
  let body = try await Transport.pull(batch, mode: "large")
  try require(body.utf8.count > 262144, "E-N02 valid large response crosses chunk boundary")
  let chunks = try large.uploadPull(text(batch["request"] as Any), body: body)
  try require(
    chunks > 1 && large.fetch("customers").isEmpty,
    "E-N02 chunk upload never publishes partial rows")
  _ = try large.rpc("apply_pull", ["request": text(batch["request"] as Any)])
  try require(try large.fetch("customers").count == 1, "E-N02 full large UTF8 body applied once")
  print("E-N02 pages=2 large bytes=\(body.utf8.count) chunks=\(chunks)")
}
@MainActor func eN03(_ root: String) async throws {
  try await protectedPullRollback(root)
  let host = CommandHost()
  let owner = try fixtureOwner(root + "/phase-four-entity-lifetime.sqlite", host: host)
  defer { try? owner.close() }
  try owner.configureEntities()
  let ticket = try dictionary(owner.rpc("begin_pull"))
  let body = try await Transport.pull(ticket)
  _ = try owner.uploadPull(text(ticket["request"] as Any), body: body)
  let gate = Gate()
  let active = try host.submit(owner) { _, _, _ in
    await gate.pause()
    throw HostFailure.handler
  }
  try await active.admitted()
  await gate.entered.wait()
  do {
    _ = try owner.rpc("apply_pull", ["request": text(ticket["request"] as Any)])
    throw HostFailure.check("E-N03 active handler allowed pull")
  } catch HostFailure.rpc("Busy") {}
  let generation = owner.generation
  do {
    try owner.resetEntities("discard")
    throw HostFailure.check("E-N03 active reset accepted")
  } catch HostFailure.rpc("Busy") {}
  try require(owner.generation == generation, "E-N03 failed Busy reset retains generation")
  let applying = Task {
    try await host.atIdle(owner) {
      try owner.rpc("apply_pull", ["request": text(ticket["request"] as Any)])
    }
  }
  await Task.yield()
  active.cancel()
  gate.release()
  _ = try await applying.value
  try require(
    try owner.fetch("customers").count == 1,
    "E-N03 original Busy ticket/body retained until handler abort")
  let network = Gate()
  var calls = 0
  let stale = Task {
    try await owner.pull { batch in
      calls += 1
      let body = try await Transport.pull(batch)
      if calls == 1 { await network.pause() }
      return body
    }
  }
  await network.entered.wait()
  _ = try owner.rpc(
    "direct_batch",
    [
      "writes": [
        [
          "op": "insert", "table": "customers",
          "fields": fields(["country": .string("unrelated local")]),
        ]
      ]
    ])
  network.release()
  try await stale.value
  try require(
    calls == 2 && owner.fetch("customers").count == 2,
    "E-N03 StalePull refetches under fresh ticket and preserves independent local write")
  _ = try entityInsert(owner, label: "pending reset")
  let oldGeneration = owner.generation
  var retainedCallbacks = 0
  let retainedToken = try owner.observe("observe_status") { _ in retainedCallbacks += 1 }
  do {
    try owner.resetEntities()
    throw HostFailure.check("E-N03 default reset lost recovery")
  } catch HostFailure.rpc("Protocol") {}
  try require(
    owner.generation == oldGeneration && owner.fetch("orders").count == 1,
    "E-N03 failed preserve reset has no lifetime effect")
  try owner.drain()
  try require(
    retainedCallbacks == 1, "E-N03 failed reset preserves existing subscription and owned snapshot")
  try owner.unsubscribe(retainedToken)
  var callbacks = 0
  let invalidate: ([String: Any]) throws -> Void = { _ in
    callbacks += 1
    try owner.resetEntities("discard")
  }
  _ = try owner.observe(
    "observe_data", args: ["query": query("orders"), "mode": "rows", "columns": []],
    callback: invalidate)
  _ = try owner.observe("observe_status", callback: invalidate)
  _ = try owner.observe(
    "observe_outcome", args: ["key": text(owner.entries().last!["key"] as Any)],
    callback: invalidate)
  _ = try owner.observe("observe_progress", callback: invalidate)
  try owner.drain()
  try require(
    callbacks == 1 && owner.generation == oldGeneration + 1 && owner.fetch("customers").isEmpty
      && owner.fetch("orders").isEmpty,
    "E-N03 successful reset stops remaining extracted old-generation frames")
  let held = Gate()
  let late = Task {
    try await owner.pull { batch in
      let body = try await Transport.pull(batch)
      await held.pause()
      return body
    }
  }
  await held.entered.wait()
  try owner.resetEntities("discard")
  held.release()
  do {
    try await late.value
    throw HostFailure.check("E-N03 pre-reset pull mutated new generation")
  } catch HostFailure.closed {}
  try require(
    try dictionary(owner.rpc("sync_progress"))["pages"] as? String == "0",
    "E-N03 late pull progress remains reset")
  try await owner.pull { try await Transport.pull($0) }
  try require(try owner.fetch("customers").count == 1, "E-N03 fresh-generation null pull works")
  let oldNetwork = Gate()
  let newNetwork = Gate()
  let settled = DeadlineSignal()
  var oldCalls = 0
  var newCalls = 0
  let oldPump = TransportPump(owner) { envelopes in
    oldCalls += 1
    let response = try await Transport.send(envelopes)
    await oldNetwork.pause()
    return response
  }
  _ = try entityInsert(owner, label: "old transport generation")
  await oldNetwork.entered.wait()
  try owner.resetEntities("discard")
  let newPump = TransportPump(owner) { envelopes in
    newCalls += 1
    let response = try await Transport.send(envelopes)
    await newNetwork.pause()
    return response
  }
  defer {
    oldPump.close()
    newPump.close()
  }
  _ = try entityInsert(owner, label: "replacement transport generation")
  let newKey = try text(owner.entries().last!["key"] as Any)
  _ = try owner.observe("observe_outcome", args: ["key": newKey]) { event in
    if try text(dictionary(event["value"] as Any)["state"] as Any) == "acknowledged" {
      settled.fire()
    }
  }
  await newNetwork.entered.wait()
  oldNetwork.release()
  await Task.yield()
  newNetwork.release()
  try await settled.wait(name: "E-N03 old response blocked replacement pump")
  try require(
    oldCalls == 1 && newCalls == 1 && owner.entries().count == 1
      && owner.fetch("orders").count == 1,
    "E-N03 pre-reset send cannot mutate or clear replacement generation pump")
}
@MainActor func protectedPullRollback(_ root: String) async throws {
  let owner = try fixtureOwner(
    root + "/phase-four-entity-protected-pull.sqlite", host: CommandHost())
  defer { try? owner.close() }
  try owner.configureEntities()
  let id = "server:protected"
  let seed = try entityPageFrame(
    "protected:seed",
    change: entityChangeFrame(id, values: orderFields("old10"), time: 10, op: "create"))
  try await owner.pull { _ in seed }
  _ = try owner.entityBatch([["op": "remove", "table": "orders", "id": id]])
  let deleteKey = try text(owner.entries().last!["key"] as Any)
  let response = try entityPageFrame(
    "protected:server20",
    change: entityChangeFrame(id, values: ["label": .string("server20")], time: 20))
  do {
    try await owner.pull { _ in response }
    throw HostFailure.check("E-N03 optimistic tombstone checkpointed away authoritative update")
  } catch HostFailure.rpc("PullConflict") {}
  _ = try owner.rpc("cancel", ["key": deleteKey])
  var since = ""
  try await owner.pull { batch in
    since = try text(batch["since"] as Any)
    return response
  }
  let label = try Tagged(
    wire: dictionary(owner.fetch("orders")[0]["fields"] as Any)["label"] as Any)
  try require(
    since == "protected:seed" && label == .string("server20"),
    "E-N03 rollback then refetch applies preserved authoritative page")
}
@MainActor func entityRestartWriter(_ root: String) async throws {
  let owner = try fixtureOwner(root + "/phase-four-entity-restart.sqlite", host: CommandHost())
  try owner.configureEntities()
  try await owner.pull { try await Transport.pull($0) }
  let row = try entityInsert(owner, label: "durable optimistic entity")
  let (envelopes, due) = try pushed(owner, 1000)
  let state: [String: Any] = [
    "id": try text(row["id"] as Any), "envelope": envelopes[0], "due": String(due!),
  ]
  try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]).write(
    to: URL(fileURLWithPath: root + "/phase-four-entity-restart.json"), options: .atomic)
  print("E-N04 writer partial-bootstrap/queued-entity committed barrier")
  fflush(stdout)
  kill(getpid(), SIGKILL)
  throw HostFailure.check("SIGKILL unexpectedly returned")
}
@MainActor func entityRestartReader(_ root: String) async throws {
  let owner = try fixtureOwner(root + "/phase-four-entity-restart.sqlite", host: CommandHost())
  defer { try? owner.close() }
  try owner.configureEntities()
  let saved = try dictionary(
    JSONSerialization.jsonObject(
      with: Data(contentsOf: URL(fileURLWithPath: root + "/phase-four-entity-restart.json"))))
  let progress = try dictionary(owner.rpc("sync_progress"))
  try require(
    progress["ready"] as? Bool == false && progress["pages"] as? String == "1"
      && owner.fetch("orders").count == 1,
    "E-N04 independent process recovers partial checkpoint and optimistic row")
  let due = Int64(try text(saved["due"] as Any))!
  try require(
    try pushed(owner, due).0 == [text(saved["envelope"] as Any)],
    "E-N04 frozen diff bytes and due survive process death")
  var continuation = ""
  try await owner.pull { batch in
    continuation = try text(batch["since"] as Any)
    return try await Transport.pull(batch)
  }
  try require(
    continuation == "fixture:page-one"
      && dictionary(owner.rpc("sync_progress"))["ready"] as? Bool == true,
    "E-N04 real second HTTP page resumes committed checkpoint")
  do {
    try owner.resetEntities()
    throw HostFailure.check("E-N04 default reset discarded sent recovery")
  } catch HostFailure.rpc("Protocol") {}
  try owner.resetEntities("discard")
  let replacement = try entityInsert(owner, label: "new generation")
  try require(
    try text(replacement["id"] as Any) != text(saved["id"] as Any),
    "E-N04 reset never reuses entity identity")
  print("PASS E-N04 independent reader")
}
@MainActor func eN04(_ root: String) async throws {
  for role in ["entity-writer", "entity-reader"] {
    let child = Process()
    child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
    child.arguments = [role]
    child.environment = ProcessInfo.processInfo.environment
    try child.run()
    child.waitUntilExit()
    if role == "entity-writer" {
      try require(
        child.terminationReason == .uncaughtSignal && child.terminationStatus == SIGKILL,
        "E-N04 writer reached committed SIGKILL barrier")
    } else {
      try require(
        child.terminationReason == .exit && child.terminationStatus == 0,
        "E-N04 independent reader passed")
    }
  }
}
@MainActor func entityCases(_ root: String) async -> Int {
  var failures = 0
  for (name, run) in [("E-N01", eN01), ("E-N02", eN02), ("E-N03", eN03), ("E-N04", eN04)] {
    do {
      try await run(root)
      print("PASS " + name)
    } catch {
      failures += 1
      print("FAIL " + name + ": \(error)")
    }
  }
  print("native entities: \(4-failures) passed, \(failures) failed")
  return failures
}

// API-0010 / TC-0035 S08. This private reference host remains confined to
// the parent-created canonical synthetic root; it is not a public path API.
private struct NativeStoreProtection {
  let root: String
  let name: String
  let excluded: Bool

  init(root: String, name: String, key: [UInt8], excluded: Bool = true) throws {
    guard key.count == 32 else { throw HostFailure.rpc("InvalidRequest") }
    do { try Self.validateLocation(root: root, name: name) } catch {
      throw HostFailure.rpc("InvalidRequest")
    }
    self.root = root
    self.name = name
    self.excluded = excluded
  }

  var path: String { root + "/" + name }

  private static func validateLocation(root: String, name: String) throws {
    let invalid = HostFailure.rpc("InvalidRequest")
    guard name.hasPrefix("phase-four-"), name.hasSuffix(".sqlite"),
      name.count > "phase-four-.sqlite".count,
      name.utf8.allSatisfy({
        (48...57).contains($0) || (65...90).contains($0)
          || (97...122).contains($0) || $0 == 45 || $0 == 46
      }),
      !name.contains(".."), !root.contains("\0"), root.hasPrefix("/")
    else { throw invalid }
    let directory = URL(fileURLWithPath: root, isDirectory: true)
    let prefix = "hoya-phase-four-"
    let directoryName = directory.lastPathComponent
    guard directoryName.hasPrefix(prefix) else { throw invalid }
    let parts = directoryName.dropFirst(prefix.count).split(
      separator: "-", omittingEmptySubsequences: false)
    guard parts.count == 2, let pid = UInt32(parts[0]), pid > 0,
      String(pid) == parts[0], parts[1].count == 32,
      parts[1].utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) })
    else { throw invalid }
    let temporary = try physicalPath(FileManager.default.temporaryDirectory.path)
    guard try physicalPath(root) == root,
      directory.deletingLastPathComponent().path == temporary
    else { throw invalid }
    var component = "/"
    for part in root.split(separator: "/") {
      component = (component as NSString).appendingPathComponent(String(part))
      let attributes = try FileManager.default.attributesOfItem(atPath: component)
      guard attributes[.type] as? FileAttributeType == .typeDirectory else { throw invalid }
    }
    let path = root + "/" + name
    do {
      let attributes = try FileManager.default.attributesOfItem(atPath: path)
      guard attributes[.type] as? FileAttributeType == .typeRegular else { throw invalid }
    } catch let error as NSError
      where error.domain == NSCocoaErrorDomain
      && error.code == NSFileReadNoSuchFileError
    {
      // A new owned file is allowed; no other filesystem error is absence.
    }
  }

  func apply(using operation: (URL, Bool) throws -> Void = NativeStoreProtection.writePolicy) throws
  {
    do {
      try Self.validateLocation(root: root, name: name)
      let directory = URL(fileURLWithPath: root, isDirectory: true)
      try operation(directory, excluded)
      let values = try directory.resourceValues(forKeys: [.isExcludedFromBackupKey])
      guard values.isExcludedFromBackup == excluded else {
        throw HostFailure.rpc("BackupPolicyFailure")
      }
    } catch {
      throw HostFailure.rpc("BackupPolicyFailure")
    }
  }

  private static func writePolicy(_ directory: URL, _ excluded: Bool) throws {
    var directory = directory
    var values = URLResourceValues()
    values.isExcludedFromBackup = excluded
    try directory.setResourceValues(values)
  }

  func afterOwnedReplacement(ownerIsOpen: Bool) throws {
    guard !ownerIsOpen else { throw HostFailure.rpc("Busy") }
    try apply()
  }
}

private func backupValue(_ root: String) throws -> Bool {
  try URL(fileURLWithPath: root, isDirectory: true)
    .resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true
}

private func setBackupValue(_ root: String, _ excluded: Bool) throws {
  var value = URLResourceValues()
  value.isExcludedFromBackup = excluded
  var directory = URL(fileURLWithPath: root, isDirectory: true)
  try directory.setResourceValues(value)
}

private func rejectsProtection(_ work: () throws -> Void, _ message: String) throws {
  do {
    try work()
  } catch {
    return
  }
  throw HostFailure.check(message)
}

@MainActor private func nativeBackupCases(_ root: String) -> Int32 {
  let key = Array(repeating: UInt8(0x5a), count: 32)
  let name = "phase-four-security.sqlite"
  var failed: Int32 = 0
  let cases: [(String, () throws -> Void)] = [
    (
      "S08 default real Foundation hint",
      {
        try setBackupValue(root, false)
        let store = try NativeStoreProtection(root: root, name: name, key: key)
        try store.apply()
        try require(try backupValue(root), "default must set and read back exclusion")
        try require(
          !FileManager.default.fileExists(atPath: store.path), "backup prep creates no database")
      }
    ),
    (
      "S08 explicit backup opt-in",
      {
        try setBackupValue(root, true)
        let store = try NativeStoreProtection(root: root, name: name, key: key, excluded: false)
        try store.apply()
        try require(try !backupValue(root), "explicit opt-in must read false")
      }
    ),
    (
      "S08 input rejection before filesystem changes",
      {
        try setBackupValue(root, false)
        let before = try FileManager.default.contentsOfDirectory(atPath: root).sorted()
        for count in [0, 31, 33] {
          try rejectsProtection(
            {
              _ = try NativeStoreProtection(
                root: root, name: name, key: Array(repeating: 1, count: count))
            }, "invalid key accepted")
        }
        for bad in [
          "", "../phase-four-escape.sqlite", "phase-four-dir/child.sqlite", "other.sqlite",
        ] {
          try rejectsProtection(
            {
              _ = try Owner(protectedKey: key, root: root, name: bad, host: CommandHost())
            }, "invalid store name accepted")
        }
        try require(try !backupValue(root), "rejection changed backup hint")
        try require(
          try FileManager.default.contentsOfDirectory(atPath: root).sorted() == before,
          "rejection changed root inventory")
        let linked = root + "/phase-four-security-link.sqlite"
        let target = root + "/phase-four-security-target.sqlite"
        try Data("unrelated synthetic canary".utf8).write(to: URL(fileURLWithPath: target))
        defer {
          try? FileManager.default.removeItem(atPath: linked)
          try? FileManager.default.removeItem(atPath: target)
        }
        try FileManager.default.createSymbolicLink(atPath: linked, withDestinationPath: target)
        try rejectsProtection(
          {
            _ = try NativeStoreProtection(
              root: root, name: "phase-four-security-link.sqlite", key: key)
          }, "symlink file accepted")
        try rejectsProtection(
          {
            _ = try NativeStoreProtection(root: target, name: name, key: key)
          }, "nondirectory root accepted")
        try require(
          try Data(contentsOf: URL(fileURLWithPath: target))
            == Data("unrelated synthetic canary".utf8),
          "validation touched unrelated target")
      }
    ),
    (
      "S08 protected constructor binds the pre-established root",
      {
        let temporary = URL(fileURLWithPath: root).deletingLastPathComponent()
        let foreign = temporary.appendingPathComponent(
          "hoya-phase-four-" + String(getpid()) + "-"
            + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased(),
          isDirectory: true)
        try FileManager.default.createDirectory(at: foreign, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: foreign) }
        try setBackupValue(foreign.path, false)
        let before = try FileManager.default.contentsOfDirectory(atPath: foreign.path)
        do {
          _ = try Owner(protectedKey: key, root: foreign.path, name: name, host: CommandHost())
          throw HostFailure.check("protected constructor accepted a foreign task-owned root")
        } catch HostFailure.rpc("InvalidRequest") {}
        try require(
          try !backupValue(foreign.path),
          "unauthorized root backup attributes changed before RPC rejection")
        try require(
          try FileManager.default.contentsOfDirectory(atPath: foreign.path) == before,
          "unauthorized root inventory changed")
      }
    ),
    (
      "S08 platform policy failure is explicit",
      {
        try setBackupValue(root, false)
        let store = try NativeStoreProtection(root: root, name: name, key: key)
        try rejectsProtection(
          {
            try store.apply(using: { _, _ in throw HostFailure.check("synthetic setter failure") })
          }, "platform setter failure ignored")
        try require(try !backupValue(root), "failed setter changed backup hint")
        try require(
          !FileManager.default.fileExists(atPath: store.path), "policy failure created a database")
        try rejectsProtection(
          { try store.apply(using: { _, _ in }) },
          "policy readback mismatch ignored")
      }
    ),
    (
      "S08 closed replacement revalidates policy",
      {
        let store = try NativeStoreProtection(root: root, name: name, key: key)
        try setBackupValue(root, false)
        try rejectsProtection(
          { try store.afterOwnedReplacement(ownerIsOpen: true) },
          "replacement preparation while open accepted")
        try require(try !backupValue(root), "open replacement changed attributes")
        let original = URL(fileURLWithPath: store.path)
        let replacement = URL(fileURLWithPath: root + "/phase-four-security-next.sqlite")
        try Data("closed original".utf8).write(to: original)
        try Data("closed replacement".utf8).write(to: replacement)
        try FileManager.default.removeItem(at: original)
        try FileManager.default.moveItem(at: replacement, to: original)
        try store.afterOwnedReplacement(ownerIsOpen: false)
        try require(try backupValue(root), "closed replacement did not reapply/read back")
        try require(
          try Data(contentsOf: original) == Data("closed replacement".utf8),
          "policy revalidation changed replacement contents")
        try FileManager.default.removeItem(at: original)
        let link = root + "/phase-four-security.sqlite"
        let target = root + "/phase-four-security-replacement.sqlite"
        try Data("replacement canary".utf8).write(to: URL(fileURLWithPath: target))
        defer {
          try? FileManager.default.removeItem(atPath: link)
          try? FileManager.default.removeItem(atPath: target)
        }
        try FileManager.default.createSymbolicLink(atPath: link, withDestinationPath: target)
        try rejectsProtection(
          { try store.afterOwnedReplacement(ownerIsOpen: false) },
          "replacement symlink accepted")
      }
    ),
  ]
  for (name, run) in cases {
    do {
      try run()
      print("PASS " + name)
    } catch {
      failed += 1
      print("FAIL " + name + ": \(error)")
    }
  }
  print("native backup: \(6-failed) passed, \(failed) failed")
  return failed
}

// API-0010 S10: keys below are synthetic caller inputs, never generated or
// persisted by a native owner. They do not model application key lifecycle.
private let securityFixtureKey = Array(repeating: UInt8(0x5a), count: 32)

private func protectedSnapshot(_ root: String, _ name: String) throws -> [String: Data] {
  var values: [String: Data] = [:]
  for file in try FileManager.default.contentsOfDirectory(atPath: root)
  where file == name || file.hasPrefix(name + "-") {
    values[file] = try Data(contentsOf: URL(fileURLWithPath: root + "/" + file))
  }
  return values
}

@MainActor private func nativeSecureReader(_ root: String) throws {
  let owner = try Owner(
    protectedKey: securityFixtureKey, root: root,
    name: "phase-four-security-open.sqlite", host: CommandHost())
  defer { try? owner.close() }
  try owner.configureEntities(scope: "security-scope-canary")
  try require(
    try owner.fetch("orders").count == 3,
    "independent protected reader restores command entity and pulled rows")
  let progress = try dictionary(owner.rpc("sync_progress"))
  try require(
    progress["ready"] as? Bool == true && progress["pages"] as? String == "1",
    "independent protected reader restores checkpoint progress")
  let due = try pushed(owner, 1000).1
  try require(due != nil, "independent protected reader restores frozen retry deadline")
  print("PASS S10 independent protected reader")
}

@MainActor private func nativeProtectedCases(_ root: String) async -> Int32 {
  var failed: Int32 = 0
  let cases: [(String, () async throws -> Void)] = [
    (
      "S10 protected foreign open and process recovery",
      {
        let name = "phase-four-security-open.sqlite"
        let host = CommandHost()
        let openStart = DispatchTime.now().uptimeNanoseconds
        let owner = try Owner(protectedKey: securityFixtureKey, root: root, name: name, host: host)
        let openElapsed = DispatchTime.now().uptimeNanoseconds - openStart
        print("S10 protected open sample ns=" + String(openElapsed))
        defer { try? owner.close() }
        let status = try dictionary(owner.rpc("protection"))
        try require(
          status["encrypted"] as? Bool == true
            && text(status["cipher"] as Any).contains("4.19.0")
            && status["backup_exclusion"] as? String == "host-managed",
          "protected capability reports actual cipher and host-managed backup")
        try require(try backupValue(root), "protected owner verifies default backup hint")
        try owner.configureEntities(scope: "security-scope-canary")
        let page = try entityPageFrame(
          "security-checkpoint-canary",
          change: entityChangeFrame(
            "server:security-pulled", values: orderFields("security-pull-canary"),
            time: 20, op: "create"))
        try await owner.pull { _ in page }
        _ = try await createOrder(host, owner, "security-command-canary")
        _ = try entityInsert(owner, label: "security-entity-canary")
        let first = try pushed(owner, 1000)
        try require(
          first.0.count == 2 && first.1 != nil, "protected frozen command and entity retry")
        try owner.close()
        let bytes = try protectedSnapshot(root, name)
        let hex = securityFixtureKey.map { String(format: "%02x", $0) }.joined()
        for data in bytes.values {
          for canary in [
            "security-command-canary", "security-entity-canary", "security-pull-canary",
            "security-checkpoint-canary", "security-scope-canary", hex, "SQLite format 3",
          ] {
            try require(
              data.range(of: Data(canary.utf8)) == nil,
              "protected main or sidecar revealed a canary/header/key")
          }
        }
        let reopened = try Owner(
          protectedKey: securityFixtureKey, root: root, name: name, host: host)
        try require(
          try pushed(reopened, first.1!).0 == first.0,
          "protected reopen preserves exact frozen bytes")
        try reopened.close()
        let child = Process()
        child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        child.arguments = ["security-reader"]
        child.environment = ProcessInfo.processInfo.environment
        try child.run()
        child.waitUntilExit()
        try require(
          child.terminationReason == .exit && child.terminationStatus == 0,
          "independent protected reader succeeds")
      }
    ),
    (
      "S10 invalid wrong and legacy keys never downgrade",
      {
        let name = "phase-four-security-refusal.sqlite"
        let owner = try Owner(
          protectedKey: securityFixtureKey, root: root, name: name, host: CommandHost())
        _ = try await createOrder(owner.host, owner, "security-wrong-key-canary")
        try owner.close()
        let before = try protectedSnapshot(root, name)
        do {
          _ = try Owner(
            protectedKey: Array(repeating: 0x6b, count: 32), root: root, name: name,
            host: CommandHost())
          throw HostFailure.check("wrong key accepted")
        } catch HostFailure.rpc("BackendFailure") {}
        try require(
          try protectedSnapshot(root, name) == before, "wrong key mutated protected files")
        let args: [String: Any] = [
          "path": root + "/" + name, "namespace": "synthetic-security",
          "version": "1", "authored": false,
          "options": [
            "capacity": "1000", "history": "100", "initial_delay_ms": "1000",
            "multiplier": "2", "max_delay_ms": "60000",
          ],
        ]
        do {
          _ = try RPC.call("open_secure", args)
          throw HostFailure.check("missing key accepted")
        } catch HostFailure.rpc("InvalidRequest") {}
        try require(
          try protectedSnapshot(root, name) == before, "missing key mutated protected files")
        let correct = try Owner(
          protectedKey: securityFixtureKey, root: root, name: name, host: CommandHost())
        try require(
          try correct.fetch("orders").count == 1, "correct key still reads after rejected opens")
        try correct.close()
        let legacyName = "phase-four-security-legacy.sqlite"
        let legacy = try Owner(unprotected: root + "/" + legacyName, host: CommandHost())
        _ = try await createOrder(legacy.host, legacy, "explicit legacy canary")
        try require(
          try dictionary(legacy.rpc("protection"))["encrypted"] as? Bool == false,
          "legacy owner never claims encryption")
        try legacy.close()
        let legacyBefore = try protectedSnapshot(root, legacyName)
        do {
          _ = try Owner(
            protectedKey: securityFixtureKey, root: root, name: legacyName, host: CommandHost())
          throw HostFailure.check("plaintext file converted or accepted")
        } catch HostFailure.rpc("BackendFailure") {}
        try require(
          try protectedSnapshot(root, legacyName) == legacyBefore,
          "legacy refusal mutated plaintext")
      }
    ),
    (
      "S08 protected owner closed replacement and explicit opt-in",
      {
        let name = "phase-four-security-replacement.sqlite"
        let owner = try Owner(
          protectedKey: securityFixtureKey, root: root, name: name, host: CommandHost())
        _ = try await createOrder(owner.host, owner, "security-replacement-canary")
        do {
          try owner.revalidateAfterOwnedReplacement()
          throw HostFailure.check("open protected owner allowed replacement preparation")
        } catch HostFailure.rpc("Busy") {}
        try owner.close()
        let file = URL(fileURLWithPath: root + "/" + name)
        let retained = try Data(contentsOf: file)
        try retained.write(to: file, options: .atomic)
        try setBackupValue(root, false)
        try owner.revalidateAfterOwnedReplacement()
        try require(try backupValue(root), "closed protected replacement reapplies backup policy")
        let reopened = try Owner(
          protectedKey: securityFixtureKey, root: root, name: name,
          host: CommandHost(), excludedFromBackup: false)
        defer { try? reopened.close() }
        try require(try !backupValue(root), "protected owner explicit opt-in verifies false")
        try require(
          try reopened.fetch("orders").count == 1, "closed replacement preserves encrypted rows")
      }
    ),
  ]
  for (name, run) in cases {
    do {
      try await run()
      print("PASS " + name)
    } catch {
      failed += 1
      print("FAIL " + name + ": \(error)")
    }
  }
  print("native protected: \(3-failed) passed, \(failed) failed")
  return failed
}

@main struct Commands {
  @MainActor static func main() async {
    // Keep bounded long-running host evidence observable before process exit.
    setbuf(stdout, nil)
    guard let root = ProcessInfo.processInfo.environment["HOYADB_PHASE4_ROOT"] else {
      // The parent owns one fresh canonical root and supplies it at child
      // process startup. No in-process environment mutation or fixture
      // root setter can retarget an initialized native registry.
      let base: String
      do { base = try physicalPath(FileManager.default.temporaryDirectory.path) } catch {
        print("FAIL native harness canonical temp directory")
        exit(1)
      }
      let name =
        "hoya-phase-four-" + String(getpid()) + "-"
        + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
      let root = base + "/" + name
      do {
        try FileManager.default.createDirectory(atPath: root, withIntermediateDirectories: false)
        let child = Process()
        child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        child.arguments = Array(CommandLine.arguments.dropFirst())
        child.environment = ProcessInfo.processInfo.environment.merging([
          "HOYADB_PHASE4_ROOT": root,
          "HOYADB_PHASE6_PROTECTED": CommandLine.arguments.contains("phase-six-protected")
            ? "1" : "0",
        ]) { _, value in value }
        try child.run()
        child.waitUntilExit()
        if child.terminationReason == .exit && child.terminationStatus == 0 {
          try FileManager.default.removeItem(atPath: root)
          exit(0)
        }
        print("FAIL owned diagnostics retained at " + root)
        exit(1)
      } catch {
        print("FAIL native harness: \(error)")
        exit(1)
      }
    }
    if CommandLine.arguments.count == 2 {
      let mode = CommandLine.arguments[1]
      do {
        if mode == "phase-six-security" {
          exit(await nativeProtectedCases(root) == 0 ? 0 : 1)
        } else if mode == "security-reader" {
          try nativeSecureReader(root)
          exit(0)
        } else if mode == "phase-six-backup" {
          exit(nativeBackupCases(root) == 0 ? 0 : 1)
        } else if mode == "phase-six-legacy" {
          try await h01(root)
          print("PASS H01 explicit unprotected constructor")
          try await h02(root)
          print("PASS H02 explicit unprotected constructor")
          try await h03(root)
          print("PASS H03 explicit unprotected constructor")
          try await h04(root)
          print("PASS H04 explicit unprotected constructor")
          exit(0)
        } else if mode == "case-entity-writes" {
          try await eN01(root)
          print("PASS E-N01")
          exit(0)
        } else if mode == "case-entity-lifetime" {
          try await eN03(root)
          print("PASS E-N03")
          exit(0)
        } else if mode == "phase-five" {
          let failures = await entityCases(root)
          exit(failures == 0 ? 0 : 1)
        } else if mode == "entity-writer" {
          try await entityRestartWriter(root)
        } else if mode == "entity-reader" {
          try await entityRestartReader(root)
          exit(0)
        } else if mode == "case-h03-bound" {
          try await h03Bound(root)
          print("PASS H03 bound")
          exit(0)
        } else if mode == "case-h04-pages" {
          try await h04Pages(root)
          print("PASS H04 pages")
          exit(0)
        } else if mode == "case-h06-pages" {
          try await h06Pages(root)
          print("PASS H06 pages")
          exit(0)
        } else if mode == "case-h06-plan" {
          try await h06Plan(root)
          print("PASS H06 plan")
          exit(0)
        } else if mode.hasPrefix("writer-") {
          try await restartWriter(root, after: mode.hasSuffix("after"))
        } else if mode.hasPrefix("reader-") {
          try await restartReader(root, after: mode.hasSuffix("after"))
          print("PASS H05 " + mode)
          exit(0)
        } else if mode == "phase-six-protected" {
          guard ProcessInfo.processInfo.environment["HOYADB_PHASE6_PROTECTED"] == "1" else {
            throw HostFailure.rpc("InvalidRequest")
          }
          // Execute the unchanged complete H01–H06 and E-N01–E-N04 suites below.
        } else {
          throw HostFailure.check("unknown child mode")
        }
      } catch {
        print("FAIL " + mode + ": \(error)")
        exit(1)
      }
    }
    var failures = 0
    do {
      try await h01(root)
      print("PASS H01")
    } catch {
      failures += 1
      print("FAIL H01: \(error)")
    }
    do {
      try await h02(root)
      print("PASS H02")
    } catch {
      failures += 1
      print("FAIL H02: \(error)")
    }
    do {
      try await h03(root)
      try await h03Bound(root)
      print("PASS H03")
    } catch {
      failures += 1
      print("FAIL H03: \(error)")
    }
    do {
      try await h04(root)
      try await h04Pages(root)
      print("PASS H04")
    } catch {
      failures += 1
      print("FAIL H04: \(error)")
    }
    do {
      try await h05(root)
      print("PASS H05")
    } catch {
      failures += 1
      print("FAIL H05: \(error)")
    }
    do {
      try await h06(root)
      print("PASS H06")
    } catch {
      failures += 1
      print("FAIL H06: \(error)")
    }
    print("native commands: \(6-failures) passed, \(failures) failed")
    failures += await entityCases(root)
    exit(failures == 0 ? 0 : 1)
  }
}
