// TC-0040 BK01-BK04, BK06, BK07 (threads), BK09, BK-F1..BK-F7 and SW-L1..SW-L5 (SPEC-0036 items 5
// and 7) for
// the Swift SDK on macOS: real Darwin claims, flags and locks in task-owned temporary directories,
// with a fake NativeCalling for open replies and an injectable fault seam. No iOS claim.
import Darwin
import Foundation
import XCTest

@testable import HoyaDB

private actor OpenRuntime: NativeCalling {
  private(set) var opens = 0
  var failOpen = false

  func setFailOpen(_ value: Bool) { failOpen = value }

  func open(_ input: Data, key: Data) async throws -> Wire {
    opens += 1
    if failOpen { throw HoyaError.core("BackendFailure") }
    return .string("1")
  }

  func call(_ input: Data) async throws -> Wire {
    guard try Wire.decode(input).object?["op"]?.string == "protection" else { return .null }
    return .object([
      "encrypted": .boolean(true), "cipher": .string("SQLCipher"), "version": .string("fake"),
      "backup": .string("host_managed"),
    ])
  }
}

private enum Fault: Sendable, Equatable {
  case setFlag, readbackMismatch, truncate, writeNothing
  case writePartial(Int)
  case sync, foreignOwner, listing
}

private final class FaultIO: BackupPolicyIO, @unchecked Sendable {
  private let real = DarwinBackupPolicyIO()
  private let lock = NSLock()
  private var fault: Fault?
  private var counts: [String: Int] = [:]
  var afterOpenDirectory: (@Sendable (String) -> Void)?
  var afterOpenClaim: (@Sendable (Int32) -> Void)?
  var duringListing: (@Sendable () -> Void)?
  var duringSetFlag: (@Sendable () -> Bool)?
  var clock: (@Sendable () -> UInt64)?

  func inject(_ fault: Fault?) {
    lock.lock()
    self.fault = fault
    lock.unlock()
  }

  private func active(_ candidate: Fault) -> Bool {
    lock.lock()
    defer { lock.unlock() }
    return fault == candidate
  }

  private func count(_ name: String) {
    lock.lock()
    counts[name, default: 0] += 1
    lock.unlock()
  }

  func calls(_ name: String) -> Int {
    lock.lock()
    defer { lock.unlock() }
    return counts[name, default: 0]
  }

  var totalCalls: Int {
    lock.lock()
    defer { lock.unlock() }
    return counts.values.reduce(0, +)
  }

  func lstatInfo(_ path: String) -> FileInfo? {
    count("lstat")
    return real.lstatInfo(path)
  }

  func openDirectory(_ path: String) -> Int32 {
    count("openDirectory")
    let descriptor = real.openDirectory(path)
    afterOpenDirectory?(path)
    return descriptor
  }

  func fstatInfo(_ descriptor: Int32) -> FileInfo? {
    count("fstat")
    guard let info = real.fstatInfo(descriptor) else { return nil }
    if active(.foreignOwner), info.isRegular {
      return FileInfo(
        device: info.device, inode: info.inode, mode: info.mode, links: info.links,
        owner: info.owner &+ 1, size: info.size)
    }
    return info
  }

  func fstatatInfo(_ directory: Int32, _ name: String) -> FileInfo? {
    count("fstatat")
    return real.fstatatInfo(directory, name)
  }

  func openClaim(_ directory: Int32, _ name: String) -> Int32 {
    count("openClaim")
    let descriptor = real.openClaim(directory, name)
    if descriptor >= 0 { afterOpenClaim?(directory) }
    return descriptor
  }

  func read(_ descriptor: Int32, maximum: Int) -> [UInt8]? {
    count("read")
    return real.read(descriptor, maximum: maximum)
  }

  func hasOtherEntries(_ directory: Int32, ignoring name: String) -> Bool? {
    count("list")
    duringListing?()
    if active(.listing) { return nil }
    return real.hasOtherEntries(directory, ignoring: name)
  }

  func excludedFlag(_ path: String) -> Bool? {
    count("readFlag")
    guard let value = real.excludedFlag(path) else { return nil }
    return active(.readbackMismatch) ? !value : value
  }

