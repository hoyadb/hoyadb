// API-0011 / TC-0036: caller schema values; validation remains in Rust.
import Foundation

public enum HoyaError: Error, Equatable, Sendable {
  case invalidRequest
  case invalidResponse
  case boundaryFailure
  case backupPolicyFailure
  case busy
  case closed
  case core(String)
  /// SPEC-0036: the subscription ended with this kind (for example "LimitExceeded");
  /// it delivers nothing further.
  case subscriptionTerminated(String)
}

/// SPEC-0036 redacted background failure: no message, key, payload or row.
public struct BackgroundError: Equatable, Sendable {
  public enum Source: String, Sendable { case observation, callback, sync, merge, timer }
  public let source: Source
  public let kind: String
  public let token: String?

  public init(source: Source, kind: String, token: String?) {
    self.source = source
    self.kind = kind
    self.token = token
  }

  /// Fixed public kinds only; anything else, including a malformed wire kind, is BackendFailure.
  static let publicKinds: Set<String> = [
    "BackendFailure", "MigrationFailure", "SchemaConflict", "InvalidSchema", "InvalidQuery",
    "InvalidRow", "TableNotFound", "RowAlreadyExists", "RowNotFound", "InvalidCommand",
    "InvalidTicket", "Busy", "Capacity", "InvalidPayload", "Protocol", "UnknownLocalReference",
    "RollbackConflict", "Closed", "LimitExceeded", "AuthorityRequired", "StalePull",
    "PullConflict", "InvalidRequest", "InvalidHandle", "UnexpectedPending",
    "UnsupportedOperation", "Cancelled", "CallbackFailure",
  ]

  static func publicKind(_ kind: String?) -> String {
    guard let kind, publicKinds.contains(kind) else { return "BackendFailure" }
    return kind
  }
}

extension HoyaError {
  /// Redacted public kind of an SDK error for background reports.
  var publicKind: String {
    switch self {
    case .invalidRequest: return "InvalidRequest"
    case .invalidResponse: return "InvalidPayload"
    case .boundaryFailure: return "BackendFailure"
    case .backupPolicyFailure: return "BackendFailure"
    case .busy: return "Busy"
    case .closed: return "Closed"
    case .core(let kind): return BackgroundError.publicKind(kind)
    case .subscriptionTerminated(let kind): return BackgroundError.publicKind(kind)
    }
  }
}

public struct Column: Equatable, Sendable {
  public enum Kind: String, Sendable {
    case string = "String"
    case integer = "Integer"
    case float = "Float"
    case boolean = "Boolean"
    case timestamp = "Timestamp"
    case reference = "Reference"
  }

  public let kind: Kind
  public let optional: Bool

  public init(_ kind: Kind, optional: Bool = false) {
    self.kind = kind
    self.optional = optional
  }
}

public enum Relation: Equatable, Sendable {
  case toOne(table: String, foreignKey: String, optional: Bool)
  case hasMany(table: String, foreignKey: String)
  case manyToMany(table: String, through: String, sourceRelation: String, targetRelation: String)
}

public struct Table: Equatable, Sendable {
  public let columns: [String: Column]
  public let relations: [String: Relation]

  public init(columns: [String: Column], relations: [String: Relation] = [:]) {
    self.columns = columns
    self.relations = relations
  }
}

public struct Schema: Equatable, Sendable {
  public let version: UInt32
  public let tables: [String: Table]

  public init(version: UInt32, tables: [String: Table]) {
    self.version = version
    self.tables = tables
  }
}

public struct Protection: Equatable, Sendable {
  public let encrypted: Bool
  public let cipher: String
  public let version: String
  public let backup: String
  public let verifiedBackupExcluded: Bool?
}

public struct DatabaseOptions: Equatable, Sendable {
  public let capacity: UInt32
  public let history: UInt32
  public let initialDelayMilliseconds: Int64
  public let multiplier: UInt32
  public let maxDelayMilliseconds: Int64

  public init(
    capacity: UInt32 = 1000, history: UInt32 = 100, initialDelayMilliseconds: Int64 = 1000,
    multiplier: UInt32 = 2, maxDelayMilliseconds: Int64 = 60_000
  ) {
    self.capacity = capacity
    self.history = history
    self.initialDelayMilliseconds = initialDelayMilliseconds
    self.multiplier = multiplier
    self.maxDelayMilliseconds = maxDelayMilliseconds
  }

  var wire: Wire {
    .object([
      "capacity": .string(String(capacity)), "history": .string(String(history)),
      "initial_delay_ms": .string(String(initialDelayMilliseconds)),
      "multiplier": .string(String(multiplier)),
      "max_delay_ms": .string(String(maxDelayMilliseconds)),
    ])
  }
}
