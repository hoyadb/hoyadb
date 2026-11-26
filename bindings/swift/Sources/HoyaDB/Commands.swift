// API-0011 §2/3: scoped async handlers; every operation delegates to Rust.
import Foundation

public indirect enum JSONValue: Equatable, Sendable {
  case null
  case boolean(Bool)
  case string(String)
  case integer(Int64)
  case float(Double)
  case array([JSONValue])
  case object([String: JSONValue])
}

public struct CapturedContext: Equatable, Sendable {
  public let identity: JSONValue
  public let nowMilliseconds: Int64

  public init(identity: JSONValue, nowMilliseconds: Int64) {
    self.identity = identity
    self.nowMilliseconds = nowMilliseconds
  }
}

public struct CommandDefinition: Equatable, Sendable {
  public struct Argument: Equatable, Sendable {
    public enum Kind: String, Sendable {
      case any = "Any"
      case string = "String"
      case integer = "Integer"
      case float = "Float"
      case boolean = "Boolean"
      case array = "Array"
      case object = "Object"
    }
    public let kind: Kind
    public let optional: Bool

    public init(_ kind: Kind, optional: Bool = false) {
      self.kind = kind
      self.optional = optional
    }
  }

  public enum PathPart: Equatable, Sendable {
    case key(String)
    case index(UInt32)
  }
  public let name: String
  public let arguments: [String: Argument]
  public let referencePaths: [[PathPart]]

  public init(
    _ name: String, arguments: [String: Argument] = [:], referencePaths: [[PathPart]] = []
  ) {
    self.name = name
    self.arguments = arguments
    self.referencePaths = referencePaths
  }
}

public struct CommandResult: Equatable, Sendable {
  public let key: String
  public let value: JSONValue
}

extension JSONValue {
  var wire: Wire {
    get throws {
      var nodes = 0
      return try encoded(depth: 0, nodes: &nodes)
    }
  }

  private func encoded(depth: Int, nodes: inout Int) throws -> Wire {
    nodes += 1
    guard depth <= 64, nodes <= 100_000 else { throw HoyaError.core("LimitExceeded") }
    switch self {
    case .null: return try Cell.null.wire
    case .boolean(let value): return try Cell.boolean(value).wire
    case .string(let value): return try Cell.string(value).wire
    case .integer(let value): return try Cell.integer(value).wire
    case .float(let value): return try Cell.float(value).wire
    case .array(let values):
      return .object([
        "kind": .string("array"),
        "value": .array(try values.map { try $0.encoded(depth: depth + 1, nodes: &nodes) }),
      ])
    case .object(let fields):
      return .object([
        "kind": .string("object"),
        "value": .object(try fields.mapValues { try $0.encoded(depth: depth + 1, nodes: &nodes) }),
      ])
    }
  }

  init(wire: Wire) throws {
    guard let object = wire.object, let kind = object["kind"]?.string else {
      throw HoyaError.invalidResponse
    }
    if kind == "array" {
      guard object.count == 2, case .array(let values) = object["value"] else {
        throw HoyaError.invalidResponse
      }
      self = .array(try values.map { try JSONValue(wire: $0) })
    } else if kind == "object" {
      guard object.count == 2, let fields = object["value"]?.object else {
        throw HoyaError.invalidResponse
      }
      self = .object(try fields.mapValues { try JSONValue(wire: $0) })
    } else {
      switch try Cell(wire: wire) {
      case .null: self = .null
      case .boolean(let value): self = .boolean(value)
      case .string(let value): self = .string(value)
      case .integer(let value): self = .integer(value)
      case .float(let value): self = .float(value)
      case .reference, .timestamp: throw HoyaError.invalidResponse
      }
    }
  }

  public static func decodeJSON(_ body: String) async throws -> JSONValue {
    let value = try await NativeRuntime.shared.call(
      Wire.frame("json_decode", args: ["body": .string(body)]).encoded())
    return try JSONValue(wire: value)
  }

  public func encodeJSON() async throws -> String {
    let value = try await NativeRuntime.shared.call(
      Wire.frame("json_encode", args: ["value": wire]).encoded())
    guard let result = value.string else { throw HoyaError.invalidResponse }
    return result
  }
}

extension CapturedContext {
  var wire: Wire {
    get throws {
      .object(["identity": try identity.wire, "now_ms": .string(String(nowMilliseconds))])
    }
  }

  init(wire: Wire) throws {
    guard let value = wire.object, value.count == 2, let identity = value["identity"],
      let text = value["now_ms"]?.string, let number = Int64(text), String(number) == text
    else { throw HoyaError.invalidResponse }
    self.init(identity: try JSONValue(wire: identity), nowMilliseconds: number)
  }
}