  func setExcludedFlag(_ path: String, _ excluded: Bool) -> Bool {
    count("setFlag")
    if let duringSetFlag { return duringSetFlag() }
    if active(.setFlag) { return false }
    return real.setExcludedFlag(path, excluded)
  }

  func truncate(_ descriptor: Int32) -> Bool {
    count("truncate")
    if active(.truncate) { return false }
    return real.truncate(descriptor)
  }

  func write(_ descriptor: Int32, _ bytes: [UInt8]) -> Int {
    count("write")
    if active(.writeNothing) { return -1 }
    lock.lock()
    let current = fault
    lock.unlock()
    if case .writePartial(let prefix) = current {
      return real.write(descriptor, Array(bytes.prefix(prefix)))
    }
    return real.write(descriptor, bytes)
  }

  func sync(_ descriptor: Int32) -> Bool {
    count("sync")
    if active(.sync) { return false }
    return real.sync(descriptor)
  }

  func close(_ descriptor: Int32) { real.close(descriptor) }

  func milliseconds() -> UInt64 { clock?() ?? real.milliseconds() }

  func pause(milliseconds: UInt32) async {
    if clock == nil { await real.pause(milliseconds: milliseconds) }
  }
}

private final class Directory: @unchecked Sendable {
  let url: URL
  var path: String { url.path }

  init() throws {
    let raw = FileManager.default.temporaryDirectory.appendingPathComponent(
      "hoya-backup-policy-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: raw, withIntermediateDirectories: false)
    guard let canonical = realpath(raw.path, nil) else { throw HoyaError.invalidRequest }
    url = URL(fileURLWithPath: String(cString: canonical), isDirectory: true)
    free(canonical)
  }

  var claimPath: String { path + "/.hoyadb-backup-policy" }
  var claim: String? { try? String(contentsOfFile: claimPath, encoding: .utf8) }
  var flag: Bool? {
    try? url.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup
  }
  var entries: Set<String> {
    Set((try? FileManager.default.contentsOfDirectory(atPath: path)) ?? [])
  }

  func setFlag(_ value: Bool) throws {
    var values = URLResourceValues()
    values.isExcludedFromBackup = value
    var target = url
    try target.setResourceValues(values)
  }

  func write(_ name: String, _ text: String) {
    FileManager.default.createFile(atPath: path + "/" + name, contents: Data(text.utf8))
  }

  func inode(_ name: String) -> UInt64? {
    var value = stat()
    return lstat(path + "/" + name, &value) == 0 ? value.st_ino : nil
  }

  func truncateClaimForManualRecovery() { Darwin.truncate(claimPath, 0) }

  deinit { try? FileManager.default.removeItem(at: url) }
}

private let schema = Schema(
  version: 1, tables: ["notes": Table(columns: ["body": Column(.string)])])
private let key = Data(repeating: 7, count: 32)

final class BackupPolicyTests: XCTestCase, @unchecked Sendable {
  private func open(
    _ directory: Directory, _ name: String, excluded: Bool, io: any BackupPolicyIO,
    runtime: OpenRuntime
  ) async throws {
    let db = Database(runtime: runtime, backupPolicyIO: io)
    try await db.openProtected(
      directory: directory.url, name: name, key: key, schema: schema,
      namespace: "40404040404040404040404040400020", backupExcluded: excluded)
    try await db.close()
  }

  private func rejects(
    _ expected: HoyaError, _ directory: Directory, _ name: String, excluded: Bool,
    io: any BackupPolicyIO, runtime: OpenRuntime, _ message: String
  ) async {
    do {
      try await open(directory, name, excluded: excluded, io: io, runtime: runtime)
      XCTFail("expected \(expected): \(message)")
    } catch {
      XCTAssertEqual(error as? HoyaError, expected, message)
    }
  }

