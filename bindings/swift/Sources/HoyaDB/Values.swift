// API-0011 §3: lossless public values, translated to the existing Rust contracts.
import Foundation

public enum Cell: Equatable, Sendable {
  case null
  case boolean(Bool)
  case string(String)
  case integer(Int64)
  case float(Double)
  case timestamp(Int64)
  case reference(String)
}

public struct Row: Equatable, Sendable {
  public let id: String
  public let fields: [String: Cell]

  public init(id: String, fields: [String: Cell]) {
    self.id = id
    self.fields = fields
  }
}

public struct Query: Equatable, Sendable {
  public struct Predicate: Equatable, Sendable {
    public enum Operator: String, Sendable {
      case equal = "Eq"
      case notEqual = "Ne"
      case less = "Lt"
      case lessOrEqual = "Le"
      case greater = "Gt"
      case greaterOrEqual = "Ge"
      case oneOf = "In"
    }

    public let field: String
    public let operation: Operator
    public let values: [Cell]

    public init(_ field: String, _ operation: Operator, _ values: [Cell]) {
      self.field = field
      self.operation = operation
      self.values = values
    }
  }

  public struct Order: Equatable, Sendable {
    public enum Direction: String, Sendable {
      case ascending = "Asc"
      case descending = "Desc"
    }
    public let field: String
    public let direction: Direction

    public init(_ field: String, _ direction: Direction = .ascending) {
      self.field = field
      self.direction = direction
    }
  }

  public struct Source: Equatable, Sendable {
    public let table: String
    public let id: String
    public let relation: String

    public init(table: String, id: String, relation: String) {
      self.table = table
      self.id = id
      self.relation = relation
    }
  }

  public let table: String
  public let predicates: [Predicate]
  public let order: Order?
  public let limit: UInt32?
  public let source: Source?

  public init(
    _ table: String, predicates: [Predicate] = [], order: Order? = nil,
    limit: UInt32? = nil, source: Source? = nil
  ) {
    self.table = table
    self.predicates = predicates
    self.order = order
    self.limit = limit
    self.source = source
  }
}

public enum Write: Equatable, Sendable {
  case insert(table: String, fields: [String: Cell], proposedID: String? = nil)
  case replace(table: String, id: String, fields: [String: Cell])
  case remove(table: String, id: String)
}

public struct Change: Equatable, Sendable {
  public let table: String
  public let id: String
  public let before: Row?
  public let after: Row?
}

extension Cell {
  var wire: Wire {
    get throws {
      switch self {
      case .null: return .object(["kind": .string("null")])
      case .boolean(let value):
        return .object(["kind": .string("boolean"), "value": .boolean(value)])
      case .string(let value): return .object(["kind": .string("string"), "value": .string(value)])
      case .reference(let value):
        return .object(["kind": .string("reference"), "value": .string(value)])
      case .integer(let value):
        return .object(["kind": .string("integer"), "value": .string(String(value))])
      case .timestamp(let value):
        return .object(["kind": .string("timestamp"), "value": .string(String(value))])
      case .float(let value):
        guard value.isFinite else { throw HoyaError.invalidRequest }
        return .object(["kind": .string("float"), "value": .string(String(value))])
      }
    }
  }

  init(wire: Wire) throws {
    guard let object = wire.object, let kind = object["kind"]?.string else {
      throw HoyaError.invalidResponse
    }
    if kind == "null" {
      guard object.count == 1 else { throw HoyaError.invalidResponse }
      self = .null
      return
    }
    guard object.count == 2, let value = object["value"] else { throw HoyaError.invalidResponse }
    switch kind {
    case "boolean":
      guard let value = value.boolean else { throw HoyaError.invalidResponse }
      self = .boolean(value)
    case "string", "reference":
      guard let value = value.string else { throw HoyaError.invalidResponse }
      self = kind == "string" ? .string(value) : .reference(value)
    case "integer", "timestamp":
      guard let value = value.string, let number = Int64(value), String(number) == value else {
        throw HoyaError.invalidResponse
      }
      self = kind == "integer" ? .integer(number) : .timestamp(number)
    case "float":
      guard let value = value.string, let number = Double(value), number.isFinite else {
        throw HoyaError.invalidResponse
      }
      self = .float(number)
    default: throw HoyaError.invalidResponse
    }
  }
}

