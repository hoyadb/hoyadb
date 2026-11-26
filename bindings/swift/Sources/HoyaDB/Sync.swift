// API-0011 §3/7: opaque envelopes and explicit application reconciliation.
import Foundation

public enum RemoteOutcome: Equatable, Sendable {
  case applied(JSONValue)
  case corrected(JSONValue)
  case rejected(String)
}

public struct QueueEntry: Equatable, Sendable {
  public enum Kind: String, Sendable { case command, entity }
  public enum State: String, Sendable {
    case queued, sent, acknowledged, applied, corrected, rejected, cancelled, discarded
  }
  public let kind: Kind
  public let table: String?
  public let recordID: String?
  public let key: String
  public let state: State
  public let blockedOn: [String]
  public let awaitingLocal: Bool
  public let awaitingKey: String?
  public let rolledBack: Bool
  public let attempts: UInt32
  public let nextDueMilliseconds: Int64?
}

public struct QueueStatus: Equatable, Sendable {
  public let pending: UInt64
  public let sending: UInt64
  public let awaitingOutcome: UInt64
  public let live: UInt64
  public let blocked: Bool
  public let entries: [QueueEntry]
}

public struct PushBatch: Equatable, Sendable {
  public let envelopes: [String]
  public let nextDueMilliseconds: Int64?
  public let hasMore: Bool
}

public struct PendingReconciliation: Equatable, Sendable {
  public let key: String
  public let outcome: RemoteOutcome
}

public enum ReconcileWrite: Equatable, Sendable {
  case put(table: String, row: Row)
  case remove(table: String, id: String)
  case entity(EntityChange)
}

/// SPEC-0039: an explicit expected before-image of a row outside the command closure.
/// `expected == nil` expects the row to be absent.
public struct CorrectionGuard: Equatable, Sendable {
  public let table: String
  public let id: String
  public let expected: [String: Cell]?
  public init(table: String, id: String, expected: [String: Cell]?) {
    self.table = table
    self.id = id
    self.expected = expected
  }
}

public struct ReconcilePlan: Equatable, Sendable {
  public let writes: [ReconcileWrite]
  public let mappings: [String: String]
  public init(writes: [ReconcileWrite] = [], mappings: [String: String] = [:]) {
    self.writes = writes
    self.mappings = mappings
  }
}

extension RemoteOutcome {
  var wire: Wire {
    get throws {
      switch self {
      case .applied(let value):
        return .object(["status": .string("applied"), "result": try value.wire])
      case .corrected(let value):
        return .object(["status": .string("corrected"), "result": try value.wire])
      case .rejected(let reason):
        return .object(["status": .string("rejected"), "reason": .string(reason)])
      }
    }
  }

  init(wire: Wire) throws {
    guard let value = wire.object, value.count == 2, let state = value["status"]?.string else {
      throw HoyaError.invalidResponse
    }
    switch state {
    case "applied", "corrected":
      guard let result = value["result"] else { throw HoyaError.invalidResponse }
      let decoded = try JSONValue(wire: result)
      self = state == "applied" ? .applied(decoded) : .corrected(decoded)
    case "rejected":
      guard let reason = value["reason"]?.string else { throw HoyaError.invalidResponse }
      self = .rejected(reason)
    default: throw HoyaError.invalidResponse
    }
  }
}

extension ReconcileWrite {
  var wire: Wire {
    get throws {
      switch self {
      case .put(let table, let row):
        return .object([
          "op": .string("put"), "table": .string(table), "id": .string(row.id),
          "fields": .object(try row.fields.mapValues { try $0.wire }),
        ])
      case .remove(let table, let id):
        return .object(["op": .string("remove"), "table": .string(table), "id": .string(id)])
      case .entity(let change):
        return .object(["op": .string("entity"), "change": try change.wire])
      }
    }
  }
}

extension CorrectionGuard {
  var wire: Wire {
    get throws {
      .object([
        "table": .string(table), "id": .string(id),
        "expected": try expected.map { .object(try $0.mapValues { try $0.wire }) } ?? .null,
      ])
    }
  }
}