  // BK01 in both orders.
  func testOppositePoliciesInOneDirectoryRejectTheSecondStoreWithoutChanges() async throws {
    for first in [true, false] {
      let d = try Directory()
      let runtime = OpenRuntime()
      let io = FaultIO()
      try await open(d, "a.sqlite", excluded: first, io: io, runtime: runtime)
      d.write("a.sqlite", "store a")
      let inode = d.inode("a.sqlite")
      await rejects(
        .backupPolicyFailure, d, "b.sqlite", excluded: !first, io: io, runtime: runtime, "BK01")
      XCTAssertEqual(d.flag, first)
      XCTAssertEqual(d.claim, first ? "excluded\n" : "included\n")
      XCTAssertEqual(d.inode("a.sqlite"), inode)
      XCTAssertEqual(try String(contentsOfFile: d.path + "/a.sqlite", encoding: .utf8), "store a")
      XCTAssertEqual(d.entries, ["a.sqlite", ".hoyadb-backup-policy"])
      let opens = await runtime.opens
      XCTAssertEqual(opens, 1, "no native open for the rejected store")
    }
  }

  // BK02.
  func testReopenWithChangedFlagIsRejectedAndTheOriginalReopens() async throws {
    let d = try Directory()
    let runtime = OpenRuntime()
    let io = FaultIO()
    try await open(d, "a.sqlite", excluded: true, io: io, runtime: runtime)
    d.write("a.sqlite", "store a")
    await rejects(
      .backupPolicyFailure, d, "a.sqlite", excluded: false, io: io, runtime: runtime, "BK02")
    XCTAssertEqual(d.flag, true)
    XCTAssertEqual(d.entries, ["a.sqlite", ".hoyadb-backup-policy"])
    try await open(d, "a.sqlite", excluded: true, io: io, runtime: runtime)
    let opens = await runtime.opens
    XCTAssertEqual(opens, 2)
  }

  // BK03: a legacy directory is recorded or rejected without a flag write.
  func testLegacyDirectoryIsRecordedOrRejectedWithoutFlagWrites() async throws {
    for request in [true, false] {
      let d = try Directory()
      try d.setFlag(true)
      d.write("legacy.sqlite", "legacy")
      let inode = d.inode("legacy.sqlite")
      let runtime = OpenRuntime()
      let io = FaultIO()
      if request {
        try await open(d, "legacy.sqlite", excluded: true, io: io, runtime: runtime)
        XCTAssertEqual(d.claim, "excluded\n")
      } else {
        await rejects(
          .backupPolicyFailure, d, "legacy.sqlite", excluded: false, io: io, runtime: runtime,
          "BK03 mismatch")
        XCTAssertEqual(d.claim, "", "only the empty claim was created")
      }
      XCTAssertEqual(io.calls("setFlag"), 0, "a legacy flag is never written")
      XCTAssertEqual(d.flag, true)
      XCTAssertEqual(d.inode("legacy.sqlite"), inode)
      XCTAssertEqual(
        try String(contentsOfFile: d.path + "/legacy.sqlite", encoding: .utf8), "legacy")
    }
  }

  // BK04: claim-file controls reject before any flag or reservation write.
  func testClaimFileControlsRejectBeforeAnyWrite() async throws {
    let cases: [(String, (Directory, FaultIO) throws -> Void)] = [
      (
        "symlinked claim",
        { d, _ in
          d.write("target", "")
          try FileManager.default.createSymbolicLink(
            atPath: d.claimPath, withDestinationPath: d.path + "/target")
        }
      ),
      (
        "hard-linked claim",
        { d, _ in
          d.write("other", "")
          try FileManager.default.linkItem(atPath: d.path + "/other", toPath: d.claimPath)
        }
      ),
      (
        "oversized claim",
        { d, _ in d.write(".hoyadb-backup-policy", String(repeating: "x", count: 17)) }
      ),
      ("garbled claim", { d, _ in d.write(".hoyadb-backup-policy", "bogus\n") }),
      ("foreign-owned claim", { _, io in io.inject(.foreignOwner) }),
      (
        "directory swapped after validation",
        { d, io in
          io.afterOpenDirectory = { path in
            _ = Darwin.rename(path, path + ".old")
            _ = Darwin.mkdir(path, 0o700)
          }
          _ = d
        }
      ),
      (
        "claim replaced while held",
        { d, io in
          let path = d.claimPath
          io.afterOpenClaim = { _ in
            _ = Darwin.rename(path, path + ".old")
            FileManager.default.createFile(atPath: path, contents: Data())
          }
        }
      ),
    ]
    for (label, arrange) in cases {
      let d = try Directory()
      let io = FaultIO()
      let runtime = OpenRuntime()
      let before = d.flag
      try arrange(d, io)
      await rejects(
        .backupPolicyFailure, d, "a.sqlite", excluded: true, io: io, runtime: runtime, label)
      XCTAssertEqual(io.calls("setFlag"), 0, label)
      XCTAssertEqual(io.calls("write"), 0, label)
      XCTAssertEqual(d.flag, before, label)
      let opens = await runtime.opens
      XCTAssertEqual(opens, 0, label)
      try? FileManager.default.removeItem(atPath: d.path + ".old")
    }
  }

