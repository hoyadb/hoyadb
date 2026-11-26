// TC-0036 G06: a foreign native-wrapper release wakes the existing core FIFO.
import Foundation
import XCTest

@testable import HoyaDB

private actor HeldFalseBeginRuntime: NativeCalling {
  private let suspended: @Sendable () -> Void
  private var first = true
  private var continuation: CheckedContinuation<Void, Never>?
  private(set) var owner: String?

  init(suspended: @escaping @Sendable () -> Void) { self.suspended = suspended }
  func open(_ input: Data, key: Data) async throws -> Wire {
    let result = try await NativeRuntime.shared.open(input, key: key)
    owner = result.string
    return result
  }
  func call(_ input: Data) async throws -> Wire {
    let result = try await NativeRuntime.shared.call(input)
    if try Wire.decode(input).object?["op"] == .string("begin"), result == .boolean(false), first {
      first = false
      await withCheckedContinuation {
        continuation = $0
        suspended()
      }
    }
    return result
  }
  func resume() {
    continuation?.resume()
    continuation = nil
  }
}

final class AdmissionTests: XCTestCase, @unchecked Sendable {
  func testForeignReleaseBeforeSuspendedFalseBeginResumesWithoutPolling() async throws {
    let schema = Schema(version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
    let held = expectation(description: "actual begin false reply held")
    let admitted = expectation(
      description: "queued Swift command resumes after foreign bus release")
    let runtime = HeldFalseBeginRuntime(suspended: { held.fulfill() })
    let db = Database(runtime: runtime)
    try await db.openMemory(schema: schema, namespace: "96969696969696969696969696969696")
    // ADR-0037: independent native owners no longer share FIFO. Exercise the foreign
    // transport release on this same logical owner, without a second store owner.
    let capturedOwner = await runtime.owner
    let foreignOwner = try XCTUnwrap(capturedOwner)
    let ticket = try await NativeRuntime.shared.call(
      Wire.frame("request", owner: foreignOwner, args: [:]).encoded())
    _ = try await NativeRuntime.shared.call(
      Wire.frame(
        "begin", owner: foreignOwner,
        args: [
          "request": ticket, "definition": CommandDefinition("foreign-v1").wire,
          "value": JSONValue.object([:]).wire,
          "context": CapturedContext(identity: .null, nowMilliseconds: 1000).wire,
        ]
      ).encoded())
    let task = Task {
      try await db.command(
        CommandDefinition("waiting-v1"), value: .object([:]),
        context: .init(identity: .null, nowMilliseconds: 1000)
      ) { _ in
        admitted.fulfill()
        return .null
      }
    }
    await fulfillment(of: [held], timeout: 2)
    // A native/RN transport cooperates with the same Apple notification after
    // its native call has returned; it has no Swift Database.execute wake path.
    _ = try await NativeRuntime.shared.call(
      Wire.frame("abort", owner: foreignOwner, args: ["request": ticket]).encoded())
    NotificationCenter.default.post(name: NativeAdmission.notificationName, object: nil)
    await runtime.resume()
    await fulfillment(of: [admitted], timeout: 2)
    task.cancel()  // Ensures the RED fixture also retires an unawakened waiter.
    _ = try? await task.value
    try await db.close()
  }

  func testNativeTransportPublishesTerminalFailureAfterCallAndNoReadPulse() async throws {
    let terminal = expectation(description: "payload-free terminal-attempt notification")
    let observer = NotificationCenter.default.addObserver(
      forName: NativeAdmission.notificationName, object: nil, queue: nil
    ) { note in
      XCTAssertNil(note.object)
      XCTAssertNil(note.userInfo)
      terminal.fulfill()
    }
    _ = try? await NativeRuntime.shared.call(
      Wire.frame("abort", owner: "18446744073709551615", args: ["request": .string("1")]).encoded())
    await fulfillment(of: [terminal], timeout: 2)
    NotificationCenter.default.removeObserver(observer)
    let read = expectation(description: "no wake after read or begin false")
    read.isInverted = true
    let readObserver = NotificationCenter.default.addObserver(
      forName: NativeAdmission.notificationName, object: nil, queue: nil
    ) { _ in read.fulfill() }
    _ = try? await NativeRuntime.shared.call(
      Wire.frame("schema", owner: "18446744073709551615", args: [:]).encoded())
    await fulfillment(of: [read], timeout: 0.03)
    NotificationCenter.default.removeObserver(readObserver)
  }
}
