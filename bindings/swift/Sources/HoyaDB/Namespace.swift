// API-0011: platform namespace entropy; no key derivation or cipher fallback.
import Foundation
import Security

enum NamespaceDefaults {
  static func jitter() throws -> UInt16 {
    try jitter {
      var bytes = [UInt8](repeating: 0, count: 2)
      guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
        throw HoyaError.core("EntropyUnavailable")
      }
      return UInt16(bytes[0]) << 8 | UInt16(bytes[1])
    }
  }

  static func jitter(sample: () throws -> UInt16) throws -> UInt16 {
    while true {
      let value = try sample()
      // 26 complete buckets cover 0..<65026; reject the remainder so all
      // 2501 permitted outcomes have exactly the same number of preimages.
      if value < 65026 { return value % 2501 }
    }
  }

  static func generate() throws -> String {
    var bytes = [UInt8](repeating: 0, count: 16)
    guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
      throw HoyaError.core("EntropyUnavailable")
    }
    let alphabet = Array("0123456789abcdef".utf8)
    var encoded = [UInt8]()
    encoded.reserveCapacity(32)
    for byte in bytes {
      encoded.append(alphabet[Int(byte >> 4)])
      encoded.append(alphabet[Int(byte & 15)])
    }
    return String(decoding: encoded, as: UTF8.self)
  }
}
