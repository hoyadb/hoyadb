// SPEC-0036 item 5: Apple protected-store backup policy claim. The filesystem and Foundation
// calls go through an internal seam so tests can inject faults; production uses Darwin.
import Darwin
import Foundation

struct FileInfo: Equatable, Sendable {
  let device: UInt64
  let inode: UInt64
  let mode: UInt16
  let links: UInt16
  let owner: UInt32
  let size: Int64

  var isDirectory: Bool { mode & S_IFMT == S_IFDIR }
  var isRegular: Bool { mode & S_IFMT == S_IFREG }

  init(device: UInt64, inode: UInt64, mode: UInt16, links: UInt16, owner: UInt32, size: Int64) {
    self.device = device
    self.inode = inode
    self.mode = mode
    self.links = links
    self.owner = owner
    self.size = size
  }

  init(_ value: stat) {
    self.init(
      device: UInt64(UInt32(bitPattern: value.st_dev)), inode: value.st_ino, mode: value.st_mode,
      links: value.st_nlink, owner: value.st_uid, size: value.st_size)
  }
}

/// Internal filesystem seam for the backup policy claim. Every method reports failure instead
/// of throwing; `errno`-style codes are returned where the algorithm branches on them.
protocol BackupPolicyIO: Sendable {
  func lstatInfo(_ path: String) -> FileInfo?
  func openDirectory(_ path: String) -> Int32
  func fstatInfo(_ descriptor: Int32) -> FileInfo?
  func fstatatInfo(_ directory: Int32, _ name: String) -> FileInfo?
  /// openat(O_RDWR|O_CREAT|O_NOFOLLOW|O_CLOEXEC|O_EXLOCK|O_NONBLOCK, 0600): descriptor or -errno.
  func openClaim(_ directory: Int32, _ name: String) -> Int32
  func read(_ descriptor: Int32, maximum: Int) -> [UInt8]?
  /// Whether the directory has any entry other than `ignoring`; nil when listing fails.
  func hasOtherEntries(_ directory: Int32, ignoring name: String) -> Bool?
  func excludedFlag(_ path: String) -> Bool?
  func setExcludedFlag(_ path: String, _ excluded: Bool) -> Bool
  func truncate(_ descriptor: Int32) -> Bool
  /// Bytes written, or -1 when nothing could be written.
  func write(_ descriptor: Int32, _ bytes: [UInt8]) -> Int
  func sync(_ descriptor: Int32) -> Bool
  func close(_ descriptor: Int32)
  func milliseconds() -> UInt64
  func pause(milliseconds: UInt32) async
}

struct DarwinBackupPolicyIO: BackupPolicyIO {
  func lstatInfo(_ path: String) -> FileInfo? {
    var value = stat()
    return Darwin.lstat(path, &value) == 0 ? FileInfo(value) : nil
  }

  func openDirectory(_ path: String) -> Int32 {
    Darwin.open(path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
  }

  func fstatInfo(_ descriptor: Int32) -> FileInfo? {
    var value = stat()
    return Darwin.fstat(descriptor, &value) == 0 ? FileInfo(value) : nil
  }

  func fstatatInfo(_ directory: Int32, _ name: String) -> FileInfo? {
    var value = stat()
    return Darwin.fstatat(directory, name, &value, AT_SYMLINK_NOFOLLOW) == 0 ? FileInfo(value) : nil
  }

  func openClaim(_ directory: Int32, _ name: String) -> Int32 {
    let descriptor = Darwin.openat(
      directory, name, O_RDWR | O_CREAT | O_NOFOLLOW | O_CLOEXEC | O_EXLOCK | O_NONBLOCK, 0o600)
    return descriptor >= 0 ? descriptor : -errno
  }

  func read(_ descriptor: Int32, maximum: Int) -> [UInt8]? {
    var buffer = [UInt8](repeating: 0, count: maximum)
    let count = buffer.withUnsafeMutableBytes {
      Darwin.pread(descriptor, $0.baseAddress, maximum, 0)
    }
    return count >= 0 ? Array(buffer.prefix(count)) : nil
  }

  func hasOtherEntries(_ directory: Int32, ignoring name: String) -> Bool? {
    // fdopendir takes ownership of its descriptor, so list through a duplicate.
    let duplicate = Darwin.dup(directory)
    guard duplicate >= 0 else { return nil }
    guard let stream = Darwin.fdopendir(duplicate) else {
      Darwin.close(duplicate)
      return nil
    }
    defer { Darwin.closedir(stream) }
    Darwin.rewinddir(stream)
    return Self.scanEntries(ignoring: name) {
      guard let entry = Darwin.readdir(stream) else { return nil }
      return withUnsafeBytes(of: entry.pointee.d_name) { raw in
        String(decoding: raw.prefix(Int(entry.pointee.d_namlen)), as: UTF8.self)
      }
    }
  }

  /// The readdir loop over `next`, which returns an entry name or nil like readdir(3): nil with
  /// errno still 0 ends the listing, nil with errno set is a listing error.
  static func scanEntries(ignoring name: String, next: () -> String?) -> Bool? {
    while true {
      errno = 0
      guard let entryName = next() else { return errno == 0 ? false : nil }
      if entryName != "." && entryName != ".." && entryName != name { return true }
    }
  }

  func excludedFlag(_ path: String) -> Bool? {
    try? URL(fileURLWithPath: path, isDirectory: true).resourceValues(forKeys: [
      .isExcludedFromBackupKey
    ]).isExcludedFromBackup
  }

  func setExcludedFlag(_ path: String, _ excluded: Bool) -> Bool {
    var url = URL(fileURLWithPath: path, isDirectory: true)
    var values = URLResourceValues()
    values.isExcludedFromBackup = excluded
    return (try? url.setResourceValues(values)) != nil
  }

  func truncate(_ descriptor: Int32) -> Bool { Darwin.ftruncate(descriptor, 0) == 0 }

  func write(_ descriptor: Int32, _ bytes: [UInt8]) -> Int {
    bytes.withUnsafeBytes { Darwin.pwrite(descriptor, $0.baseAddress, bytes.count, 0) }
  }

  func sync(_ descriptor: Int32) -> Bool { Darwin.fsync(descriptor) == 0 }

  func close(_ descriptor: Int32) { Darwin.close(descriptor) }

  func milliseconds() -> UInt64 { DispatchTime.now().uptimeNanoseconds / 1_000_000 }

  func pause(milliseconds: UInt32) async {
    try? await Task.sleep(nanoseconds: UInt64(milliseconds) * 1_000_000)
  }
}

/// SPEC-0036 item 5: `.hoyadb-backup-policy` in the validated directory reserves one backup flag
/// for every store there. The claim is held only while deciding and is released before the
/// key-bearing native open, which stays path-based. Nothing is unlinked, renamed or re-truncated.
enum BackupPolicyClaim {
  static let name = ".hoyadb-backup-policy"
  static let acquisitionMilliseconds: UInt64 = 2_000
  private static let excludedBytes = Array("excluded\n".utf8)
  private static let includedBytes = Array("included\n".utf8)

