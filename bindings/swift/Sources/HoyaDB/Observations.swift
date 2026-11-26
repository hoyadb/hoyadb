// API-0011 §7 / TC-0036 G05: selected-executor managed subscription lifecycle.
import Foundation

/// Runs managed observation callbacks. It must execute submitted work serially in FIFO order
/// (the default `DispatchQueue.main` does): only then are callbacks invoked in event order and is
/// a terminal `.subscriptionTerminated` delivery the last one. A concurrent executor can reorder
/// callbacks, run a terminal delivery first and thereby suppress that listener's earlier
/// deliveries (SPEC-0036).
public typealias EventExecutor = @Sendable (@escaping @Sendable () -> Void) -> Void

public enum ObservationMode: String, Sendable {
  case membership
  case columns
  case count
}

final class ListenerFlag: @unchecked Sendable {
  private let lock = NSLock()
  private var live = true

  func cancel() {
    lock.lock()
    live = false
    lock.unlock()
  }

  func isLive() -> Bool {
    lock.lock()
    defer { lock.unlock() }
    return live
  }
}

public final class Subscription: @unchecked Sendable {
  private weak var database: Database?
  private let token: String
  private let generation: UInt64
  private let flag: ListenerFlag

  init(database: Database, token: String, generation: UInt64, flag: ListenerFlag) {
    self.database = database
    self.token = token
    self.generation = generation
    self.flag = flag
  }

  public func cancel() {
    flag.cancel()
    let database = database
    let token = token
    let generation = generation
    Task { await database?.cancelObservation(token, generation: generation) }
  }

  deinit { cancel() }
}

public protocol TimerCancellation: Sendable {
  func cancel()
}

private struct SleepTimer: TimerCancellation {
  let task: Task<Void, Never>

  func cancel() { task.cancel() }
}

public typealias MonotonicClock = @Sendable () -> UInt64
public typealias TimerScheduler =
  @Sendable (UInt64, @escaping @Sendable () -> Void) -> any TimerCancellation

enum ObservationDefaults {
  static let clock: MonotonicClock = { DispatchTime.now().uptimeNanoseconds / 1_000_000 }
  static let scheduler: TimerScheduler = { milliseconds, work in
    SleepTimer(
      task: Task {
        do {
          try await Task.sleep(nanoseconds: milliseconds * 1_000_000)
          if !Task.isCancelled { work() }
        } catch {}
      })
  }
}

struct ManagedObservations: Sendable {
  struct Listener: Sendable {
    let flag: ListenerFlag
    let count: Bool
    let unsubscribe: String
    let executor: EventExecutor
    let callback: @Sendable (Result<ProtocolValue, HoyaError>) -> Void

    func deliver(_ value: Result<ProtocolValue, HoyaError>) {
      executor {
        if flag.isLive() { callback(value) }
      }
    }

    /// SPEC-0036: the last delivery of a terminated subscription; it retires the flag.
    func deliverFinal(_ value: Result<ProtocolValue, HoyaError>) {
      executor {
        if flag.isLive() {
          flag.cancel()
          callback(value)
        }
      }
    }
  }

  var listeners: [String: Listener] = [:]
  var registering = 0
  var draining: UInt64?
  var needsDrain = false
  var drainTask: Task<Void, Never>?
  var timer: (any TimerCancellation)?
  /// A drain sampled the clock after the outstanding deadline timer was armed, so the core may
  /// have stamped a count delivery later than that timer assumes.
  var drainedSinceArm = false
  /// Count listeners with a committed write since their last delivered value.
  var countDirty: Set<String> = []
  /// A fired deadline whose timer was armed before a later drain awaits that drain's outcome.
  var deadlineFollowUp = false
  var buffered: [Wire] = []

  mutating func invalidate() {
    for listener in listeners.values { listener.flag.cancel() }
    listeners.removeAll()
    drainTask?.cancel()
    drainTask = nil
    timer?.cancel()
    timer = nil
    drainedSinceArm = false
    countDirty.removeAll()
    deadlineFollowUp = false
    buffered.removeAll()
    registering = 0
    draining = nil
    needsDrain = false
  }
}

extension Database {
  public func drainNotifications() async throws {
    guard observations.registering == 0, observations.draining == nil else { throw HoyaError.busy }
    observations.drainTask?.cancel()
    observations.drainTask = nil
    // SPEC-0036: an explicit drain returns its own failure to the caller; it is not reported.
    if let failure = await drainObserved() { throw failure }
  }

  /// Managed drains report an owner-wide failure once to the background sink (SPEC-0036 D fact).
  func managedDrain(generation: UInt64? = nil) async {
    let generation = generation ?? epoch
    if let failure = await drainObserved(generation: generation) {
      reportBackground(.observation, failure.publicKind, token: nil, generation: generation)
    }
  }

