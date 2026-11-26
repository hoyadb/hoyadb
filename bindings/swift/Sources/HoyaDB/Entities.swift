// API-0011 §3/6: owned entity/pull values; Rust owns resolution and clocks.
import Foundation

public struct EntityChange: Equatable, Sendable {
  public enum Operation: String, Sendable { case create, update, delete }
  public let table: String
  public let recordID: String
  public let operation: Operation
  public let fields: [String: Cell]
  public let removedFields: [String]
  public let timestamp: Int64
  public let fieldTimestamps: [String: Int64]

  public init(
    table: String, recordID: String, operation: Operation, fields: [String: Cell] = [:],
    removedFields: [String] = [], timestamp: Int64, fieldTimestamps: [String: Int64] = [:]
  ) {
    self.table = table
    self.recordID = recordID
    self.operation = operation
    self.fields = fields
    self.removedFields = removedFields
    self.timestamp = timestamp
    self.fieldTimestamps = fieldTimestamps
  }
}

public struct PullPage: Equatable, Sendable {
  public let checkpoint: String
  public let complete: Bool
  public let changes: [EntityChange]

  public init(checkpoint: String, complete: Bool, changes: [EntityChange]) {
    self.checkpoint = checkpoint
    self.complete = complete
    self.changes = changes
  }
}

public struct SyncProgress: Equatable, Sendable {
  public enum Phase: String, Sendable {
    case notStarted = "not_started"
    case bootstrapping, ready
  }
  public let phase: Phase
  public let ready: Bool
  public let pages: Int64
  public let changes: Int64
  public let tombstones: UInt64
}

public struct FieldStamp: Equatable, Sendable {
  public let timestamp: Int64
  public let authoritative: Bool
}

public struct RowMetadata: Equatable, Sendable {
  public let fields: [String: FieldStamp]
  public let tombstone: FieldStamp?
}

public struct EntityResolution: Equatable, Sendable {
  public let row: Row?
  public let metadata: RowMetadata
}

public struct MergeInput: Equatable, Sendable {
  public let schema: Schema
  public let current: Row?
  public let metadata: RowMetadata
  public let incoming: EntityChange
  public let defaultResolution: EntityResolution
}

public typealias MergeHook = @Sendable (MergeInput) async throws -> Row?
public typealias EntityContextProvider = @Sendable () throws -> CapturedContext

extension EntityChange {
  var wire: Wire {
    get throws {
      .object([
        "table": .string(table), "record_id": .string(recordID), "op": .string(operation.rawValue),
        "changes": .object(try fields.mapValues { try $0.wire }),
        "removed_fields": .array(removedFields.map(Wire.string)),
        "timestamp_ms": .string(String(timestamp)),
        "field_timestamps": .object(fieldTimestamps.mapValues { .string(String($0)) }),
      ])
    }
  }

  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 7,
      let table = object["table"]?.string, let id = object["record_id"]?.string,
      let op = object["op"]?.string, let operation = Operation(rawValue: op),
      let fields = object["changes"]?.object, case .array(let removed) = object["removed_fields"],
      let time = object["timestamp_ms"], let stamps = object["field_timestamps"]?.object
    else { throw HoyaError.invalidResponse }
    self.init(
      table: table, recordID: id, operation: operation,
      fields: try fields.mapValues { try Cell(wire: $0) },
      removedFields: try removed.map {
        guard let value = $0.string else { throw HoyaError.invalidResponse }
        return value
      },
      timestamp: try time.signedInteger(),
      fieldTimestamps: try stamps.mapValues { try $0.signedInteger() })
  }
}

extension PullPage {
  var wire: Wire {
    get throws {
      .object([
        "checkpoint": .string(checkpoint), "complete": .boolean(complete),
        "changes": .array(try changes.map { try $0.wire }),
      ])
    }
  }

  public static func decodeJSON(_ body: String) throws -> PullPage {
    let value = try Wire.decode(Data(body.utf8))
    guard let object = value.object, object.count == 3,
      let checkpoint = object["checkpoint"]?.string,
      let complete = object["complete"]?.boolean, case .array(let changes) = object["changes"]
    else { throw HoyaError.invalidResponse }
    return PullPage(
      checkpoint: checkpoint, complete: complete,
      changes: try changes.map { try EntityChange(wire: $0) })
  }

  public func encodeJSON() throws -> String { String(decoding: try wire.encoded(), as: UTF8.self) }
}