extension ReconcilePlan {
  var wire: Wire {
    get throws {
      .object([
        "writes": .array(try writes.map { try $0.wire }),
        "mappings": .object(mappings.mapValues(Wire.string)),
      ])
    }
  }
}

extension QueueEntry {
  init(wire: Wire) throws {
    guard let value = wire.object, value.count == 11, let kind = value["kind"]?.string,
      let kind = Kind(rawValue: kind), let table = value["table"],
      let recordID = value["record_id"],
      let key = value["key"]?.string, let state = value["state"]?.string,
      let state = State(rawValue: state),
      case .array(let blocked) = value["blocked_on"],
      let awaiting = value["awaiting_local"]?.boolean,
      let awaitingKey = value["awaiting_key"], awaitingKey == .null || awaitingKey.string != nil,
      let rolledBack = value["rolled_back"]?.boolean, let attempts = value["attempts"],
      let due = value["next_due_ms"],
      table == .null || table.string != nil, recordID == .null || recordID.string != nil
    else { throw HoyaError.invalidResponse }
    self.kind = kind
    self.table = table.string
    self.recordID = recordID.string
    self.key = key
    self.state = state
    self.blockedOn = try blocked.map {
      guard let text = $0.string else { throw HoyaError.invalidResponse }
      return text
    }
    self.awaitingLocal = awaiting
    self.awaitingKey = awaitingKey.string
    self.rolledBack = rolledBack
    guard let count = UInt32(exactly: try attempts.unsignedInteger()) else {
      throw HoyaError.invalidResponse
    }
    self.attempts = count
    self.nextDueMilliseconds = due == .null ? nil : try due.signedInteger()
  }
}

extension QueueStatus {
  init(wire: Wire) throws {
    guard let value = wire.object, value.count == 6, let pending = value["pending"],
      let sending = value["sending"],
      let awaiting = value["awaiting_outcome"], let live = value["live"],
      let blocked = value["blocked"]?.boolean,
      case .array(let entries) = value["commands"]
    else { throw HoyaError.invalidResponse }
    self.pending = try pending.unsignedInteger()
    self.sending = try sending.unsignedInteger()
    self.awaitingOutcome = try awaiting.unsignedInteger()
    self.live = try live.unsignedInteger()
    self.blocked = blocked
    self.entries = try entries.map { try QueueEntry(wire: $0) }
  }
}

extension PushBatch {
  init(wire: Wire) throws {
    guard let value = wire.object, value.count == 3,
      case .array(let envelopes) = value["envelopes"],
      let due = value["next_due_ms"], let more = value["has_more"]?.boolean
    else { throw HoyaError.invalidResponse }
    self.envelopes = try envelopes.map {
      guard let text = $0.string else { throw HoyaError.invalidResponse }
      return text
    }
    nextDueMilliseconds = due == .null ? nil : try due.signedInteger()
    hasMore = more
  }
}

/// API-0011 §4 paged reads. The unpaged call stays the fast path; only when its complete reply
/// exceeds one frame are the core-owned prefixes followed by key cursor. The core orders and
/// bounds every page; this loop only concatenates them into the unpaged shape.
func pagedRead(
  _ operation: String, list: String,
  call: @Sendable (String, [String: Wire]) async throws -> Wire
) async throws -> Wire {
  do {
    return try await call(operation, [:])
  } catch HoyaError.core("LimitExceeded") {}
  var attempt = 0
  while true {
    var items: [Wire] = []
    var after: Wire = .null
    var first: [String: Wire]?
    do {
      var more = true
      while more {
        let reply: Wire
        do {
          reply = try await call(operation + "_page", ["after": after])
        } catch HoyaError.invalidRequest where first == nil {
          // An older native library has no paged operation: keep the original failure.
          throw HoyaError.core("LimitExceeded")
        }
        guard let page = reply.object, case .array(let entries) = page[list],
          let hasMore = page["has_more"]?.boolean, !entries.isEmpty || !hasMore
        else { throw HoyaError.invalidResponse }
        if first == nil { first = page }
        items.append(contentsOf: entries)
        if let last = entries.last {
          guard let key = last.object?["key"]?.string else { throw HoyaError.invalidResponse }
          after = .string(key)
        }
        more = hasMore
      }
    } catch HoyaError.core("Protocol") where after != .null && attempt < 2 {
      // The cursor's entry settled between pages: restart from the first prefix.
      attempt += 1
      continue
    }
    if list == "entries" { return .array(items) }
    var status = first ?? [:]
    status["has_more"] = nil
    status[list] = .array(items)
    return .object(status)
  }
}

