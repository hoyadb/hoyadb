// API-0011 §7: shared post-C-ABI payload-free Apple release notification.
import Foundation

enum NativeAdmission {
  static let notificationName = Notification.Name("dev.hoyadb.admissionReleased.v1")

  static func notifyAfterCall(_ input: Data) {
    guard let frame = try? Wire.decode(input), let operation = frame.object?["op"]?.string,
      ["finish", "abort", "cancel_request", "close", "reset"].contains(operation)
    else { return }
    NotificationCenter.default.post(name: notificationName, object: nil)
  }
}
