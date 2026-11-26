// API-0011: managed joins remain core-owned and queryable.
import Foundation

private func relationArguments(_ table: String, id: String, relation: String, target: String)
  -> [String: Wire]
{
  [
    "table": .string(table), "id": .string(id), "relation": .string(relation),
    "target": .string(target),
  ]
}

extension Database {
  public func link(
    _ table: String, id: String, relation: String, target: String, fields: [String: Cell] = [:],
    proposedID: String? = nil
  ) async throws -> Row {
    var args = relationArguments(table, id: id, relation: relation, target: target)
    args["fields"] = .object(try fields.mapValues { try $0.wire })
    args["proposed_id"] = try proposedIDValue(proposedID)
    return try Row(wire: await execute("link", arguments: args))
  }

  public func unlink(_ table: String, id: String, relation: String, target: String) async throws
    -> UInt64
  {
    try await execute(
      "unlink", arguments: relationArguments(table, id: id, relation: relation, target: target)
    ).unsignedInteger()
  }

  public func entityLink(
    _ table: String, id: String, relation: String, target: String, fields: [String: Cell] = [:]
  ) async throws -> Row {
    var args = relationArguments(table, id: id, relation: relation, target: target)
    args["fields"] = .object(try fields.mapValues { try $0.wire })
    return try Row(wire: await entityMutation("entity_link", arguments: args))
  }

  public func entityUnlink(_ table: String, id: String, relation: String, target: String)
    async throws -> UInt64
  {
    try await entityMutation(
      "entity_unlink",
      arguments: relationArguments(table, id: id, relation: relation, target: target)
    ).unsignedInteger()
  }
}

extension CommandScope {
  public func link(
    _ table: String, id: String, relation: String, target: String, fields: [String: Cell] = [:],
    proposedID: String? = nil
  ) async throws -> Row {
    var args = relationArguments(table, id: id, relation: relation, target: target)
    args["fields"] = .object(try fields.mapValues { try $0.wire })
    args["proposed_id"] = proposedID.map(Wire.string) ?? .null
    return try Row(wire: await call("scope_link", args))
  }

  public func unlink(_ table: String, id: String, relation: String, target: String) async throws
    -> UInt64
  {
    try await call(
      "scope_unlink", relationArguments(table, id: id, relation: relation, target: target)
    ).unsignedInteger()
  }
}