extension CommandDefinition {
  var wire: Wire {
    .object([
      "name": .string(name),
      "arguments": .object(
        arguments.mapValues {
          .object(["kind": .string($0.kind.rawValue), "optional": .boolean($0.optional)])
        }),
      "reference_paths": .array(
        referencePaths.map { parts in
          .array(
            parts.map { part in
              switch part {
              case .key(let key): return .object(["key": .string(key)])
              case .index(let index): return .object(["index": .string(String(index))])
              }
            })
        }),
    ])
  }
}

final class CommandWakeEvent: @unchecked Sendable {
  private let lock = NSLock()
  private var resolved = false
  private var continuation: CheckedContinuation<Void, Never>?

  func wait() async {
    await withCheckedContinuation { continuation in
      lock.lock()
      if resolved {
        lock.unlock()
        continuation.resume()
      } else {
        self.continuation = continuation
        lock.unlock()
      }
    }
  }

  func signal() {
    lock.lock()
    resolved = true
    let continuation = continuation
    self.continuation = nil
    lock.unlock()
    continuation?.resume()
  }
}

actor CommandWake {
  static let shared = CommandWake()
  private var waiters: [CommandWakeEvent] = []

  func prepare() -> CommandWakeEvent {
    let event = CommandWakeEvent()
    waiters.append(event)
    return event
  }

  func discard(_ event: CommandWakeEvent) {
    waiters.removeAll { $0 === event }
    event.signal()
  }

  func notify() {
    let ready = waiters
    waiters.removeAll()
    for waiter in ready { waiter.signal() }
  }
}

final class CommandControl: @unchecked Sendable {
  enum Phase { case queued, active, finishing, done }
  struct Target: Sendable {
    let ticket: String
    let generation: UInt64
    let active: Bool
  }
  private let lock = NSLock()
  private var phase = Phase.queued
  private var cancelled = false
  private var ticket: String?
  private var generation: UInt64 = 0

  func bind(_ ticket: String, generation: UInt64) {
    lock.lock()
    self.ticket = ticket
    self.generation = generation
    lock.unlock()
  }

  func cancel() {
    lock.lock()
    if phase != .finishing && phase != .done { cancelled = true }
    lock.unlock()
  }

  func checkCancellation() throws {
    lock.lock()
    defer { lock.unlock() }
    if cancelled { throw CancellationError() }
  }

  func admitted() {
    lock.lock()
    phase = .active
    lock.unlock()
  }

  func submit() throws {
    lock.lock()
    defer { lock.unlock() }
    if cancelled { throw CancellationError() }
    phase = .finishing
  }

  func finish() {
    lock.lock()
    phase = .done
    lock.unlock()
  }

  func activeTarget() throws -> Target {
    lock.lock()
    defer { lock.unlock() }
    if cancelled { throw CancellationError() }
    guard phase == .active, let ticket else { throw HoyaError.closed }
    return Target(ticket: ticket, generation: generation, active: true)
  }

  func cleanupTarget(includingFinishing: Bool = false) -> Target? {
    lock.lock()
    defer { lock.unlock() }
    guard phase == .queued || phase == .active || (includingFinishing && phase == .finishing),
      let ticket
    else { return nil }
    return Target(ticket: ticket, generation: generation, active: phase != .queued)
  }
}

public final class CommandScope: @unchecked Sendable {
  private weak var database: Database?
  private let control: CommandControl

  init(database: Database, control: CommandControl) {
    self.database = database
    self.control = control
  }

  func call(_ operation: String, _ arguments: [String: Wire]) async throws -> Wire {
    guard let database else { throw HoyaError.closed }
    return try await database.scopedCommandCall(control, operation: operation, arguments: arguments)
  }

  public func arguments() async throws -> JSONValue {
    try JSONValue(wire: await call("scope_args", [:]))
  }

  public func context() async throws -> CapturedContext {
    try CapturedContext(wire: await call("scope_context", [:]))
  }

  public func fetch(_ query: Query) async throws -> [Row] {
    let result = try await call("scope_fetch", ["query": query.wire])
    guard case .array(let rows) = result else { throw HoyaError.invalidResponse }
    return try rows.map { try Row(wire: $0) }
  }

  public func insert(_ table: String, fields: [String: Cell], proposedID: String? = nil)
    async throws -> Row
  {
    try Row(
      wire: await call(
        "scope_insert",
        [
          "table": .string(table),
          "fields": .object(try fields.mapValues { try $0.wire }),
          "proposed_id": proposedID.map(Wire.string) ?? .null,
        ]))
  }