extension Database {
  public func queueStatus() async throws -> QueueStatus {
    try QueueStatus(
      wire: await pagedRead("queue_status", list: "commands") { operation, arguments in
        try await self.execute(operation, arguments: arguments)
      })
  }

  public func preparePush(nowMilliseconds: Int64, jitterBasisPoints: UInt16) async throws
    -> PushBatch
  {
    try PushBatch(
      wire: await execute(
        "prepare_push",
        arguments: [
          "now_ms": .string(String(nowMilliseconds)),
          "jitter_basis_points": .string(String(jitterBasisPoints)),
        ]))
  }

  public func receive(_ key: String, outcome: RemoteOutcome?) async throws {
    _ = try await execute(
      "receive", arguments: ["key": .string(key), "outcome": outcome?.wire ?? .null])
  }

  public func requestRedelivery(_ key: String, nowMilliseconds: Int64) async throws {
    _ = try await execute(
      "request_redelivery",
      arguments: ["key": .string(key), "now_ms": .string(String(nowMilliseconds))])
  }

  public func receiveEntity(
    table: String, recordID: String, writeID: String, outcome: RemoteOutcome?
  ) async throws {
    _ = try await execute(
      "receive_entity",
      arguments: [
        "table": .string(table), "record_id": .string(recordID), "write_id": .string(writeID),
        "outcome": outcome?.wire ?? .null,
      ])
  }

  public func receiveWire(_ body: String) async throws -> ProtocolValue {
    try await execute("receive_wire", arguments: ["body": .string(body)])
  }

  public func pendingReconciliations() async throws -> [PendingReconciliation] {
    let value = try await pagedRead("pending_reconciliations", list: "entries") {
      operation, arguments in try await self.execute(operation, arguments: arguments)
    }
    guard case .array(let entries) = value else { throw HoyaError.invalidResponse }
    return try entries.map {
      guard let fields = $0.object, fields.count == 2, let key = fields["key"]?.string,
        let outcome = fields["outcome"]
      else { throw HoyaError.invalidResponse }
      return PendingReconciliation(key: key, outcome: try RemoteOutcome(wire: outcome))
    }
  }

  public func reconcile(_ key: String, plan: ReconcilePlan) async throws {
    _ = try await reconcileResolving(key, plan: plan)
  }

  /// SPEC-0039/0040: one reporting reconcile with the caller's exact expectations; errors are
  /// returned unchanged.
  public func reconcileGuarded(_ key: String, plan: ReconcilePlan, guards: [CorrectionGuard])
    async throws
  {
    _ = try await reconcileResolving(key, plan: plan, guards: guards)
  }

  public func rollback(_ key: String) async throws {
    _ = try await rollbackResolving(key)
  }

  public func cancel(_ key: String) async throws {
    _ = try await execute("cancel", arguments: ["key": .string(key)])
  }

  public func forget(_ key: String) async throws {
    _ = try await execute("forget", arguments: ["key": .string(key)])
  }
}

public typealias PushTransport = @Sendable (_ opaqueBatchJSON: String) async throws -> String
public typealias Reconciler = @Sendable (PendingReconciliation) async throws -> ReconcilePlan
public typealias UTCClock = @Sendable () throws -> Int64
public typealias JitterProvider = @Sendable () throws -> UInt16