  public func observe(
    _ query: ProtocolValue, mode: ObservationMode = .membership, columns: [String] = [],
    executor: @escaping EventExecutor = { work in DispatchQueue.main.async(execute: work) },
    callback: @escaping @Sendable (Result<ProtocolValue, HoyaError>) -> Void
  ) async throws -> Subscription {
    try await registerManaged(
      "observe",
      arguments: [
        "query": query, "mode": .string(mode.rawValue), "columns": .array(columns.map(Wire.string)),
      ], count: mode == .count, sync: false, executor: executor, callback: callback)
  }

  func registerManaged(
    _ operation: String, arguments: [String: Wire], count: Bool = false, sync: Bool = true,
    executor: @escaping EventExecutor,
    callback: @escaping @Sendable (Result<ProtocolValue, HoyaError>) -> Void
  ) async throws -> Subscription {
    let generation = epoch
    observations.registering += 1
    defer {
      if epoch == generation {
        observations.registering -= 1
        scheduleObservationDrain()
      }
    }
    let value = try await request(operation, arguments: arguments)
    guard let token = value.string else { throw HoyaError.invalidResponse }
    let flag = ListenerFlag()
    observations.listeners[token] = ManagedObservations.Listener(
      flag: flag, count: count, unsubscribe: sync ? "unsubscribe_sync" : "unsubscribe",
      executor: executor, callback: callback)
    return Subscription(database: self, token: token, generation: generation, flag: flag)
  }

  func cancelObservation(_ token: String, generation: UInt64) async {
    guard generation == epoch else { return }
    guard let listener = observations.listeners.removeValue(forKey: token) else { return }
    listener.flag.cancel()
    _ = try? await request(listener.unsubscribe, arguments: ["subscription": .string(token)])
    if !observations.listeners.values.contains(where: { $0.count }) {
      observations.timer?.cancel()
      observations.timer = nil
    }
  }

  func scheduleObservationDrain() {
    guard !observations.listeners.isEmpty, observations.drainTask == nil else { return }
    let generation = epoch
    observations.drainTask = Task { [weak self] in
      await Task.yield()
      guard !Task.isCancelled else { return }
      await self?.managedDrain(generation: generation)
    }
  }

  func scheduleObservationDeadline() {
    for (token, listener) in observations.listeners where listener.count {
      observations.countDirty.insert(token)
    }
    armObservationDeadline()
  }

  private func armObservationDeadline() {
    guard observations.timer == nil,
      observations.listeners.values.contains(where: { $0.count })
    else { return }
    let generation = epoch
    observations.drainedSinceArm = false
    observations.timer = timerScheduler(16) { [weak self] in
      Task { await self?.observationDeadline(generation: generation) }
    }
  }

  private func observationDeadline(generation: UInt64) async {
    guard generation == epoch else { return }
    observations.timer = nil
    // The core alone decides whether a count is due. A drain sampled after this timer was armed
    // may have stamped the last delivery later than the timer assumed, so the deadline drain can
    // come before the core's window ends. Its outcome is settled when that drain completes.
    if observations.drainedSinceArm { observations.deadlineFollowUp = true }
    await managedDrain()
  }

  /// After the drain that follows a fired deadline: a count listener still without a value for
  /// a later write gets one follow-up timer. It is the only outstanding timer, and a timer window
  /// without any drain ends the sequence, so this is neither a polling loop nor a second throttle.
  private func settleObservationDeadline() {
    guard observations.deadlineFollowUp else { return }
    observations.deadlineFollowUp = false
    guard
      observations.listeners.contains(where: {
        $0.value.count && observations.countDirty.contains($0.key)
      })
    else { return }
    armObservationDeadline()
  }

  @discardableResult
  func drainObserved(generation expected: UInt64? = nil) async -> HoyaError? {
    let generation = expected ?? epoch
    guard generation == epoch else { return nil }
    observations.drainTask = nil
    if observations.draining == generation {
      observations.needsDrain = true
      return nil
    }
    guard observations.registering == 0, !observations.listeners.isEmpty else { return nil }
    observations.draining = generation
    defer {
      if observations.draining == generation {
        observations.draining = nil
        if observations.needsDrain {
          observations.needsDrain = false
          scheduleObservationDrain()
        } else {
          settleObservationDeadline()
        }
      }
    }
    do {
      var more = true
      while more && !Task.isCancelled {
        observations.drainedSinceArm = true
        let value = try await request(
          "drain", arguments: ["now_ms": .string(String(monotonicClock()))])
        guard let result = value.object, case .array(let events) = result["events"],
          let hasMore = result["has_more"]?.boolean
        else { throw HoyaError.invalidResponse }
        observations.buffered.append(contentsOf: events)
        if observations.registering != 0 { return nil }
        let buffered = observations.buffered
        observations.buffered.removeAll()
        for event in buffered {
          guard let token = event.object?["subscription"]?.string else { continue }
          // SPEC-0036: the generic error event is terminal for every listener kind and is
          // handled before typed decoding; data error values stay recoverable.
          if event.object?["kind"] == .string("error") {
            let kind = BackgroundError.publicKind(event.object?["value"]?.object?["kind"]?.string)
            guard let listener = observations.listeners.removeValue(forKey: token) else { continue }
            reportBackground(.observation, kind, token: token, generation: generation)
            listener.deliverFinal(.failure(.subscriptionTerminated(kind)))
            if !observations.listeners.values.contains(where: { $0.count }) {
              observations.timer?.cancel()
              observations.timer = nil
            }
            continue
          }
          if let listener = observations.listeners[token] {
            if listener.count { observations.countDirty.remove(token) }
            listener.deliver(.success(event))
          }
        }
        more = hasMore
      }
    } catch {
      // SPEC-0036: an owner-wide failure is returned (explicit) or reported (managed), never
      // broadcast to listeners.
      guard generation == epoch else { return nil }
      return error as? HoyaError ?? .boundaryFailure
    }
    return nil
  }
}

