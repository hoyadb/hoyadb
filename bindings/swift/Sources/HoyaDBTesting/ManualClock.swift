// API-0011 testing entry / ST03: host scheduling only, never core deadlines.
import Foundation
import HoyaDB

public final class ManualClock: @unchecked Sendable {
  private let lock = NSLock()
  private var value: Int64

  public init(_ initial: Int64 = 0) { value = initial }

  public func now() -> Int64 {
    lock.lock()
    defer { lock.unlock() }
    return value
  }

  public func set(_ value: Int64) {
    lock.lock()
    self.value = value
    lock.unlock()
  }

  public func advance(by delta: Int64) throws {
    lock.lock()
    defer { lock.unlock() }
    let (next, overflow) = value.addingReportingOverflow(delta)
    guard !overflow else { throw HoyaError.invalidRequest }
    value = next
  }
}

public final class ManualUptime: @unchecked Sendable {
  private let lock = NSLock()
  private var value: UInt64

  public init(_ initial: UInt64 = 0) { value = initial }

  public func now() -> UInt64 {
    lock.lock()
    defer { lock.unlock() }
    return value
  }

  public func set(_ value: UInt64) {
    lock.lock()
    self.value = value
    lock.unlock()
  }

  public func advance(by delta: UInt64) throws {
    lock.lock()
    defer { lock.unlock() }
    let (next, overflow) = value.addingReportingOverflow(delta)
    guard !overflow else { throw HoyaError.invalidRequest }
    value = next
  }
}

private struct ManualTicket: TimerCancellation {
  let remove: @Sendable () -> Void
  func cancel() { remove() }
}

public final class ManualTimer: @unchecked Sendable {
  private let lock = NSLock()
  private var next: UInt64 = 0
  private var jobs: [(id: UInt64, delay: UInt64, work: @Sendable () -> Void)] = []

  public init() {}

  public var pendingDelays: [UInt64] {
    lock.lock()
    defer { lock.unlock() }
    return jobs.map(\.delay)
  }

  public func schedule(_ delay: UInt64, _ work: @escaping @Sendable () -> Void)
    -> any TimerCancellation
  {
    lock.lock()
    next += 1
    let id = next
    jobs.append((id, delay, work))
    lock.unlock()
    return ManualTicket { [weak self] in self?.remove(id) }
  }

  private func remove(_ id: UInt64) {
    lock.lock()
    jobs.removeAll { $0.id == id }
    lock.unlock()
  }

  @discardableResult public func fireNext() -> Bool {
    lock.lock()
    let work = jobs.isEmpty ? nil : jobs.removeFirst().work
    lock.unlock()
    guard let work else { return false }
    work()
    return true
  }
}
