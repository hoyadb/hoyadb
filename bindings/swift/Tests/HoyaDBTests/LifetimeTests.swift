// API-0011 / TC-0036 G09/G10: real native handles, controlled host delivery only.
import Foundation
import XCTest

@testable import HoyaDB

private actor TrackingRuntime: NativeCalling {
  private let didClose: @Sendable () -> Void
  private(set) var handle: String?
  private(set) var opens = 0
  private(set) var closes = 0

  init(didClose: @escaping @Sendable () -> Void) {
    self.didClose = didClose
  }

  func open(_ input: Data, key: Data) async throws -> Wire {
    opens += 1
    let value = try await NativeRuntime.shared.open(input, key: key)
    handle = value.string
    return value
  }

  func call(_ input: Data) async throws -> Wire {
    let value = try await NativeRuntime.shared.call(input)
    if try Wire.decode(input).object?["op"] == .string("close") {
      closes += 1
      didClose()
    }
    return value
  }
}

final class LifetimeTests: XCTestCase, @unchecked Sendable {
  func testDroppingOwnerDisposesActualNativeHandle() async throws {
    let closed = expectation(description: "actual C ABI owner close")
    let runtime = TrackingRuntime(didClose: { closed.fulfill() })
    var db: Database? = Database(runtime: runtime)
    try await db!.openMemory(
      schema: Schema(
        version: 1, tables: ["wavelengths": Table(columns: ["label": Column(.string)])]),
      namespace: "56565656565656565656565656565656")
    let handle = await runtime.handle
    XCTAssertNotNil(handle)
    db = nil
    await fulfillment(of: [closed], timeout: 2)
    let closes = await runtime.closes
    XCTAssertEqual(closes, 1)
    if let handle {
      do {
        _ = try await NativeRuntime.shared.call(
          Wire.frame("schema", owner: handle, args: [:]).encoded())
        XCTFail("dropped Swift owner leaked a usable native handle")
      } catch {
        XCTAssertEqual(error as? HoyaError, .core("InvalidHandle"))
      }
    }
  }
}