  /// Throws `.busy` when the claim stays locked past the acquisition bound and
  /// `.backupPolicyFailure` for every rejection or initialization failure (SPEC-0036 item 7).
  static func claim(
    _ path: String, directory validated: FileInfo, excluded: Bool, io: any BackupPolicyIO
  ) async throws {
    try Task.checkCancellation()
    let directory = io.openDirectory(path)
    guard directory >= 0 else { throw HoyaError.backupPolicyFailure }
    defer { io.close(directory) }
    guard let opened = io.fstatInfo(directory), opened.isDirectory, same(opened, validated) else {
      throw HoyaError.backupPolicyFailure
    }
    let claim = try await acquire(directory, io)
    defer { io.close(claim) }
    guard let held = io.fstatInfo(claim), held.isRegular, held.links == 1,
      held.owner == geteuid(), held.size <= 16,
      let named = io.fstatatInfo(directory, name), same(named, held),
      let contents = io.read(claim, maximum: 17), contents.count <= 16
    else { throw HoyaError.backupPolicyFailure }
    let requested = excluded ? excludedBytes : includedBytes
    // Re-validates the path before each Foundation flag access, which is path-based.
    func flag() throws -> Bool? {
      guard let current = io.lstatInfo(path), same(current, opened) else {
        throw HoyaError.backupPolicyFailure
      }
      return io.excludedFlag(path)
    }
    if !contents.isEmpty {
      guard contents == requested, try flag() == excluded else {
        throw HoyaError.backupPolicyFailure
      }
      return
    }
    guard let others = io.hasOtherEntries(directory, ignoring: name) else {
      throw HoyaError.backupPolicyFailure
    }
    // A cancelled attempt never begins initialization (SPEC-0036 SW-L4).
    try Task.checkCancellation()
    if !others {
      guard let current = io.lstatInfo(path), same(current, opened),
        io.setExcludedFlag(path, excluded)
      else { throw HoyaError.backupPolicyFailure }
    }
    // A legacy directory is reserved only when its existing flag already matches.
    guard try flag() == excluded, io.truncate(claim),
      io.write(claim, requested) == requested.count, io.sync(claim)
    else { throw HoyaError.backupPolicyFailure }
  }

  private static func acquire(_ directory: Int32, _ io: any BackupPolicyIO) async throws -> Int32 {
    let deadline = io.milliseconds() + acquisitionMilliseconds
    while true {
      let descriptor = io.openClaim(directory, name)
      if descriptor >= 0 { return descriptor }
      guard descriptor == -EWOULDBLOCK else { throw HoyaError.backupPolicyFailure }
      guard io.milliseconds() < deadline else { throw HoyaError.busy }
      await io.pause(milliseconds: 10)
      try Task.checkCancellation()
    }
  }

  private static func same(_ left: FileInfo, _ right: FileInfo) -> Bool {
    left.device == right.device && left.inode == right.inode
  }
}