final class SyncDriver: @unchecked Sendable {
  private weak var database: Database?
  private let generation: UInt64
  private let transport: PushTransport
  private let reconciler: Reconciler?
  private let now: UTCClock
  private let jitter: JitterProvider
  private let scheduler: TimerScheduler
  private let onError: @Sendable (HoyaError) -> Void
  private let lock = NSLock()
  private var stopped = false
  private var task: Task<Void, Never>?
  private var waiting: CommandWakeEvent?
  // Accessed only by the single run task, never a timer/callback thread.
  private var due: Int64?

  init(
    database: Database, generation: UInt64, transport: @escaping PushTransport,
    reconcile: Reconciler?, now: @escaping UTCClock, jitter: @escaping JitterProvider,
    scheduler: @escaping TimerScheduler, onError: @escaping @Sendable (HoyaError) -> Void
  ) {
    self.database = database
    self.generation = generation
    self.transport = transport
    reconciler = reconcile
    self.now = now
    self.jitter = jitter
    self.scheduler = scheduler
    self.onError = onError
  }

  func start() {
    lock.lock()
    task = Task { await self.run() }
    lock.unlock()
  }

  func stop() {
    lock.lock()
    stopped = true
    let task = task
    let waiting = waiting
    lock.unlock()
    task?.cancel()
    waiting?.signal()
  }

  func wake() {
    lock.lock()
    let waiting = waiting
    lock.unlock()
    waiting?.signal()
  }

  private func condition() -> CommandWakeEvent {
    let event = CommandWakeEvent()
    lock.lock()
    waiting = event
    let stopped = stopped
    lock.unlock()
    if stopped { event.signal() }
    return event
  }

  private func live() -> Bool {
    lock.lock()
    defer { lock.unlock() }
    return !stopped
  }

