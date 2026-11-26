// API-0011: exact structural translation only; all database semantics stay in Rust.
import CoreFoundation
import Foundation

public indirect enum ProtocolValue: Equatable, Sendable {
  case null
  case boolean(Bool)
  case string(String)
  case array([ProtocolValue])
  case object([String: ProtocolValue])

  var object: [String: Wire]? {
    if case .object(let fields) = self { return fields }
    return nil
  }

  var string: String? {
    if case .string(let value) = self { return value }
    return nil
  }

  var boolean: Bool? {
    if case .boolean(let value) = self { return value }
    return nil
  }

  private var foundation: Any {
    switch self {
    case .null: return NSNull()
    case .boolean(let value): return value
    case .string(let value): return value
    case .array(let values): return values.map(\.foundation)
    case .object(let fields): return fields.mapValues(\.foundation)
    }
  }

  func encoded() throws -> Data {
    var pending: [(Wire, Int)] = [(self, 0)]
    var nodes = 0
    while let (value, depth) = pending.popLast() {
      nodes += 1
      guard depth <= 64, nodes <= 100_000 else { throw HoyaError.core("LimitExceeded") }
      let children: [Wire]
      switch value {
      case .array(let values): children = values
      case .object(let fields): children = Array(fields.values)
      default: children = []
      }
      guard children.count <= 100_000 - nodes - pending.count else {
        throw HoyaError.core("LimitExceeded")
      }
      pending.append(contentsOf: children.map { ($0, depth + 1) })
    }
    do {
      let data = try JSONSerialization.data(withJSONObject: foundation, options: [.sortedKeys])
      guard data.count <= 1_048_576 else { throw HoyaError.core("LimitExceeded") }
      return data
    } catch let error as HoyaError {
      throw error
    } catch {
      throw HoyaError.invalidRequest
    }
  }

  static func decode(_ data: Data) throws -> Wire {
    guard data.count <= 1_048_576 else { throw HoyaError.invalidResponse }
    do {
      let root = try JSONSerialization.jsonObject(with: data)
      var nodes = 0
      func walk(_ value: Any, depth: Int) throws -> Wire {
        nodes += 1
        guard depth <= 64, nodes <= 100_000 else { throw HoyaError.invalidResponse }
        if value is NSNull { return .null }
        if let string = value as? String { return .string(string) }
        if let number = value as? NSNumber {
          guard CFGetTypeID(number) == CFBooleanGetTypeID() else {
            throw HoyaError.invalidResponse
          }
          return .boolean(number.boolValue)
        }
        if let array = value as? [Any] {
          return .array(try array.map { try walk($0, depth: depth + 1) })
        }
        if let fields = value as? [String: Any] {
          return .object(try fields.mapValues { try walk($0, depth: depth + 1) })
        }
        throw HoyaError.invalidResponse
      }
      return try walk(root, depth: 0)
    } catch let error as HoyaError {
      throw error
    } catch {
      throw HoyaError.invalidResponse
    }
  }

  func successfulReply() throws -> Wire {
    guard let fields = object, fields["abi"] == .string("1"),
      let success = fields["ok"]?.boolean
    else { throw HoyaError.invalidResponse }
    if success {
      guard Set(fields.keys) == ["abi", "ok", "value"], let value = fields["value"] else {
        throw HoyaError.invalidResponse
      }
      return value
    }
    guard Set(fields.keys) == ["abi", "ok", "error"],
      let error = fields["error"]?.object, Set(error.keys) == ["kind"],
      let kind = error["kind"]?.string
    else { throw HoyaError.invalidResponse }
    switch kind {
    case "Busy": throw HoyaError.busy
    case "Closed": throw HoyaError.closed
    case "InvalidRequest": throw HoyaError.invalidRequest
    default: throw HoyaError.core(kind)
    }
  }

  static func frame(_ op: String, owner: String? = nil, args: [String: Wire]) -> Wire {
    .object([
      "abi": .string("1"), "op": .string(op),
      "owner": owner.map(Wire.string) ?? .null, "args": .object(args),
    ])
  }
}

typealias Wire = ProtocolValue

extension Column {
  var wire: Wire {
    .object(["kind": .string(kind.rawValue), "optional": .boolean(optional)])
  }
}

extension Relation {
  var wire: Wire {
    switch self {
    case .toOne(let table, let key, let optional):
      return .object([
        "kind": .string("ToOne"), "table": .string(table), "foreign_key": .string(key),
        "optional": .boolean(optional),
      ])
    case .hasMany(let table, let key):
      return .object([
        "kind": .string("HasMany"), "table": .string(table), "foreign_key": .string(key),
      ])
    case .manyToMany(let table, let through, let source, let target):
      return .object([
        "kind": .string("ManyToMany"), "table": .string(table), "through": .string(through),
        "source_relation": .string(source), "target_relation": .string(target),
      ])
    }
  }
}

extension Table {
  var wire: Wire {
    .object([
      "columns": .object(columns.mapValues(\.wire)),
      "relations": .object(relations.mapValues(\.wire)),
    ])
  }
}

extension Schema {
  var wire: Wire {
    .object(["version": .string(String(version)), "tables": .object(tables.mapValues(\.wire))])
  }

  init(wire: Wire) throws {
    guard let fields = wire.object, let text = fields["version"]?.string,
      let version = UInt32(text), let tables = fields["tables"]?.object
    else { throw HoyaError.invalidResponse }
    self.version = version
    self.tables = try tables.mapValues { table in
      guard let value = table.object, let columns = value["columns"]?.object,
        let relations = value["relations"]?.object
      else { throw HoyaError.invalidResponse }
      let decodedColumns = try columns.mapValues { column in
        guard let column = column.object, let text = column["kind"]?.string,
          let kind = Column.Kind(rawValue: text), let optional = column["optional"]?.boolean
        else { throw HoyaError.invalidResponse }
        return Column(kind, optional: optional)
      }
      let decodedRelations: [String: Relation] = try relations.mapValues { relation in
        guard let relation = relation.object, let kind = relation["kind"]?.string,
          let table = relation["table"]?.string,
          let key = relation["foreign_key"]?.string
            ?? (kind == "ManyToMany" ? "" : nil)
        else { throw HoyaError.invalidResponse }
        switch kind {
        case "ToOne":
          guard let optional = relation["optional"]?.boolean else {
            throw HoyaError.invalidResponse
          }
          return .toOne(table: table, foreignKey: key, optional: optional)
        case "HasMany": return .hasMany(table: table, foreignKey: key)
        case "ManyToMany":
          guard let through = relation["through"]?.string,
            let source = relation["source_relation"]?.string,
            let target = relation["target_relation"]?.string
          else { throw HoyaError.invalidResponse }
          return .manyToMany(
            table: table, through: through, sourceRelation: source, targetRelation: target)
        default: throw HoyaError.invalidResponse
        }
      }
      return Table(columns: decodedColumns, relations: decodedRelations)
    }
  }
}