  public func replace(_ table: String, id: String, fields: [String: Cell]) async throws -> Row {
    try Row(
      wire: await call(
        "scope_replace",
        [
          "table": .string(table), "id": .string(id),
          "fields": .object(try fields.mapValues { try $0.wire }),
        ]))
  }

  public func remove(_ table: String, id: String) async throws -> Bool {
    let result = try await call("scope_remove", ["table": .string(table), "id": .string(id)])
    guard let removed = result.boolean else { throw HoyaError.invalidResponse }
    return removed
  }

  public func batch(_ writes: [Write]) async throws -> [Change] {
    let result = try await call("scope_batch", ["writes": .array(try writes.map { try $0.wire })])
    guard case .array(let changes) = result else { throw HoyaError.invalidResponse }
    return try changes.map { try Change(wire: $0) }
  }
}

extension Database {
  public func command(
    _ definition: CommandDefinition, value: JSONValue, context: CapturedContext,
    handler: @escaping @Sendable (CommandScope) async throws -> JSONValue
  ) async throws -> CommandResult {
    let control = CommandControl()
    return try await withTaskCancellationHandler {
      try await runCommand(
        control, definition: definition, value: value, context: context, handler: handler)
    } onCancel: {
      control.cancel()
      Task { [weak self] in
        await self?.cancelManagedCommand(control)
        await CommandWake.shared.notify()
      }
    }
  }

  private func runCommand(
    _ control: CommandControl, definition: CommandDefinition, value: JSONValue,
    context: CapturedContext,
    handler: @escaping @Sendable (CommandScope) async throws -> JSONValue
  ) async throws -> CommandResult {
    try control.checkCancellation()
    let generation = epoch
    let requestValue = try await request("request")
    guard let ticket = requestValue.string else { throw HoyaError.invalidResponse }
    control.bind(ticket, generation: generation)
    managedCommands[ticket] = control
    defer {
      managedCommands.removeValue(forKey: ticket)
      control.finish()
    }
    do {
      while true {
        try control.checkCancellation()
        // Register before trying begin: a release between false and await must
        // resolve this owned event instead of losing a global FIFO wakeup.
        let wake = await CommandWake.shared.prepare()
        do {
          let admitted = try await request(
            "begin",
            arguments: [
              "request": .string(ticket),
              "definition": definition.wire, "value": value.wire, "context": context.wire,
            ])
          guard let ready = admitted.boolean else { throw HoyaError.invalidResponse }
          if ready {
            await CommandWake.shared.discard(wake)
            control.admitted()
            break
          }
        } catch {
          await CommandWake.shared.discard(wake)
          throw error
        }
        await wake.wait()
      }
      try control.checkCancellation()
      let result = try await handler(CommandScope(database: self, control: control))
      let finished = try await submitCommandFinish(
        control, arguments: ["request": .string(ticket), "result": result.wire])
      guard let object = finished.object, object.count == 2, let key = object["key"]?.string,
        let result = object["result"]
      else { throw HoyaError.invalidResponse }
      control.finish()
      await CommandWake.shared.notify()
      // Cancellation after submitted finish does not discard the durable result.
      return CommandResult(key: key, value: try JSONValue(wire: result))
    } catch {
      await cancelManagedCommand(control, includingFinishing: true)
      await CommandWake.shared.notify()
      throw error
    }
  }

  func scopedCommandCall(_ control: CommandControl, operation: String, arguments: [String: Wire])
    async throws -> Wire
  {
    let target = try control.activeTarget()
    guard target.generation == epoch else { throw HoyaError.closed }
    var fields = arguments
    fields["request"] = .string(target.ticket)
    if operation == "scope_insert" || operation == "scope_link", fields["proposed_id"] == .null {
      fields["proposed_id"] = try proposedIDValue(nil)
    }
    if operation == "scope_batch", case .array(let writes) = fields["writes"] {
      fields["writes"] = .array(
        try writes.map { write in
          guard var object = write.object, object["op"] == .string("insert"),
            object["proposed_id"] == .null
          else { return write }
          object["proposed_id"] = try proposedIDValue(nil)
          return .object(object)
        })
    }
    let result = try await request(operation, arguments: fields)
    _ = try control.activeTarget()
    return result
  }

  func cancelManagedCommand(_ control: CommandControl, includingFinishing: Bool = false) async {
    guard let target = control.cleanupTarget(includingFinishing: includingFinishing),
      target.generation == epoch
    else { return }
    _ = try? await request(
      target.active ? "abort" : "cancel_request", arguments: ["request": .string(target.ticket)])
  }

  func invalidateCommands() {
    for control in managedCommands.values { control.cancel() }
    managedCommands.removeAll()
  }
}