  // Existing preflight: key, name and directory checks precede any filesystem action.
  func testPreflightRejectsBeforeAnyFilesystemAction() async throws {
    let d = try Directory()
    let io = FaultIO()
    let runtime = OpenRuntime()
    let db = Database(runtime: runtime, backupPolicyIO: io)
    for (directory, name, candidate) in [
      (d.url, "a.sqlite", Data(repeating: 1, count: 31)), (d.url, "nested/a.sqlite", key),
      (URL(fileURLWithPath: d.path + "/../" + d.url.lastPathComponent), "a.sqlite", key),
    ] {
      do {
        try await db.openProtected(
          directory: directory, name: name, key: candidate, schema: schema,
          namespace: "40404040404040404040404040400021")
        XCTFail("preflight must reject")
      } catch {
        XCTAssertEqual(error as? HoyaError, .invalidRequest)
      }
    }
    XCTAssertEqual(io.totalCalls, 0)
    XCTAssertEqual(d.entries, [])
  }

  // BK06 primitive characterization: the claim lock excludes a second independent open in the
  // same process and a child process (BSD flock locks via lockf -t 0), timing-free.
  func testClaimLockExcludesSameProcessAndChildUntilReleased() throws {
    let d = try Directory()
    let io = DarwinBackupPolicyIO()
    let directory = io.openDirectory(d.path)
    defer { io.close(directory) }
    let holder = io.openClaim(directory, ".hoyadb-backup-policy")
    XCTAssertGreaterThanOrEqual(holder, 0)
    XCTAssertEqual(io.openClaim(directory, ".hoyadb-backup-policy"), -EWOULDBLOCK)
    XCTAssertEqual(try lockfStatus(d.claimPath), 75, "EX_TEMPFAIL while held")
    io.close(holder)
    let second = io.openClaim(directory, ".hoyadb-backup-policy")
    XCTAssertGreaterThanOrEqual(second, 0)
    io.close(second)
    XCTAssertEqual(try lockfStatus(d.claimPath), 0, "acquired after release")
    XCTAssertTrue(FileManager.default.fileExists(atPath: d.claimPath), "lockf -k keeps the claim")
  }

  private func lockfStatus(_ path: String) throws -> Int32 {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/lockf")
    process.arguments = ["-s", "-k", "-n", "-t", "0", path, "/usr/bin/true"]
    try process.run()
    process.waitUntilExit()
    return process.terminationStatus
  }

  // Bounded acquisition: a held claim yields Busy after 2000 ms of retries, with no writes.
  func testHeldClaimYieldsBusyAfterBoundedRetriesWithoutWrites() async throws {
    let d = try Directory()
    let real = DarwinBackupPolicyIO()
    let directory = real.openDirectory(d.path)
    let holder = real.openClaim(directory, ".hoyadb-backup-policy")
    defer {
      real.close(holder)
      real.close(directory)
    }
    let io = FaultIO()
    let ticks = Ticks()
    io.clock = { ticks.next() }
    let runtime = OpenRuntime()
    let before = d.flag
    await rejects(.busy, d, "a.sqlite", excluded: true, io: io, runtime: runtime, "busy")
    XCTAssertGreaterThan(io.calls("openClaim"), 1, "retried")
    XCTAssertEqual(io.calls("setFlag"), 0)
    XCTAssertEqual(d.flag, before)
    XCTAssertEqual(d.claim, "")
  }

