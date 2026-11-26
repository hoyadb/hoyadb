// API-0011 testing entry / ST04–06: pure transport fixtures over real Rust JSON.
import HoyaDB

public enum FakeOutcome: Sendable {
  case applied(JSONValue)
  case corrected(JSONValue)
  case rejected(String)
}

public enum FakeTransportError: Error, Equatable, Sendable {
  case invalidEnvelope
  case missingHandler
  case droppedAcknowledgement
}

public actor FakeTransport {
  public typealias Handler = @Sendable (JSONValue) async throws -> FakeOutcome
  public typealias Delay = @Sendable () async throws -> Void
  public typealias Drop = @Sendable ([JSONValue]) async throws -> Bool
  public private(set) var pushes: [[JSONValue]] = []
  private let handlers: [String: Handler]
  private let entityHandlers: [String: Handler]
  private let delay: Delay
  private let dropAcknowledgement: Drop
  private let reverseReplies: Bool

  public init(
    handlers: [String: Handler] = [:], entityHandlers: [String: Handler] = [:],
    delay: @escaping Delay = {}, dropAcknowledgement: @escaping Drop = { _ in false },
    reverseReplies: Bool = false
  ) {
    self.handlers = handlers
    self.entityHandlers = entityHandlers
    self.delay = delay
    self.dropAcknowledgement = dropAcknowledgement
    self.reverseReplies = reverseReplies
  }

  public func push(_ body: String) async throws -> String {
    guard case .array(let envelopes) = try await JSONValue.decodeJSON(body) else {
      throw FakeTransportError.invalidEnvelope
    }
    pushes.append(envelopes)
    try await delay()
    var replies: [JSONValue] = []
    for envelope in envelopes {
      guard case .object(let fields) = envelope else { throw FakeTransportError.invalidEnvelope }
      func text(_ name: String) throws -> String {
        guard case .string(let value) = fields[name], !value.isEmpty else {
          throw FakeTransportError.invalidEnvelope
        }
        return value
      }
      let handler: Handler
      var correlation: [String: JSONValue]
      if fields["writeId"] != nil {
        let table = try text("table")
        guard let selected = entityHandlers[table] else { throw FakeTransportError.missingHandler }
        handler = selected
        correlation = [
          "table": .string(table), "recordId": .string(try text("recordId")),
          "writeId": .string(try text("writeId")),
        ]
      } else {
        let name = try text("command")
        guard let selected = handlers[name] else { throw FakeTransportError.missingHandler }
        handler = selected
        correlation = ["idempotencyKey": .string(try text("idempotencyKey"))]
      }
      switch try await handler(envelope) {
      case .applied(let result):
        correlation["status"] = .string("applied")
        correlation["result"] = result
      case .corrected(let result):
        correlation["status"] = .string("corrected")
        correlation["result"] = result
      case .rejected(let reason):
        correlation["status"] = .string("rejected")
        correlation["reason"] = .string(reason)
      }
      replies.append(.object(correlation))
    }
    if try await dropAcknowledgement(envelopes) { throw FakeTransportError.droppedAcknowledgement }
    if reverseReplies { replies.reverse() }
    return try await JSONValue.array(replies).encodeJSON()
  }
}
