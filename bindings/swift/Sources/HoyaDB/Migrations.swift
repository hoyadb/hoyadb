// API-0011 §3.2: authored migrations and actual retained memory backend ownership.
import Foundation

public struct Migration: Equatable, Sendable {
  public let fromVersion: UInt32
  public let to: Schema
  public let steps: [MigrationStep]

  public init(fromVersion: UInt32, to: Schema, steps: [MigrationStep]) {
    self.fromVersion = fromVersion
    self.to = to
    self.steps = steps
  }
}

public enum MigrationStep: Equatable, Sendable {
  case addTable(String, Table)
  case dropTable(String)
  case addColumn(table: String, name: String, column: Column, defaultValue: Cell?)
  case dropColumn(table: String, name: String)
  case renameColumn(table: String, from: String, to: String)
  case backfill(table: String, column: String, value: Cell)
  case setRelations(table: String, relations: [String: Relation])
}

public final class MemoryStore: Sendable {
  let handle: String

  init(_ handle: String) { self.handle = handle }

  public static func create() async throws -> MemoryStore {
    let value = try await NativeRuntime.shared.call(Wire.frame("memory_store", args: [:]).encoded())
    guard let handle = value.string else { throw HoyaError.invalidResponse }
    return MemoryStore(handle)
  }

  public func dispose() async throws {
    _ = try await NativeRuntime.shared.call(
      Wire.frame("memory_drop", args: ["store": .string(handle)]).encoded())
  }

  deinit {
    let handle = handle
    Task {
      _ = try? await NativeRuntime.shared.call(
        Wire.frame("memory_drop", args: ["store": .string(handle)]).encoded())
    }
  }
}

extension Migration {
  var wire: Wire {
    get throws {
      .object([
        "from_version": .string(String(fromVersion)), "to": to.wire,
        "steps": .array(try steps.map { try $0.wire }),
      ])
    }
  }
}

extension MigrationStep {
  var wire: Wire {
    get throws {
      switch self {
      case .addTable(let name, let definition):
        return .object([
          "op": .string("AddTable"), "table": .string(name), "definition": definition.wire,
        ])
      case .dropTable(let table):
        return .object(["op": .string("DropTable"), "table": .string(table)])
      case .addColumn(let table, let name, let column, let value):
        let defaultValue: Wire
        if let value {
          defaultValue = .object(["kind": .string("some"), "value": try value.wire])
        } else {
          defaultValue = .object(["kind": .string("none")])
        }
        return .object([
          "op": .string("AddColumn"), "table": .string(table), "name": .string(name),
          "column": column.wire, "default": defaultValue,
        ])
      case .dropColumn(let table, let name):
        return .object([
          "op": .string("DropColumn"), "table": .string(table), "name": .string(name),
        ])
      case .renameColumn(let table, let from, let to):
        return .object([
          "op": .string("RenameColumn"), "table": .string(table), "from": .string(from),
          "to": .string(to),
        ])
      case .backfill(let table, let column, let value):
        return .object([
          "op": .string("Backfill"), "table": .string(table), "column": .string(column),
          "value": try value.wire,
        ])
      case .setRelations(let table, let relations):
        return .object([
          "op": .string("SetRelations"), "table": .string(table),
          "relations": .object(relations.mapValues(\.wire)),
        ])
      }
    }
  }
}