  // BK-F1, BK-F2, BK-F3, BK-F4a: zero-byte initialization failures leave an empty claim.
  func testZeroByteInitializationFailuresFailClosedAndRetry() async throws {
    for fault in [Fault.setFlag, .readbackMismatch, .truncate, .writeNothing] {
      let d = try Directory()
      let io = FaultIO()
      let runtime = OpenRuntime()
      io.inject(fault)
      await rejects(
        .backupPolicyFailure, d, "a.sqlite", excluded: true, io: io, runtime: runtime, "\(fault)")
      XCTAssertEqual(d.claim, "", "\(fault)")
      XCTAssertEqual(d.entries, [".hoyadb-backup-policy"], "\(fault): no store")
      let failedOpens = await runtime.opens
      XCTAssertEqual(failedOpens, 0, "\(fault): no native open")
      io.inject(nil)
      try await open(d, "a.sqlite", excluded: false, io: io, runtime: runtime)
      XCTAssertEqual(d.claim, "included\n", "\(fault): retry reserves the requested flag")
      XCTAssertEqual(d.flag, false)
    }
  }

  // BK-F4b: a partial prefix fails closed until separately authorized manual truncation.
  func testPartialReservationFailsClosedUntilManualTruncation() async throws {
    let d = try Directory()
    let io = FaultIO()
    let runtime = OpenRuntime()
    io.inject(.writePartial(3))
    await rejects(
      .backupPolicyFailure, d, "a.sqlite", excluded: true, io: io, runtime: runtime, "partial")
    io.inject(nil)
    XCTAssertEqual(d.claim, "exc")
    for excluded in [true, false] {
      await rejects(
        .backupPolicyFailure, d, "a.sqlite", excluded: excluded, io: io, runtime: runtime,
        "fails closed")
    }
    XCTAssertEqual(d.entries, [".hoyadb-backup-policy"])
    d.truncateClaimForManualRecovery()
    try await open(d, "a.sqlite", excluded: false, io: io, runtime: runtime)
    XCTAssertEqual(d.claim, "included\n")
    XCTAssertEqual(d.flag, false)
  }

  // BK-F5: an fsync failure after a complete write fails closed; the reservation then decides.
  func testFsyncFailureFailsClosedAndTheWrittenReservationDecides() async throws {
    let d = try Directory()
    let io = FaultIO()
    let runtime = OpenRuntime()
    io.inject(.sync)
    await rejects(
      .backupPolicyFailure, d, "a.sqlite", excluded: true, io: io, runtime: runtime, "fsync")
    let failedOpens = await runtime.opens
    XCTAssertEqual(failedOpens, 0)
    io.inject(nil)
    XCTAssertEqual(d.claim, "excluded\n")
    await rejects(
      .backupPolicyFailure, d, "a.sqlite", excluded: false, io: io, runtime: runtime, "opposite")
    try await open(d, "a.sqlite", excluded: true, io: io, runtime: runtime)
  }

  // BK-F6a / BK-F6b: legacy reservation write failures never change the legacy flag.
  func testLegacyReservationWriteFailuresKeepTheFlag() async throws {
    for fault in [Fault.writeNothing, .writePartial(4)] {
      let d = try Directory()
      try d.setFlag(true)
      d.write("legacy.sqlite", "legacy")
      let inode = d.inode("legacy.sqlite")
      let io = FaultIO()
      let runtime = OpenRuntime()
      io.inject(fault)
      await rejects(
        .backupPolicyFailure, d, "legacy.sqlite", excluded: true, io: io, runtime: runtime,
        "\(fault)")
      io.inject(nil)
      XCTAssertEqual(d.flag, true)
      await rejects(
        .backupPolicyFailure, d, "legacy.sqlite", excluded: false, io: io, runtime: runtime,
        "differing")
      if fault == .writeNothing {
        XCTAssertEqual(d.claim, "")
      } else {
        XCTAssertEqual(d.claim, "excl")
        await rejects(
          .backupPolicyFailure, d, "legacy.sqlite", excluded: true, io: io, runtime: runtime,
          "partial")
        d.truncateClaimForManualRecovery()
      }
      try await open(d, "legacy.sqlite", excluded: true, io: io, runtime: runtime)
      XCTAssertEqual(d.claim, "excluded\n")
      XCTAssertEqual(io.calls("setFlag"), 0)
      XCTAssertEqual(d.flag, true)
      XCTAssertEqual(d.inode("legacy.sqlite"), inode)
      XCTAssertEqual(
        try String(contentsOfFile: d.path + "/legacy.sqlite", encoding: .utf8), "legacy")
    }
  }

