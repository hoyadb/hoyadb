// API-0011 / TC-0036 G01/G09/G16/G20: scoped native owner, no fixed schema.
import Darwin
import Foundation

public typealias IDGenerator = @Sendable () throws -> String?

private struct OpenedOwner: Sendable {
  let handle: String
  let configurationError: (any Error)?
}

public actor Database {
  private let runtime: any NativeCalling
  let idGenerator: IDGenerator?
  let options: DatabaseOptions
  let onBackgroundError: (@Sendable (BackgroundError) -> Void)?
  let conflict: ConflictPolicy?
  let onResolution: ResolutionCallback?
  let backupPolicyIO: any BackupPolicyIO
  private var owner: String?
  private var opening: Task<OpenedOwner, any Error>?
  private var retiring: Task<String?, Never>?
  private var closing: Task<Void, any Error>?
  private var closingEpoch: UInt64?
  var epoch: UInt64 = 0
  var observations = ManagedObservations()
  var mergeHooks: [String: MergeHook] = [:]
  var syncDriver: SyncDriver?
  var entityContext: EntityContextProvider?
  var managedCommands: [String: CommandControl] = [:]
  let monotonicClock: MonotonicClock
  let timerScheduler: TimerScheduler
  private var verifiedBackupExcluded: Bool?
  /// Whether the pending `opening` attempt may already have submitted its native open.
  private var openSubmitted = false
  private var memoryStore: MemoryStore?

  /// Retain the original public initializer symbol and its default arguments.
  public init(
    idGenerator: IDGenerator? = nil, options: DatabaseOptions = .init(),
    monotonicClock: MonotonicClock? = nil, timerScheduler: TimerScheduler? = nil
  ) {
    self.init(
      idGenerator: idGenerator, options: options, monotonicClock: monotonicClock,
      timerScheduler: timerScheduler, onBackgroundError: nil, conflict: nil, onResolution: nil)
  }

  /// Retain the SPEC-0036 initializer as the conflict options are added.
  public init(
    idGenerator: IDGenerator? = nil, options: DatabaseOptions = .init(),
    monotonicClock: MonotonicClock? = nil, timerScheduler: TimerScheduler? = nil,
    onBackgroundError: (@Sendable (BackgroundError) -> Void)? = nil
  ) {
    self.init(
      idGenerator: idGenerator, options: options, monotonicClock: monotonicClock,
      timerScheduler: timerScheduler, onBackgroundError: onBackgroundError,
      conflict: nil, onResolution: nil)
  }

  public init(
    idGenerator: IDGenerator? = nil, options: DatabaseOptions = .init(),
    monotonicClock: MonotonicClock? = nil, timerScheduler: TimerScheduler? = nil,
    onBackgroundError: (@Sendable (BackgroundError) -> Void)? = nil,
    conflict: ConflictPolicy? = nil, onResolution: ResolutionCallback? = nil
  ) {
    self.options = options
    self.idGenerator = idGenerator
    self.onBackgroundError = onBackgroundError
    self.conflict = conflict
    self.onResolution = onResolution
    backupPolicyIO = DarwinBackupPolicyIO()
    runtime = NativeRuntime.shared
    self.monotonicClock = monotonicClock ?? ObservationDefaults.clock
    self.timerScheduler = timerScheduler ?? ObservationDefaults.scheduler
  }

  init(
    runtime: any NativeCalling, idGenerator: IDGenerator? = nil, options: DatabaseOptions = .init(),
    monotonicClock: @escaping MonotonicClock = ObservationDefaults.clock,
    timerScheduler: @escaping TimerScheduler = ObservationDefaults.scheduler,
    onBackgroundError: (@Sendable (BackgroundError) -> Void)? = nil,
    conflict: ConflictPolicy? = nil, onResolution: ResolutionCallback? = nil,
    backupPolicyIO: any BackupPolicyIO = DarwinBackupPolicyIO()
  ) {
    self.runtime = runtime
    self.options = options
    self.idGenerator = idGenerator
    self.onBackgroundError = onBackgroundError
    self.conflict = conflict
    self.onResolution = onResolution
    self.backupPolicyIO = backupPolicyIO
    self.monotonicClock = monotonicClock
    self.timerScheduler = timerScheduler
  }

  deinit {
    syncDriver?.stop()
    for control in managedCommands.values { control.cancel() }
    observations.invalidate()
    let runtime = runtime
    let handle = owner
    let opening = opening
    let retiring = retiring
    let closing = closing
    let store = memoryStore
    if handle != nil || opening != nil || retiring != nil || closing != nil {
      Task {
        defer { withExtendedLifetime(store) {} }
        if let closing {
          do {
            try await closing.value
            return
          } catch {
            // A retained retired handle still needs one disposal attempt.
          }
        }
        let disposal: String?
        if let retiring {
          disposal = await retiring.value
        } else if let handle {
          disposal = handle
        } else {
          disposal = try? await opening?.value.handle
        }
        if let disposal {
          _ = try? await runtime.call(Wire.frame("close", owner: disposal, args: [:]).encoded())
          await CommandWake.shared.notify()
        }
      }
    }
  }

  public func openMemory(schema: Schema) async throws {
    try requireClosed()
    try await openMemory(schema: schema, namespace: NamespaceDefaults.generate())
  }

  public func openMemory(
    schema: Schema, namespace: String, store: MemoryStore? = nil, migrations: [Migration] = []
  ) async throws {
    try requireClosed()
    try await beginOpen(
      engine: .object([
        "kind": .string("memory"), "store": store.map { .string($0.handle) } ?? .null,
      ]), key: Data(),
      schema: schema, namespace: namespace, backup: nil, migrations: migrations, store: store)
  }

  public func openProtected(
    directory: URL, name: String, key: Data, schema: Schema, backupExcluded: Bool = true,
    migrations: [Migration] = []
  ) async throws {
    try requireClosed()
    try await openProtected(
      directory: directory, name: name, key: key, schema: schema,
      namespace: NamespaceDefaults.generate(), backupExcluded: backupExcluded,
      migrations: migrations)
  }

  public func openProtected(
    directory: URL, name: String, key: Data, schema: Schema, namespace: String,
    backupExcluded: Bool = true, migrations: [Migration] = []
  ) async throws {
    try requireClosed()
    // These checks precede filesystem/backup actions. The core independently
    // validates all schema, engine and key requirements again.
    guard key.count == 32, !name.isEmpty, name != ".", name != "..",
      !name.contains("/"), !name.contains("\\"), !name.contains("\0"),
      directory.isFileURL, (directory.host ?? "").isEmpty,
      directory.path.hasPrefix("/")
    else { throw HoyaError.invalidRequest }
    guard let canonical = realpath(directory.path, nil) else { throw HoyaError.invalidRequest }
    let canonicalPath = String(cString: canonical)
    free(canonical)
    guard canonicalPath == directory.path else { throw HoyaError.invalidRequest }
    do {
      let values = try directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
      guard values.isDirectory == true, values.isSymbolicLink != true else {
        throw HoyaError.invalidRequest
      }
    } catch {
      throw HoyaError.invalidRequest
    }
    guard let validated = backupPolicyIO.lstatInfo(directory.path), validated.isDirectory else {
      throw HoyaError.invalidRequest
    }
    let path = directory.path
    let io = backupPolicyIO
    try await beginOpen(
      engine: .object([
        "kind": .string("native"), "directory": .string(directory.path),
        "name": .string(name), "backup_excluded": .boolean(backupExcluded),
      ]), key: key, schema: schema, namespace: namespace, backup: backupExcluded,
      migrations: migrations
    ) {
      // SPEC-0036 item 5: the directory's reserved policy decides before the key-bearing open.
      try await BackupPolicyClaim.claim(
        path, directory: validated, excluded: backupExcluded, io: io)
    }
  }

  public func execute(_ operation: String, arguments: [String: ProtocolValue] = [:]) async throws
    -> ProtocolValue
  {
    guard operation != "open" else { throw HoyaError.invalidRequest }
    if operation == "drain"
      && (!observations.listeners.isEmpty || observations.registering != 0
        || observations.draining != nil)
    {
      throw HoyaError.busy
    }
    if operation == "close" {
      try await close()
      return .null
    }
    let generation = epoch
    // API-0011 §3.2: reconcile and rollback use the report-returning operations for typed,
    // advanced and managed calls alike. A malformed legacy frame keeps its original operation
    // and therefore the core's exact-shape rejection.
    var submitted = operation
    var submittedArguments = arguments
    if operation == "reconcile", Set(arguments.keys) == ["key", "plan"] {
      submitted = "reconcile_resolving"
      submittedArguments["guards"] = .array([])
    } else if operation == "reconcile_guarded" {
      submitted = "reconcile_resolving"
    } else if operation == "rollback" {
      submitted = "rollback_resolving"
    }
    let result = try await request(submitted, arguments: submittedArguments)
    if operation == "reset" {
      stopSync()
      observations.invalidate()
      invalidateCommands()
      invalidateEntityCallbacks()
      try advanceEpoch()
    } else if Self.observedMutations.contains(operation) {
      scheduleObservationDrain()
      scheduleObservationDeadline()
      // Preparing a push changes observable Sent state, but the sync driver
      // must not wake itself again merely because it checked its due queue.
      if operation != "prepare_push" { syncDriver?.wake() }
    }
    if ["finish", "abort", "cancel_request", "reset"].contains(operation) {
      await CommandWake.shared.notify()
    }
    if ["reconcile_resolving", "rollback_resolving", "discard"].contains(submitted) {
      guard let key = arguments["key"]?.string else { throw HoyaError.invalidResponse }
      deliverResolution(try Resolution(wire: result), key: key, generation: generation)
    }
    // The unit-returning operations drop only the report.
    return submitted == operation ? result : .null
  }

  public func schema() async throws -> Schema {
    try Schema(wire: await request("schema"))
  }

  public func protection() async throws -> Protection {
    let result = try await request("protection")
    guard let value = result.object, let encrypted = value["encrypted"]?.boolean,
      let cipher = value["cipher"]?.string, let version = value["version"]?.string,
      let backup = value["backup"]?.string
    else { throw HoyaError.invalidResponse }
    return Protection(
      encrypted: encrypted, cipher: cipher, version: version, backup: backup,
      verifiedBackupExcluded: verifiedBackupExcluded)
  }

  public func close() async throws {
    stopSync()
    observations.invalidate()
    invalidateCommands()
    invalidateEntityCallbacks()
    if let closing, let token = closingEpoch {
      try await finishClose(closing, token: token)
      return
    }
    if retiring == nil {
      guard owner != nil || opening != nil else { return }
      try advanceEpoch()
      let handle = owner
      let attempt = opening
      // A pending policy claim is cancelled; a submitted native open is awaited and disposed.
      if !openSubmitted { attempt?.cancel() }
      retiring = Task {
        if let handle { return handle }
        return try? await attempt?.value.handle
      }
      owner = nil
      verifiedBackupExcluded = nil
    }
    guard let retiring else { return }
    let runtime = runtime
    let task = Task {
      if let handle = await retiring.value {
        _ = try await runtime.call(Wire.frame("close", owner: handle, args: [:]).encoded())
      }
    }
    closing = task
    closingEpoch = epoch
    try await finishClose(task, token: epoch)
  }

  private func finishClose(_ task: Task<Void, any Error>, token: UInt64) async throws {
    do {
      try await task.value
      await CommandWake.shared.notify()
      if closingEpoch == token {
        closing = nil
        closingEpoch = nil
        retiring = nil
        opening = nil
        memoryStore = nil
      }
    } catch {
      if closingEpoch == token {
        closing = nil
        closingEpoch = nil
      }
      // Retain the result handle for disposal retry; never replay key-bearing open.
      throw error
    }
  }

  /// API-0011 §2: close/reset invalidate host callbacks and merge caches of the retired epoch.
  private func invalidateEntityCallbacks() {
    entityContext = nil
    mergeHooks = [:]
  }

  private func requireClosed() throws {
    guard owner == nil, opening == nil, retiring == nil, closing == nil else {
      throw HoyaError.busy
    }
  }

  /// Admits native submission only for the current, uncancelled Opening attempt.
  private func admitOpen(_ token: UInt64) throws {
    guard epoch == token, retiring == nil, closing == nil, !Task.isCancelled else {
      throw HoyaError.closed
    }
    openSubmitted = true
  }

  private func advanceEpoch() throws {
    guard epoch < UInt64.max else { throw HoyaError.core("LimitExceeded") }
    epoch += 1
  }

  private func beginOpen(
    engine: Wire, key: Data, schema: Schema, namespace: String, backup: Bool?,
    migrations: [Migration] = [], store: MemoryStore? = nil,
    prepare: (@Sendable () async throws -> Void)? = nil
  ) async throws {
    try requireClosed()
    try advanceEpoch()
    let token = epoch
    let input = try Wire.frame(
      "open",
      args: [
        "engine": engine, "schema": schema.wire,
        "migrations": .array(try migrations.map { try $0.wire }),
        "namespace": .string(namespace),
        "options": options.wire,
      ]
    ).encoded()
    memoryStore = store
    let runtime = runtime
    let task = Task {
      if let prepare {
        do {
          try await prepare()
        } catch {
          // A claim outcome after close reports the closed attempt, not the claim.
          if error is CancellationError || Task.isCancelled { throw HoyaError.closed }
          throw error
        }
        try admitOpen(token)
      }
      let value = try await runtime.open(input, key: key)
      guard let handle = value.string else { throw HoyaError.invalidResponse }
      if epoch == token, retiring == nil, closing == nil, let conflict {
        do {
          _ = try await runtime.call(
            Wire.frame("set_conflict_policy", owner: handle, args: ["policy": conflict.wire])
              .encoded())
        } catch {
          // Return ownership even on configuration failure: close/deinit must
          // await this attempt and dispose its exact handle, including retries.
          return OpenedOwner(handle: handle, configurationError: error)
        }
      }
      return OpenedOwner(handle: handle, configurationError: nil)
    }
    opening = task
    openSubmitted = prepare == nil
    do {
      let opened = try await task.value
      guard epoch == token, retiring == nil, closing == nil else { throw HoyaError.closed }
      if let failure = opened.configurationError {
        retiring = Task { opened.handle }
        // A failed close remains retryable and its error is returned; never
        // publish the owner or lose the handle to conceal a cleanup failure.
        try await close()
        throw failure
      }
      owner = opened.handle
      opening = nil
      verifiedBackupExcluded = backup
    } catch {
      if epoch == token, retiring == nil, closing == nil {
        opening = nil
        memoryStore = nil
      }
      throw error
    }
  }

  func ownerGeneration() throws -> UInt64 {
    guard owner != nil else { throw HoyaError.closed }
    return epoch
  }

  /// SPEC-0036: one redacted fact per failure of SDK-initiated work in the current epoch.
  func reportBackground(
    _ source: BackgroundError.Source, _ kind: String, token: String?, generation: UInt64
  ) {
    guard generation == epoch, owner != nil, let sink = onBackgroundError else { return }
    sink(BackgroundError(source: source, kind: BackgroundError.publicKind(kind), token: token))
  }

  func request(_ op: String, arguments: [String: Wire] = [:]) async throws -> Wire {
    guard let owner else { throw HoyaError.closed }
    let token = epoch
    let value = try await runtime.call(Wire.frame(op, owner: owner, args: arguments).encoded())
    guard epoch == token, self.owner == owner else { throw HoyaError.closed }
    return value
  }

  func submitCommandFinish(_ control: CommandControl, arguments: [String: Wire]) async throws
    -> Wire
  {
    guard let owner else { throw HoyaError.closed }
    let generation = epoch
    // Complete bounded encoding before the irrevocable submission decision.
    let frame = try Wire.frame("finish", owner: owner, args: arguments).encoded()
    try control.submit()
    let value = try await runtime.call(frame)
    if generation == epoch, self.owner == owner {
      scheduleObservationDrain()
      scheduleObservationDeadline()
      syncDriver?.wake()
    }
    return value
  }

  private static let observedMutations: Set<String> = [
    "insert", "replace", "remove", "batch", "link", "unlink", "finish", "refresh",
    "prepare_push", "request_redelivery", "reconcile_guarded", "reconcile_resolving",
    "rollback_resolving", "discard",
    "reconcile", "rollback", "cancel", "receive", "receive_entity", "receive_wire",
    "entity_batch", "patch", "entity_link", "entity_unlink", "apply_pull", "merge_commit",
  ]

}
