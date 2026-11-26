// SPEC-0040 / API-0011: two reactions, one bounded resolution report.
import Foundation

public enum ConflictReaction: String, Sendable { case stop, discard }

public struct ConflictPolicy: Equatable, Sendable {
  public let defaultReaction: ConflictReaction
  public let commands: [String: ConflictReaction]

  public init(
    default defaultReaction: ConflictReaction = .stop, commands: [String: ConflictReaction] = [:]
  ) {
    self.defaultReaction = defaultReaction
    self.commands = commands
  }

  var wire: Wire {
    .object([
      "default": .string(defaultReaction.rawValue),
      "commands": .object(commands.mapValues { .string($0.rawValue) }),
    ])
  }
}

public enum Resolution: Equatable, Sendable {
  case completed
  case keptCurrent(discardedKeys: [String], unmapped: [String], skippedWrites: UInt64)
}

public typealias ResolutionCallback = @Sendable (String, Resolution) throws -> Void

extension Resolution {
  init(wire: Wire) throws {
    guard let value = wire.object, let kind = value["kind"]?.string else {
      throw HoyaError.invalidResponse
    }
    if kind == "completed", value.count == 1 {
      self = .completed
      return
    }
    guard kind == "kept_current", value.count == 4,
      case .array(let keys) = value["discarded_keys"],
      case .array(let unmapped) = value["unmapped"], let skipped = value["skipped_writes"]
    else { throw HoyaError.invalidResponse }
    func strings(_ values: [Wire]) throws -> [String] {
      try values.map {
        guard let value = $0.string else { throw HoyaError.invalidResponse }
        return value
      }
    }
    self = .keptCurrent(
      discardedKeys: try strings(keys), unmapped: try strings(unmapped),
      skippedWrites: try skipped.unsignedInteger())
  }
}

extension Database {
  /// Changes only this open owner. Reopen reapplies the constructor's host configuration.
  public func setConflictPolicy(_ policy: ConflictPolicy) async throws {
    _ = try await execute("set_conflict_policy", arguments: ["policy": policy.wire])
  }

  public func reconcileResolving(_ key: String, plan: ReconcilePlan, guards: [CorrectionGuard] = [])
    async throws -> Resolution
  {
    try Resolution(
      wire: await execute(
        "reconcile_resolving",
        arguments: [
          "key": .string(key), "plan": plan.wire,
          "guards": .array(try guards.map { try $0.wire }),
        ]))
  }

  public func rollbackResolving(_ key: String) async throws -> Resolution {
    try Resolution(wire: await execute("rollback_resolving", arguments: ["key": .string(key)]))
  }

  public func discard(_ key: String) async throws -> Resolution {
    try Resolution(wire: await execute("discard", arguments: ["key": .string(key)]))
  }

  func deliverResolution(_ result: Resolution, key: String, generation: UInt64) {
    guard generation == epoch, case .keptCurrent = result, let onResolution else { return }
    do {
      try onResolution(key, result)
    } catch {
      reportBackground(.callback, "CallbackFailure", token: nil, generation: generation)
    }
  }
}