  // BK09: a reservation survives a failed native open.
  func testReservationSurvivesNativeOpenFailure() async throws {
    let d = try Directory()
    let io = FaultIO()
    let runtime = OpenRuntime()
    await runtime.setFailOpen(true)
    await rejects(
      .core("BackendFailure"), d, "a.sqlite", excluded: true, io: io, runtime: runtime, "native")
    await runtime.setFailOpen(false)
    XCTAssertEqual(d.claim, "excluded\n")
    XCTAssertEqual(d.flag, true)
    await rejects(
      .backupPolicyFailure, d, "a.sqlite", excluded: false, io: io, runtime: runtime, "opposite")
    try await open(d, "a.sqlite", excluded: true, io: io, runtime: runtime)
  }

  // BK07 (threads only): two concurrent opposite claims after directory validation.
  func testConcurrentOppositeClaimsInOneProcessReserveExactlyOne() async throws {
    let d = try Directory()
    let barrier = Barrier(parties: 2)
    let io = FaultIO()
    io.afterOpenDirectory = { _ in barrier.arrive() }
    let runtime = OpenRuntime()
    async let first: Result<Void, HoyaError> = attempt(
      d, "a.sqlite", excluded: true, io: io, runtime: runtime)
    async let second: Result<Void, HoyaError> = attempt(
      d, "b.sqlite", excluded: false, io: io, runtime: runtime)
    let results = await [first, second]
    let successes = results.filter { if case .success = $0 { return true } else { return false } }
    XCTAssertEqual(successes.count, 1)
    for case .failure(let error) in results {
      XCTAssertTrue(error == .backupPolicyFailure || error == .busy, "\(error)")
    }
    let reserved = d.claim
    XCTAssertTrue(reserved == "excluded\n" || reserved == "included\n")
    XCTAssertEqual(d.flag, reserved == "excluded\n")
  }

  // BK-F7 at the readdir iteration seam: NULL with errno set is a listing error, not the end.
  func testListingErrorIsDistinguishedFromEndOfListing() {
    let claim = ".hoyadb-backup-policy"
    XCTAssertNil(
      DarwinBackupPolicyIO.scanEntries(ignoring: claim) {
        errno = EIO
        return nil
      }, "error before any entry")
    var names = [".", "..", claim]
    XCTAssertNil(
      DarwinBackupPolicyIO.scanEntries(ignoring: claim) {
        if names.isEmpty {
          errno = EIO
          return nil
        }
        return names.removeFirst()
      }, "error before an existing store entry is returned")
    errno = EBADF
    XCTAssertEqual(DarwinBackupPolicyIO.scanEntries(ignoring: claim) { nil }, false, "stale errno")
    names = [".", "..", claim]
    errno = EBADF
    XCTAssertEqual(
      DarwinBackupPolicyIO.scanEntries(ignoring: claim) {
        names.isEmpty ? nil : names.removeFirst()
      },
      false, "true end of listing")
    names = [".", "store.sqlite"]
    XCTAssertEqual(
      DarwinBackupPolicyIO.scanEntries(ignoring: claim) {
        names.isEmpty ? nil : names.removeFirst()
      },
      true)
  }

  // BK-F7 policy step (characterization: the claim already rejects a failed listing).
  func testListingFailureRejectsWithoutWritesOrNativeOpen() async throws {
    let d = try Directory()
    let io = FaultIO()
    let runtime = OpenRuntime()
    io.inject(.listing)
    let before = d.flag
    await rejects(
      .backupPolicyFailure, d, "a.sqlite", excluded: true, io: io, runtime: runtime, "list")
    XCTAssertEqual(io.calls("setFlag"), 0)
    XCTAssertEqual(io.calls("write"), 0)
    XCTAssertEqual(d.flag, before)
    XCTAssertEqual(d.claim, "")
    let opens = await runtime.opens
    XCTAssertEqual(opens, 0)
  }

  /// Holds `d`'s claim lock so an open on it waits in claim-lock acquisition.
  private func hold(_ d: Directory) -> (release: () -> Void, io: DarwinBackupPolicyIO) {
    let real = DarwinBackupPolicyIO()
    let directory = real.openDirectory(d.path)
    let holder = real.openClaim(directory, ".hoyadb-backup-policy")
    XCTAssertGreaterThanOrEqual(holder, 0)
    var released = false
    return (
      {
        guard !released else { return }
        released = true
        real.close(holder)
        real.close(directory)
      }, real
    )
  }

