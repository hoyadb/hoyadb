// API-0011: the borrowed C reply is copied on a dedicated serial executor.
import Foundation
import HoyaNative

protocol NativeCalling: Sendable {
  func open(_ input: Data, key: Data) async throws -> Wire
  func call(_ input: Data) async throws -> Wire
}

private final class ReplyCollector {
  var bytes: Data?
  var calls = 0
  var valid = true
}

private let copyReply:
  @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void = {
    context, bytes, length in
    guard let context else { return }
    let collector = Unmanaged<ReplyCollector>.fromOpaque(context).takeUnretainedValue()
    collector.calls += 1
    guard length >= 0, length <= 1_048_576, let bytes else {
      collector.valid = false
      return
    }
    collector.bytes = Data(bytes: bytes, count: length)
  }

final class NativeRuntime: NativeCalling, @unchecked Sendable {
  static let shared = NativeRuntime()
  private let queue = DispatchQueue(label: "dev.hoyadb.native")
  private let releaseObserver: any NSObjectProtocol

  init() {
    releaseObserver = NotificationCenter.default.addObserver(
      forName: NativeAdmission.notificationName, object: nil, queue: nil
    ) { _ in
      Task { await CommandWake.shared.notify() }
    }
  }

  deinit { NotificationCenter.default.removeObserver(releaseObserver) }

  func open(_ input: Data, key: Data) async throws -> Wire {
    try await invoke(input, key: key)
  }

  func call(_ input: Data) async throws -> Wire {
    try await invoke(input, key: nil)
  }

  private func invoke(_ input: Data, key: Data?) async throws -> Wire {
    try await withCheckedThrowingContinuation { continuation in
      queue.async {
        let collector = ReplyCollector()
        let context = Unmanaged.passUnretained(collector).toOpaque()
        let status = input.withUnsafeBytes { (json: UnsafeRawBufferPointer) in
          if let key {
            return key.withUnsafeBytes { (key: UnsafeRawBufferPointer) in
              hoya_v1_open(
                json.bindMemory(to: UInt8.self).baseAddress, json.count,
                key.bindMemory(to: UInt8.self).baseAddress, key.count, copyReply, context)
            }
          }
          return hoya_v1_call(
            json.bindMemory(to: UInt8.self).baseAddress, json.count, copyReply, context)
        }
        // C has returned: neither the collector nor its registry guard spans
        // this payload-free platform wake, including failed terminal attempts.
        NativeAdmission.notifyAfterCall(input)
        guard status == 0 else {
          continuation.resume(throwing: status == 1 ? HoyaError.invalidRequest : .boundaryFailure)
          return
        }
        guard collector.valid, collector.calls == 1, let bytes = collector.bytes else {
          continuation.resume(throwing: HoyaError.invalidResponse)
          return
        }
        do {
          continuation.resume(returning: try Wire.decode(bytes).successfulReply())
        } catch {
          continuation.resume(throwing: error)
        }
      }
    }
  }
}