extension Row {
  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 2,
      let id = object["id"]?.string, let fields = object["fields"]?.object
    else { throw HoyaError.invalidResponse }
    self.init(id: id, fields: try fields.mapValues { try Cell(wire: $0) })
  }

  var wire: Wire {
    get throws {
      .object(["id": .string(id), "fields": .object(try fields.mapValues { try $0.wire })])
    }
  }
}

extension Query {
  var wire: Wire {
    get throws {
      let predicates: [Wire] = try self.predicates.map { predicate in
        .object([
          "field": .string(predicate.field), "operator": .string(predicate.operation.rawValue),
          "values": .array(try predicate.values.map { try $0.wire }),
        ])
      }
      let order: Wire =
        self.order.map {
          .object(["field": .string($0.field), "direction": .string($0.direction.rawValue)])
        } ?? .null
      let source: Wire =
        self.source.map {
          .object([
            "table": .string($0.table), "id": .string($0.id), "relation": .string($0.relation),
          ])
        } ?? .null
      return .object([
        "table": .string(table), "predicates": .array(predicates), "order": order,
        "limit": limit.map { .string(String($0)) } ?? .null, "source": source,
      ])
    }
  }
}

extension Write {
  var wire: Wire {
    get throws {
      switch self {
      case .insert(let table, let fields, let proposal):
        return .object([
          "op": .string("insert"), "table": .string(table),
          "fields": .object(try fields.mapValues { try $0.wire }),
          "proposed_id": proposal.map(Wire.string) ?? .null,
        ])
      case .replace(let table, let id, let fields):
        return .object([
          "op": .string("replace"), "table": .string(table), "id": .string(id),
          "fields": .object(try fields.mapValues { try $0.wire }),
        ])
      case .remove(let table, let id):
        return .object(["op": .string("remove"), "table": .string(table), "id": .string(id)])
      }
    }
  }
}

extension Change {
  init(wire: Wire) throws {
    guard let object = wire.object, object.count == 4, let table = object["table"]?.string,
      let id = object["id"]?.string, let before = object["before"], let after = object["after"]
    else { throw HoyaError.invalidResponse }
    self.table = table
    self.id = id
    self.before = before == .null ? nil : try Row(wire: before)
    self.after = after == .null ? nil : try Row(wire: after)
  }
}

extension Database {
  func proposedIDValue(_ explicit: String?) throws -> Wire {
    if let explicit { return .string(explicit) }
    guard let idGenerator else { return .null }
    guard let value = try idGenerator(), !value.isEmpty else { throw HoyaError.invalidRequest }
    return .string(value)
  }

  func prepareWrites(_ writes: [Write]) throws -> [Wire] {
    try writes.map { write in
      let value = try write.wire
      guard case .insert(_, _, let explicit) = write, var object = value.object else {
        return value
      }
      object["proposed_id"] = try proposedIDValue(explicit)
      return .object(object)
    }
  }

  public func insert(_ table: String, fields: [String: Cell], proposedID: String? = nil)
    async throws -> Row
  {
    try Row(
      wire: await execute(
        "insert",
        arguments: [
          "table": .string(table),
          "fields": .object(try fields.mapValues { try $0.wire }),
          "proposed_id": try proposedIDValue(proposedID),
        ]))
  }

  public func replace(_ table: String, id: String, fields: [String: Cell]) async throws -> Row {
    try Row(
      wire: await execute(
        "replace",
        arguments: [
          "table": .string(table), "id": .string(id),
          "fields": .object(try fields.mapValues { try $0.wire }),
        ]))
  }

  public func remove(_ table: String, id: String) async throws -> Bool {
    let result = try await execute(
      "remove", arguments: ["table": .string(table), "id": .string(id)])
    guard let value = result.boolean else { throw HoyaError.invalidResponse }
    return value
  }

  public func fetch(_ query: Query) async throws -> [Row] {
    let result = try await execute("fetch", arguments: ["query": query.wire])
    guard case .array(let rows) = result else { throw HoyaError.invalidResponse }
    return try rows.map { try Row(wire: $0) }
  }

  public func batch(_ writes: [Write]) async throws -> [Change] {
    let result = try await execute(
      "batch", arguments: ["writes": .array(try prepareWrites(writes))])
    guard case .array(let changes) = result else { throw HoyaError.invalidResponse }
    return try changes.map { try Change(wire: $0) }
  }
}
