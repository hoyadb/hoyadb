// Ordinary application imports only the public Swift SDK. No testable/internal API.
import Foundation
import HoyaDB

private actor ForeignGate {
  private var released = false
  private var continuation: CheckedContinuation<Void, Never>?

  func wait() async {
    if released { return }
    await withCheckedContinuation { continuation = $0 }
  }

  func release() {
    released = true
    continuation?.resume()
    continuation = nil
  }
}

private final class ForeignCompletion: @unchecked Sendable {
  private let lock = NSLock()
  private var callback: (@Sendable (String?) -> Void)?

  init(_ callback: @escaping @Sendable (String?) -> Void) { self.callback = callback }

  func finish(_ error: String?) {
    lock.lock()
    let work = callback
    callback = nil
    lock.unlock()
    work?(error)
  }
}

private actor ForeignState {
  static let shared = ForeignState()
  private var database: Database?
  private var gate: ForeignGate?
  private var task: Task<Void, Error>?

  func prepare() async throws {
    guard database == nil else { throw HoyaError.busy }
    let db = Database()
    try await db.openMemory(
      schema: Schema(version: 1, tables: ["foreign_notes": Table(columns: ["label": Column(.string)])]),
      namespace: "a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6a6")
    database = db
  }

  func start(_ callback: ForeignCompletion) throws {
    guard let database, task == nil else { throw HoyaError.busy }
    let gate = ForeignGate()
    self.gate = gate
    task = Task {
      do {
        _ = try await database.command(
          CommandDefinition("swift-foreign-v1"), value: .object([:]),
          context: CapturedContext(identity: .string("foreign-consumer"), nowMilliseconds: 1000)
        ) { scope in
          _ = try await scope.fetch(Query("foreign_notes"))
          callback.finish(nil)
          await gate.wait()
          return .null
        }
      } catch {
        callback.finish("ForeignFailure")
        throw error
      }
    }
  }

  func release() async throws {
    guard let task, let gate else { throw HoyaError.closed }
    await gate.release()
    try await task.value
    self.task = nil
    self.gate = nil
  }

  func close() async throws {
    task?.cancel()
    await gate?.release()
    _ = try? await task?.value
    task = nil
    gate = nil
    if let database { try await database.close() }
    database = nil
  }
}

// Public: the generated Objective-C header exposes only public @objc declarations.
@objc(HoyaForeignControl)
public final class HoyaForeignControl: NSObject {
  @objc public static func prepare(_ completion: @escaping @Sendable (String?) -> Void) {
    Task {
      do { try await ForeignState.shared.prepare(); completion(nil) }
      catch { completion("ForeignFailure") }
    }
  }

  @objc public static func start(_ completion: @escaping @Sendable (String?) -> Void) {
    let once = ForeignCompletion(completion)
    Task {
      do { try await ForeignState.shared.start(once) }
      catch { once.finish("ForeignFailure") }
    }
  }

  @objc public static func releaseScope(_ completion: @escaping @Sendable (String?) -> Void) {
    Task {
      do { try await ForeignState.shared.release(); completion(nil) }
      catch { completion("ForeignFailure") }
    }
  }

  @objc public static func close(_ completion: @escaping @Sendable (String?) -> Void) {
    Task {
      do { try await ForeignState.shared.close(); completion(nil) }
      catch { completion("ForeignFailure") }
    }
  }
}