extension ProtocolValue {
  func signedInteger() throws -> Int64 {
    guard let text = string, let number = Int64(text), String(number) == text else {
      throw HoyaError.invalidResponse
    }
    return number
  }

  func unsignedInteger() throws -> UInt64 {
    guard let text = string, let number = UInt64(text), String(number) == text else {
      throw HoyaError.invalidResponse
    }
    return number
  }
}

extension SyncProgress {
  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 5, let phase = object["phase"]?.string,
      let phase = Phase(rawValue: phase), let ready = object["ready"]?.boolean,
      let pages = object["pages"], let changes = object["changes"],
      let tombstones = object["tombstones"]
    else { throw HoyaError.invalidResponse }
    self.phase = phase
    self.ready = ready
    self.pages = try pages.signedInteger()
    self.changes = try changes.signedInteger()
    self.tombstones = try tombstones.unsignedInteger()
  }
}

extension FieldStamp {
  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 2, let time = object["timestamp_ms"],
      let authoritative = object["authoritative"]?.boolean
    else { throw HoyaError.invalidResponse }
    timestamp = try time.signedInteger()
    self.authoritative = authoritative
  }
}

extension RowMetadata {
  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 2, let fields = object["fields"]?.object,
      let tombstone = object["tombstone"]
    else { throw HoyaError.invalidResponse }
    self.fields = try fields.mapValues { try FieldStamp(wire: $0) }
    self.tombstone = tombstone == .null ? nil : try FieldStamp(wire: tombstone)
  }
}

extension EntityResolution {
  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 2, let row = object["row"],
      let metadata = object["metadata"]
    else { throw HoyaError.invalidResponse }
    self.row = row == .null ? nil : try Row(wire: row)
    self.metadata = try RowMetadata(wire: metadata)
  }
}

final class PullControl: @unchecked Sendable {
  private let lock = NSLock()
  private var ticket: String?
  private var generation: UInt64 = 0
  private var cancelled = false

  func bind(_ ticket: String?, generation: UInt64) {
    lock.lock()
    self.ticket = ticket
    self.generation = generation
    lock.unlock()
  }

  func cancel() {
    lock.lock()
    cancelled = true
    lock.unlock()
  }

  func target() -> (String, UInt64)? {
    lock.lock()
    defer { lock.unlock() }
    return ticket.map { ($0, generation) }
  }

  func check() throws {
    lock.lock()
    defer { lock.unlock() }
    if cancelled { throw CancellationError() }
  }
}

extension Database {
  public func configureEntities(
    scope: String, tables: [String], context: @escaping EntityContextProvider
  ) async throws {
    _ = try await execute(
      "entity_configure",
      arguments: [
        "config": .object(["scope": .string(scope), "tables": .array(tables.map(Wire.string))])
      ])
    entityContext = context
  }

  public func reconfigureEntities(
    scope: String, tables: [String], context: @escaping EntityContextProvider
  ) async throws {
    _ = try await execute(
      "entity_reconfigure",
      arguments: [
        "config": .object(["scope": .string(scope), "tables": .array(tables.map(Wire.string))])
      ])
    entityContext = context
  }

  func entityMutation(_ operation: String, arguments: [String: Wire]) async throws -> Wire {
    guard let entityContext else { throw HoyaError.core("InvalidCommand") }
    var fields = arguments
    fields["context"] = try entityContext().wire
    return try await execute(operation, arguments: fields)
  }

  public func entityBatch(_ writes: [Write]) async throws -> [Change] {
    let value = try await entityMutation(
      "entity_batch", arguments: ["writes": .array(try writes.map { try $0.wire })])
    guard case .array(let changes) = value else { throw HoyaError.invalidResponse }
    return try changes.map { try Change(wire: $0) }
  }

  public func patch(_ table: String, id: String, fields: [String: Cell]) async throws -> Row {
    try Row(
      wire: await entityMutation(
        "patch",
        arguments: [
          "table": .string(table), "id": .string(id),
          "fields": .object(try fields.mapValues { try $0.wire }),
        ]))
  }

  public func syncProgress() async throws -> SyncProgress {
    try SyncProgress(wire: await execute("sync_progress"))
  }

  public func setMergeHooks(_ hooks: [String: MergeHook]) { mergeHooks = hooks }