  private func call(
    _ operation: String, _ arguments: [String: Wire] = [:],
    validate: @Sendable (Wire) throws -> Void = { _ in }
  ) async throws -> Wire {
    while live() {
      try Task.checkCancellation()
      let wake = await CommandWake.shared.prepare()
      do {
        guard
          let result = try await database?.syncRequest(
            self, generation: generation, operation: operation, arguments: arguments)
        else { throw HoyaError.closed }
        try validate(result)
        await CommandWake.shared.discard(wake)
        try Task.checkCancellation()
        guard live() else { throw CancellationError() }
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
    throw CancellationError()
  }

  private func cycle() async throws {
    // SPEC-0039: keys whose core reconcile conflicted are not retried in this pass.
    var conflicted = Set<String>()
    var more = true
    // A pass that fails before its own prepare reply has no deadline: reusing the previous
    // pass's (now past) deadline would reschedule the failing pass immediately, forever.
    due = nil
    while more && live() {
      let batch = try PushBatch(
        wire: await call(
          "prepare_push",
          [
            "now_ms": .string(String(try now())),
            "jitter_basis_points": .string(String(try jitter())),
          ]))
      due = batch.nextDueMilliseconds
      more = batch.hasMore
      if !batch.envelopes.isEmpty {
        // Core envelopes stay opaque. Only the enclosing array punctuation is
        // constructed here; no number/value reserialization or reordering.
        let response = try await transport("[" + batch.envelopes.joined(separator: ",") + "]")
        try Task.checkCancellation()
        let receipts = try await call("receive_wire", ["body": .string(response)]) { value in
          // Only an admission Busy retries the owned response; the reports are read below.
          guard case .array(let reports) = value else { throw HoyaError.invalidResponse }
          for report in reports {
            guard let fields = report.object, let ok = fields["ok"]?.boolean else {
              throw HoyaError.invalidResponse
            }
            if !ok {
              guard let kind = fields["error"]?.object?["kind"]?.string else {
                throw HoyaError.invalidResponse
              }
              if kind == "Busy" { throw HoyaError.busy }
            }
          }
        }
        // API-0011 §7: each failed receipt is one sync failure; the pass still reconciles the
        // outcomes that were received.
        if case .array(let reports) = receipts {
          for report in reports {
            if report.object?["ok"] == .boolean(false),
              let kind = report.object?["error"]?.object?["kind"]?.string
            {
              onError(.core(kind))
            }
          }
        }
      }
      if let reconciler {
        let pending = try await pagedRead("pending_reconciliations", list: "entries") {
          operation, arguments in try await self.call(operation, arguments)
        }
        guard case .array(let values) = pending else { throw HoyaError.invalidResponse }
        for value in values {
          let pending = try PendingReconciliation(wire: value)
          if conflicted.contains(pending.key) { continue }
          let plan = try await reconciler(pending)
          try Task.checkCancellation()
          do {
            _ = try await call(
              "reconcile_resolving",
              [
                "key": .string(pending.key), "plan": plan.wire, "guards": .array([]),
              ])
          } catch HoyaError.core("RollbackConflict") {
            // The entry stays pending; later entries still reconcile in this pass.
            conflicted.insert(pending.key)
            onError(.core("RollbackConflict"))
          }
        }
      }
      await database?.drainSyncNotifications(self, generation: generation)
    }
  }

  private func run() async {
    defer { stop() }
    while live() && !Task.isCancelled {
      let event = condition()
      do { try await cycle() } catch {
        guard live(), !Task.isCancelled else { return }
        onError(error as? HoyaError ?? .boundaryFailure)
      }
      guard live(), !Task.isCancelled else { return }
      var timer: (any TimerCancellation)?
      if let due {
        do {
          let current = try now()
          let (difference, overflow) = due.subtractingReportingOverflow(current)
          let delay =
            due <= current ? 0 : min(UInt64(overflow ? Int64.max : difference), 2_147_483_647)
          timer = scheduler(delay) { event.signal() }
        } catch { onError(error as? HoyaError ?? .boundaryFailure) }
      }
      await withTaskCancellationHandler {
        await event.wait()
      } onCancel: {
        event.signal()
      }
      timer?.cancel()
    }
  }
}

extension PendingReconciliation {
  init(wire: Wire) throws {
    guard let fields = wire.object, fields.count == 2, let key = fields["key"]?.string,
      let outcome = fields["outcome"]
    else { throw HoyaError.invalidResponse }
    self.key = key
    self.outcome = try RemoteOutcome(wire: outcome)
  }
}

extension Database {
  public func startSync(
    transport: @escaping PushTransport, reconcile: Reconciler? = nil,
    now: @escaping UTCClock = { Int64(Date().timeIntervalSince1970 * 1000) },
    jitter: JitterProvider? = nil,
    onError: @escaping @Sendable (HoyaError) -> Void = { _ in }
  ) throws {
    _ = try ownerGeneration()
    stopSync()
    let generation = epoch
    // SPEC-0036: the existing onError adapter is kept; each sync failure is also one S fact.
    let report: @Sendable (HoyaError) -> Void = { [weak self] error in
      onError(error)
      Task {
        await self?.reportBackground(.sync, error.publicKind, token: nil, generation: generation)
      }
    }
    let driver = SyncDriver(
      database: self, generation: generation, transport: transport, reconcile: reconcile,
      now: now, jitter: jitter ?? { try NamespaceDefaults.jitter() }, scheduler: timerScheduler,
      onError: report)
    syncDriver = driver
    driver.start()
  }

  public func stopSync() {
    syncDriver?.stop()
    syncDriver = nil
  }

  func syncRequest(
    _ driver: SyncDriver, generation: UInt64, operation: String, arguments: [String: Wire]
  ) async throws -> Wire {
    guard syncDriver === driver, epoch == generation else { throw HoyaError.closed }
    return try await execute(operation, arguments: arguments)
  }

  func drainSyncNotifications(_ driver: SyncDriver, generation: UInt64) async {
    guard syncDriver === driver, epoch == generation else { return }
    await managedDrain(generation: generation)
  }
}