public enum ObservedValue: Equatable, Sendable {
  case rows([Row])
  case count(UInt64)
}

private func translateEvent<T: Sendable>(
  _ result: Result<ProtocolValue, HoyaError>, decode: (Wire) throws -> T
) -> Result<T, HoyaError> {
  result.flatMap { event in
    do {
      guard let value = event.object?["value"] else { throw HoyaError.invalidResponse }
      return .success(try decode(value))
    } catch { return .failure(error as? HoyaError ?? .invalidResponse) }
  }
}

extension Database {
  public func observe(
    _ query: Query, mode: ObservationMode = .membership, columns: [String] = [],
    executor: @escaping EventExecutor = { work in DispatchQueue.main.async(execute: work) },
    callback: @escaping @Sendable (Result<ObservedValue, HoyaError>) -> Void
  ) async throws -> Subscription {
    try await observe(query.wire, mode: mode, columns: columns, executor: executor) { result in
      callback(
        translateEvent(result) { value in
          guard let object = value.object, let kind = object["kind"]?.string else {
            throw HoyaError.invalidResponse
          }
          switch kind {
          case "rows":
            guard object.count == 2, case .array(let rows) = object["rows"] else {
              throw HoyaError.invalidResponse
            }
            return .rows(try rows.map { try Row(wire: $0) })
          case "count":
            guard object.count == 2, let count = object["count"] else {
              throw HoyaError.invalidResponse
            }
            return .count(try count.unsignedInteger())
          case "error":
            guard let kind = object["error"]?.object?["kind"]?.string else {
              throw HoyaError.invalidResponse
            }
            throw HoyaError.core(kind)
          default: throw HoyaError.invalidResponse
          }
        })
    }
  }

  public func observeQueueStatus(
    executor: @escaping EventExecutor = { work in DispatchQueue.main.async(execute: work) },
    callback: @escaping @Sendable (Result<QueueStatus, HoyaError>) -> Void
  ) async throws -> Subscription {
    try await registerManaged("observe_status", arguments: [:], executor: executor) { result in
      callback(translateEvent(result) { try QueueStatus(wire: $0) })
    }
  }

  public func observeProgress(
    executor: @escaping EventExecutor = { work in DispatchQueue.main.async(execute: work) },
    callback: @escaping @Sendable (Result<SyncProgress, HoyaError>) -> Void
  ) async throws -> Subscription {
    try await registerManaged("observe_progress", arguments: [:], executor: executor) { result in
      callback(translateEvent(result) { try SyncProgress(wire: $0) })
    }
  }
}

public enum CommandOutcome: Equatable, Sendable {
  case queued, sent, acknowledged
  case applied(JSONValue)
  case corrected(JSONValue)
  case rejected(String)
}

extension Database {
  public func observeOutcome(
    _ key: String,
    executor: @escaping EventExecutor = { work in DispatchQueue.main.async(execute: work) },
    callback: @escaping @Sendable (Result<CommandOutcome, HoyaError>) -> Void
  ) async throws -> Subscription {
    try await registerManaged(
      "observe_outcome", arguments: ["key": .string(key)], executor: executor
    ) { result in
      callback(
        translateEvent(result) { value in
          guard let fields = value.object, let state = fields["state"]?.string else {
            throw HoyaError.invalidResponse
          }
          switch state {
          case "queued": return .queued
          case "sent": return .sent
          case "acknowledged": return .acknowledged
          case "applied", "corrected":
            guard let payload = fields["result"] else { throw HoyaError.invalidResponse }
            let decoded = try JSONValue(wire: payload)
            return state == "applied" ? .applied(decoded) : .corrected(decoded)
          case "rejected":
            guard let reason = fields["reason"]?.string else { throw HoyaError.invalidResponse }
            return .rejected(reason)
          default: throw HoyaError.invalidResponse
          }
        })
    }
  }
}