  // The exact operation and its owned proposal survive Busy. The core's
  // coordinator decides readiness; this wake is not a second admission queue.
  func resumeOwned(_ operation: String, arguments: [String: Wire], generation: UInt64) async throws
    -> Wire
  {
    while true {
      try Task.checkCancellation()
      guard epoch == generation else { throw HoyaError.closed }
      let wake = await CommandWake.shared.prepare()
      do {
        let result = try await execute(operation, arguments: arguments)
        await CommandWake.shared.discard(wake)
        return result
      } catch HoyaError.busy {
        await withTaskCancellationHandler {
          await wake.wait()
        } onCancel: {
          wake.signal()
        }
        await CommandWake.shared.discard(wake)
      } catch {
        await CommandWake.shared.discard(wake)
        throw error
      }
    }
  }

  public func pullOnce(
    transport: @escaping @Sendable (_ since: String?, _ scope: String) async throws -> PullPage
  ) async throws -> SyncProgress {
    let control = PullControl()
    return try await withTaskCancellationHandler {
      try await runPull(control, transport: transport)
    } onCancel: {
      control.cancel()
      Task { [weak self] in
        await self?.cancelPull(control)
        await CommandWake.shared.notify()
      }
    }
  }

  private func cancelPull(_ control: PullControl) async {
    guard let (ticket, generation) = control.target(), epoch == generation else { return }
    _ = try? await request("abort_pull", arguments: ["request": .string(ticket)])
  }

  private func runPull(
    _ control: PullControl,
    transport: @escaping @Sendable (_ since: String?, _ scope: String) async throws -> PullPage
  ) async throws -> SyncProgress {
    let generation = epoch
    defer { control.bind(nil, generation: generation) }
    let hooks = mergeHooks
    let definition = hooks.isEmpty ? nil : try await schema()
    while epoch == generation {
      try Task.checkCancellation()
      // Busy waits through the release mechanism, like every other owned pull stage.
      let batch = try await resumeOwned("begin_pull", arguments: [:], generation: generation)
      guard let object = batch.object, let ticket = object["request"]?.string,
        let checkpoint = object["since"], let scope = object["scope"]?.string,
        checkpoint == .null || checkpoint.string != nil
      else { throw HoyaError.invalidResponse }
      control.bind(ticket, generation: generation)
      do {
        try control.check()
        let page = try await transport(checkpoint.string, scope)
        try Task.checkCancellation()
        guard epoch == generation else { throw HoyaError.closed }
        if let definition {
          var step = try await resumeOwned(
            "merge_prepare",
            arguments: [
              "request": .string(ticket), "page": page.wire,
              "tables": .array(hooks.keys.sorted().map(Wire.string)),
            ], generation: generation)
          while step.object?["kind"] == .string("offer") {
            guard let offer = step.object, let session = offer["session"],
              let index = offer["step"],
              let table = offer["table"]?.string, let hook = hooks[table]
            else { throw HoyaError.invalidResponse }
            var parts: [String: Wire] = [:]
            for name in ["current", "metadata", "incoming", "default"] {
              parts[name] = try await request(
                "merge_input",
                arguments: ["session": session, "step": index, "part": .string(name)])
            }
            guard let current = parts["current"], let metadata = parts["metadata"],
              let incoming = parts["incoming"], let defaultValue = parts["default"]
            else { throw HoyaError.invalidResponse }
            let input = try MergeInput(
              schema: definition, current: current == .null ? nil : Row(wire: current),
              metadata: RowMetadata(wire: metadata), incoming: EntityChange(wire: incoming),
              defaultResolution: EntityResolution(wire: defaultValue))
            let row = try await hook(input)
            try control.check()
            try Task.checkCancellation()
            guard epoch == generation else { throw HoyaError.closed }
            let proposal: Wire = try row?.wire ?? .null
            step = try await resumeOwned(
              "merge_reply", arguments: ["session": session, "step": index, "row": proposal],
              generation: generation)
          }
          guard let ready = step.object, ready["kind"] == .string("ready"),
            let session = ready["session"]
          else { throw HoyaError.invalidResponse }
          _ = try await resumeOwned(
            "merge_commit", arguments: ["session": session], generation: generation)
        } else {
          _ = try await resumeOwned(
            "apply_pull", arguments: ["request": .string(ticket), "page": page.wire],
            generation: generation)
        }
        return try await syncProgress()
      } catch {
        if epoch == generation {
          _ = try? await request("abort_pull", arguments: ["request": .string(ticket)])
        }
        if error as? HoyaError == .core("StalePull"), epoch == generation { continue }
        throw error
      }
    }
    throw HoyaError.closed
  }
}