  private func paused(_ db: Database, _ d: Directory, _ io: FaultIO) async -> Task<Void, any Error>
  {
    let attempt = Task {
      try await db.openProtected(
        directory: d.url, name: "a.sqlite", key: key, schema: schema,
        namespace: "40404040404040404040404040400022", backupExcluded: true)
    }
    for _ in 0..<500 where io.calls("openClaim") < 2 {
      try? await Task.sleep(nanoseconds: 2_000_000)
    }
    XCTAssertGreaterThanOrEqual(io.calls("openClaim"), 2, "attempt is waiting for the claim lock")
    return attempt
  }

  // SW-L1 and SW-L3.
  func testCloseDuringClaimWaitPreventsLateNativeOpenAndAllowsReopen() async throws {
    let d = try Directory()
    try d.setFlag(true)
    d.write(".hoyadb-backup-policy", "excluded\n")
    let held = hold(d)
    defer { held.release() }
    let io = FaultIO()
    let runtime = OpenRuntime()
    let db = Database(runtime: runtime, backupPolicyIO: io)
    let attempt = await paused(db, d, io)
    try await db.close()
    held.release()
    do {
      try await attempt.value
      XCTFail("the closed attempt opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .closed)
    }
    var opens = await runtime.opens
    XCTAssertEqual(opens, 0, "no key-bearing open after close")
    XCTAssertEqual(d.claim, "excluded\n")
    XCTAssertEqual(d.flag, true)
    do {
      _ = try await db.protection()
      XCTFail("no owner remains")
    } catch {
      XCTAssertEqual(error as? HoyaError, .closed)
    }
    try await db.openProtected(
      directory: d.url, name: "a.sqlite", key: key, schema: schema,
      namespace: "40404040404040404040404040400022", backupExcluded: true)
    opens = await runtime.opens
    XCTAssertEqual(opens, 1)
    let protection = try await db.protection()
    XCTAssertEqual(protection.verifiedBackupExcluded, true)
    try await db.close()
  }

  // SW-L2.
  func testSecondOpenDuringClaimWaitIsBusyWithoutTouchingItsDirectory() async throws {
    let d = try Directory()
    let held = hold(d)
    defer { held.release() }
    let io = FaultIO()
    let runtime = OpenRuntime()
    let db = Database(runtime: runtime, backupPolicyIO: io)
    let attempt = await paused(db, d, io)
    let other = try Directory()
    let before = other.flag
    do {
      try await db.openProtected(
        directory: other.url, name: "b.sqlite", key: key, schema: schema,
        namespace: "40404040404040404040404040400023", backupExcluded: false)
      XCTFail("second open on one instance proceeded")
    } catch {
      XCTAssertEqual(error as? HoyaError, .busy)
    }
    XCTAssertEqual(other.entries, [])
    XCTAssertEqual(other.flag, before)
    let opens = await runtime.opens
    XCTAssertEqual(opens, 0)
    try await db.close()
    held.release()
    _ = await attempt.result
  }

  // SW-L4: an attempt cancelled before the claim starts touches nothing.
  func testClaimCancelledBeforeEntryTouchesNothing() async throws {
    let d = try Directory()
    let io = FaultIO()
    let before = d.flag
    let path = d.path
    guard let validated = io.lstatInfo(path) else { return XCTFail("lstat") }
    let task = Task {
      withUnsafeCurrentTask { $0?.cancel() }
      try await BackupPolicyClaim.claim(path, directory: validated, excluded: true, io: io)
    }
    let result = await task.result
    XCTAssertThrowsError(try result.get()) { XCTAssertTrue($0 is CancellationError, "\($0)") }
    XCTAssertEqual(io.calls("openDirectory"), 0)
    XCTAssertEqual(io.calls("setFlag"), 0)
    XCTAssertEqual(d.entries, [])
    XCTAssertEqual(d.flag, before)
  }

  // SW-L4: close while the claim is listing; cancellation is observed before the first mutation.
  func testCloseWhileListingIsObservedBeforeAnyWrite() async throws {
    let d = try Directory()
    let io = FaultIO()
    let observed = Observed()
    io.duringListing = { observed.set(untilCancelled()) }
    let runtime = OpenRuntime()
    let db = Database(runtime: runtime, backupPolicyIO: io)
    let before = d.flag
    let attempt = Task {
      try await db.openProtected(
        directory: d.url, name: "a.sqlite", key: key, schema: schema,
        namespace: "40404040404040404040404040400024", backupExcluded: true)
    }
    for _ in 0..<500 where io.calls("list") < 1 { try? await Task.sleep(nanoseconds: 2_000_000) }
    try await db.close()
    await assertClosed(attempt)
    XCTAssertTrue(observed.value, "listing was held until close cancelled the attempt")
    XCTAssertEqual(io.calls("setFlag"), 0)
    XCTAssertEqual(io.calls("truncate"), 0)
    XCTAssertEqual(io.calls("write"), 0)
    XCTAssertEqual(d.claim, "")
    XCTAssertEqual(d.flag, before)
    let opens = await runtime.opens
    XCTAssertEqual(opens, 0)
  }

  // SW-L5 regression of the post-GREEN mapping (not initial RED): a non-cancellation flag
  // failure returned after close cancelled the attempt reports the closed attempt.
  func testFlagFailureAfterCancellationReportsClosed() async throws {
    let d = try Directory()
    let io = FaultIO()
    let observed = Observed()
    io.duringSetFlag = {
      observed.set(untilCancelled())
      return false
    }
    let runtime = OpenRuntime()
    let db = Database(runtime: runtime, backupPolicyIO: io)
    let attempt = Task {
      try await db.openProtected(
        directory: d.url, name: "a.sqlite", key: key, schema: schema,
        namespace: "40404040404040404040404040400025", backupExcluded: true)
    }
    for _ in 0..<500 where io.calls("setFlag") < 1 {
      try? await Task.sleep(nanoseconds: 2_000_000)
    }
    try await db.close()
    await assertClosed(attempt)
    XCTAssertTrue(observed.value, "the held flag syscall ran and returned after cancellation")
    XCTAssertEqual(io.calls("write"), 0)
    let opens = await runtime.opens
    XCTAssertEqual(opens, 0)
    do {
      _ = try await db.protection()
      XCTFail("no owner remains")
    } catch {
      XCTAssertEqual(error as? HoyaError, .closed)
    }
  }

  private func assertClosed(_ attempt: Task<Void, any Error>) async {
    do {
      try await attempt.value
      XCTFail("the closed attempt opened")
    } catch {
      XCTAssertEqual(error as? HoyaError, .closed, "\(error)")
    }
  }

  private func attempt(
    _ d: Directory, _ name: String, excluded: Bool, io: FaultIO, runtime: OpenRuntime
  ) async -> Result<Void, HoyaError> {
    do {
      try await open(d, name, excluded: excluded, io: io, runtime: runtime)
      return .success(())
    } catch {
      return .failure(error as? HoyaError ?? .boundaryFailure)
    }
  }
}

private final class Ticks: @unchecked Sendable {
  private let lock = NSLock()
  private var value: UInt64 = 0
  func next() -> UInt64 {
    lock.lock()
    defer { lock.unlock() }
    value += 500
    return value
  }
}

private final class Barrier: @unchecked Sendable {
  private let condition = NSCondition()
  private var waiting: Int
  init(parties: Int) { waiting = parties }
  func arrive() {
    condition.lock()
    waiting -= 1
    if waiting <= 0 { condition.broadcast() }
    let deadline = Date().addingTimeInterval(2)
    while waiting > 0 && condition.wait(until: deadline) {}
    condition.unlock()
  }
}

/// Blocks a seam call until the calling task is cancelled (bounded at 2 s).
private func untilCancelled() -> Bool {
  let deadline = Date().addingTimeInterval(2)
  while !Task.isCancelled && Date() < deadline { usleep(1_000) }
  return Task.isCancelled
}

private final class Observed: @unchecked Sendable {
  private let lock = NSLock()
  private var stored = false
  func set(_ value: Bool) {
    lock.lock()
    stored = value
    lock.unlock()
  }
  var value: Bool {
    lock.lock()
    defer { lock.unlock() }
    return stored
  }
}
