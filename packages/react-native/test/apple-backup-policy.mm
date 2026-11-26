// TC-0040 BK01-BK04, BK06, BK07 (threads and processes), BK09, BK-F1..BK-F7 and the RN
// invalidation analogs of SW-L1/SW-L4/SW-L5 (RN-L1), RN-L2 and RN-L3 (SPEC-0036 items 5 and 7)
// for the RN iOS host on
// macOS: real Darwin claims, flags and locks in task-owned directories, a fake C ABI for native
// replies and an injectable fault seam. Same fixture table as BackupPolicyTests.swift. No iOS claim.
#ifndef HOYA_HARNESS
#error "compile the harness with -DHOYA_HARNESS=1"
#endif
#import "../ios/HoyaBackupPolicy.h"
#import <hoyadb.h>
#include <algorithm>
#include <atomic>
#include <condition_variable>
#include <cstdio>
#include <cstdlib>
#include <fcntl.h>
#include <map>
#include <mutex>
#include <poll.h>
#include <set>
#include <spawn.h>
#include <string>
#include <sys/stat.h>
#include <sys/wait.h>
#include <thread>
#include <unistd.h>

extern char **environ;

// Fake C ABI: preflight accepts, open replies with owner "1" or a core failure. Each call
// records whether it ran on a harness module queue.
static std::atomic<int> nativeOpens{0};
static std::atomic<int> nativeCloses{0};
static std::atomic<bool> nativeFails{false};
static int moduleQueueKey;
static std::mutex abiLock;
static std::vector<std::pair<std::string, bool>> abiEvents;
static std::function<void()> onNativeOpen;
static void record(const char *name) {
  std::lock_guard<std::mutex> guard(abiLock);
  abiEvents.push_back({name, dispatch_get_specific(&moduleQueueKey) != nullptr});
}
static int32_t reply(const char *text, hoya_v1_reply collector, void *context) {
  collector(context, reinterpret_cast<const uint8_t *>(text), strlen(text));
  return 0;
}
extern "C" uint32_t hoya_v1_abi_version(void) { return 1; }
extern "C" int32_t hoya_v1_validate_open(const uint8_t *, size_t, size_t, hoya_v1_reply r, void *c) {
  record("validate");
  return reply("{\"abi\":\"1\",\"ok\":true,\"value\":null}", r, c);
}
extern "C" int32_t hoya_v1_open(const uint8_t *, size_t, const uint8_t *, size_t, hoya_v1_reply r,
                                void *c) {
  record("open");
  nativeOpens++;
  if (onNativeOpen) onNativeOpen();
  if (nativeFails) return reply("{\"abi\":\"1\",\"ok\":false,\"error\":{\"kind\":\"BackendFailure\"}}", r, c);
  return reply("{\"abi\":\"1\",\"ok\":true,\"value\":\"1\"}", r, c);
}
extern "C" int32_t hoya_v1_call(const uint8_t *json, size_t length, hoya_v1_reply r, void *c) {
  record("call");
  if (std::string(reinterpret_cast<const char *>(json), length).find("\"op\":\"close\"") != std::string::npos) nativeCloses++;
  return reply("{\"abi\":\"1\",\"ok\":true,\"value\":true}", r, c);
}

static int failures = 0;
static std::string current;
static void check(bool value, const std::string &message) {
  if (!value) {
    failures++;
    fprintf(stderr, "FAIL %s: %s\n", current.c_str(), message.c_str());
  }
}

enum class Fault { none, setFlag, readbackMismatch, truncate, writeNothing, writePartial, sync, foreignOwner, listing };

struct FaultIO : HoyaBackupPolicyIO {
  std::mutex lock;
  std::map<std::string, int> counts;
  std::vector<std::string> events;
  Fault fault = Fault::none;
  size_t partial = 0;
  std::function<void(const std::string &)> afterOpenDirectory;
  std::function<void()> afterOpenClaim, duringListing, afterSync, duringPause;
  std::function<bool()> duringSetFlag;
  std::function<uint64_t()> clock;

  void count(const std::string &name) {
    std::lock_guard<std::mutex> guard(lock);
    counts[name]++;
  }
  int calls(const std::string &name) {
    std::lock_guard<std::mutex> guard(lock);
    return counts[name];
  }
  int total() {
    std::lock_guard<std::mutex> guard(lock);
    int sum = 0;
    for (auto &entry : counts) sum += entry.second;
    return sum;
  }
  std::optional<HoyaFileInfo> lstatInfo(const std::string &path) override {
    count("lstat");
    return HoyaBackupPolicyIO::lstatInfo(path);
  }
  void event(const std::string &name) {
    std::lock_guard<std::mutex> guard(lock);
    events.push_back(name);
  }
  std::vector<std::string> log() {
    std::lock_guard<std::mutex> guard(lock);
    return events;
  }
  int openDirectory(const std::string &path) override {
    count("openDirectory");
    event("openDirectory");
    int descriptor = HoyaBackupPolicyIO::openDirectory(path);
    if (afterOpenDirectory) afterOpenDirectory(path);
    return descriptor;
  }
  std::optional<HoyaFileInfo> fstatInfo(int descriptor) override {
    count("fstat");
    auto value = HoyaBackupPolicyIO::fstatInfo(descriptor);
    if (value && fault == Fault::foreignOwner && value->regular()) value->owner += 1;
    return value;
  }
  std::optional<HoyaFileInfo> fstatatInfo(int directory, const char *name) override {
    count("fstatat");
    return HoyaBackupPolicyIO::fstatatInfo(directory, name);
  }
  int openClaim(int directory, const char *name) override {
    count("openClaim");
    int descriptor = HoyaBackupPolicyIO::openClaim(directory, name);
    if (descriptor >= 0 && afterOpenClaim) afterOpenClaim();
    return descriptor;
  }
  std::optional<std::vector<uint8_t>> read(int descriptor, size_t maximum) override {
    count("read");
    return HoyaBackupPolicyIO::read(descriptor, maximum);
  }
  std::optional<bool> hasOtherEntries(int directory, const char *name) override {
    count("list");
    if (duringListing) duringListing();
    if (fault == Fault::listing) return std::nullopt;
    return HoyaBackupPolicyIO::hasOtherEntries(directory, name);
  }
  std::optional<bool> excludedFlag(const std::string &path) override {
    count("readFlag");
    auto value = HoyaBackupPolicyIO::excludedFlag(path);
    if (value && fault == Fault::readbackMismatch) return !*value;
    return value;
  }
  bool setExcludedFlag(const std::string &path, bool excluded) override {
    count("setFlag");
    if (duringSetFlag) return duringSetFlag();
    if (fault == Fault::setFlag) return false;
    return HoyaBackupPolicyIO::setExcludedFlag(path, excluded);
  }
  bool truncate(int descriptor) override {
    count("truncate");
    if (fault == Fault::truncate) return false;
    return HoyaBackupPolicyIO::truncate(descriptor);
  }
  ssize_t write(int descriptor, const std::vector<uint8_t> &bytes) override {
    count("write");
    if (fault == Fault::writeNothing) return -1;
    if (fault == Fault::writePartial)
      return HoyaBackupPolicyIO::write(descriptor, std::vector<uint8_t>(bytes.begin(), bytes.begin() + partial));
    return HoyaBackupPolicyIO::write(descriptor, bytes);
  }
  bool sync(int descriptor) override {
    count("sync");
    if (fault == Fault::sync) return false;
    bool result = HoyaBackupPolicyIO::sync(descriptor);
    event("sync");
    if (afterSync) afterSync();
    return result;
  }
  uint64_t milliseconds() override { return clock ? clock() : HoyaBackupPolicyIO::milliseconds(); }
  void pause(uint32_t milliseconds) override {
    if (duringPause) duringPause();
    if (!clock) HoyaBackupPolicyIO::pause(milliseconds);
  }
};

static std::string base;
static int serial = 0;
static const char *claimName = ".hoyadb-backup-policy";

struct Directory {
  std::string parent, path;
  explicit Directory(const std::string &owned) : parent(owned), path(owned + "/hoyadb") {}
  Directory(bool create = true) {
    parent = base + "/case-" + std::to_string(++serial);
    mkdir(parent.c_str(), 0700);
    path = parent + "/hoyadb";
    if (create) mkdir(path.c_str(), 0700);
  }
  std::string claimPath() const { return path + "/" + claimName; }
  std::optional<std::string> claim() const {
    NSString *text = [NSString stringWithContentsOfFile:@(claimPath().c_str()) encoding:NSUTF8StringEncoding error:nil];
    return text ? std::optional<std::string>(text.UTF8String) : std::nullopt;
  }
  std::optional<bool> flag() const {
    NSNumber *value = nil;
    if (![[NSURL fileURLWithPath:@(path.c_str()) isDirectory:YES] getResourceValue:&value forKey:NSURLIsExcludedFromBackupKey error:nil] || !value)
      return std::nullopt;
    return static_cast<bool>(value.boolValue);
  }
  void setFlag(bool value) const {
    check([[NSURL fileURLWithPath:@(path.c_str()) isDirectory:YES] setResourceValue:@(value) forKey:NSURLIsExcludedFromBackupKey error:nil], "fixture flag");
  }
  std::set<std::string> entries() const {
    std::set<std::string> result;
    for (NSString *name in [NSFileManager.defaultManager contentsOfDirectoryAtPath:@(path.c_str()) error:nil]) result.insert(name.UTF8String);
    return result;
  }
  void write(const std::string &name, const std::string &text) const {
    [NSFileManager.defaultManager createFileAtPath:@((path + "/" + name).c_str()) contents:[NSData dataWithBytes:text.data() length:text.size()] attributes:nil];
  }
  std::string read(const std::string &name) const {
    NSString *text = [NSString stringWithContentsOfFile:@((path + "/" + name).c_str()) encoding:NSUTF8StringEncoding error:nil];
    return text ? text.UTF8String : "<missing>";
  }
  ino_t inode(const std::string &name) const {
    struct stat value;
    return lstat((path + "/" + name).c_str(), &value) == 0 ? value.st_ino : 0;
  }
  void truncateClaimForManualRecovery() const { ::truncate(claimPath().c_str(), 0); }
};

static NSArray *key(NSUInteger count = 32) {
  NSMutableArray *bytes = [NSMutableArray array];
  for (NSUInteger i = 0; i < count; i++) [bytes addObject:@(7)];
  return bytes;
}

static NSString *frame(const std::string &name, bool excluded) {
  NSDictionary *value = @{@"abi":@"1",@"op":@"open",@"owner":[NSNull null],@"args":@{
    @"engine":@{@"kind":@"native",@"directory":@"@hoyadb/default",@"name":@(name.c_str()),@"backup_excluded":@(excluded)},
    @"schema":@{@"version":@"1",@"tables":@{@"notes":@{@"columns":@{@"body":@{@"kind":@"String",@"optional":@NO}},@"relations":@{}}}},
    @"migrations":@[],@"namespace":@"40404040404040404040404040400030",
    @"options":@{@"capacity":@"1000",@"history":@"100",@"initial_delay_ms":@"1000",@"multiplier":@"2",@"max_delay_ms":@"60000"}}};
  return [[NSString alloc] initWithData:[NSJSONSerialization dataWithJSONObject:value options:0 error:nil] encoding:NSUTF8StringEncoding];
}

/// "opened", "native:<kind>" for a core failure reply, or the thrown host kind.
static std::string open(const Directory &d, const std::string &name, bool excluded, FaultIO &io,
                        HoyaOpenCancelled cancelled = nil, NSArray *secret = key()) {
  std::string parent = d.parent;
  @try {
    NSString *text = HoyaHostOpenWithPolicyIO(frame(name, excluded), secret, ^NSString *(BOOL) { return @(parent.c_str()); }, cancelled, io);
    NSDictionary *value = [NSJSONSerialization JSONObjectWithData:[text dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
    if ([value[@"ok"] boolValue]) return "opened";
    return std::string("native:") + [value[@"error"][@"kind"] UTF8String];
  } @catch (NSException *error) {
    return error.name.UTF8String;
  }
}

static void begin(const char *name) {
  current = name;
  nativeOpens = 0;
  nativeCloses = 0;
  nativeFails = false;
  onNativeOpen = nullptr;
  std::lock_guard<std::mutex> guard(abiLock);
  abiEvents.clear();
}

static void bk01() {
  for (bool first : {true, false}) {
    begin("BK01 opposite policies in one directory");
    Directory d;
    FaultIO io;
    check(open(d, "a.sqlite", first, io) == "opened", "first opens");
    d.write("a.sqlite", "store a");
    ino_t inode = d.inode("a.sqlite");
    check(open(d, "b.sqlite", !first, io) == "BackupPolicyFailure", "second rejected");
    check(d.flag() == first, "flag unchanged");
    check(d.claim() == std::string(first ? "excluded\n" : "included\n"), "reservation");
    check(d.inode("a.sqlite") == inode && d.read("a.sqlite") == "store a", "store a untouched");
    check(d.entries() == std::set<std::string>{"a.sqlite", claimName}, "no b store");
    check(nativeOpens == 1, "no native open for the rejected store");
  }
}

static void bk02() {
  begin("BK02 reopen with changed flag");
  Directory d;
  FaultIO io;
  check(open(d, "a.sqlite", true, io) == "opened", "first");
  d.write("a.sqlite", "store a");
  check(open(d, "a.sqlite", false, io) == "BackupPolicyFailure", "changed flag rejected");
  check(d.flag() == true, "flag kept");
  check(d.entries() == std::set<std::string>{"a.sqlite", claimName}, "entries");
  check(open(d, "a.sqlite", true, io) == "opened", "original reopens");
  check(nativeOpens == 2, "two native opens");
}

static void bk03() {
  for (bool request : {true, false}) {
    begin("BK03 legacy directory recorded or rejected");
    Directory d;
    d.setFlag(true);
    d.write("legacy.sqlite", "legacy");
    ino_t inode = d.inode("legacy.sqlite");
    FaultIO io;
    if (request) {
      check(open(d, "legacy.sqlite", true, io) == "opened", "matching legacy opens");
      check(d.claim() == std::string("excluded\n"), "recorded");
    } else {
      check(open(d, "legacy.sqlite", false, io) == "BackupPolicyFailure", "mismatch rejected");
      check(d.claim() == std::string(""), "only the empty claim");
    }
    check(io.calls("setFlag") == 0, "a legacy flag is never written");
    check(d.flag() == true, "flag");
    check(d.inode("legacy.sqlite") == inode && d.read("legacy.sqlite") == "legacy", "legacy untouched");
  }
}

static void bk04() {
  std::vector<std::pair<std::string, std::function<void(Directory &, FaultIO &)>>> cases = {
    {"symlinked claim", [](Directory &d, FaultIO &) { d.write("target", ""); symlink((d.path + "/target").c_str(), d.claimPath().c_str()); }},
    {"hard-linked claim", [](Directory &d, FaultIO &) { d.write("other", ""); link((d.path + "/other").c_str(), d.claimPath().c_str()); }},
    {"oversized claim", [](Directory &d, FaultIO &) { d.write(claimName, std::string(17, 'x')); }},
    {"garbled claim", [](Directory &d, FaultIO &) { d.write(claimName, "bogus\n"); }},
    {"foreign-owned claim", [](Directory &, FaultIO &io) { io.fault = Fault::foreignOwner; }},
    {"directory swapped after validation", [](Directory &, FaultIO &io) {
      io.afterOpenDirectory = [](const std::string &path) { rename(path.c_str(), (path + ".old").c_str()); mkdir(path.c_str(), 0700); };
    }},
    {"claim replaced while held", [](Directory &d, FaultIO &io) {
      std::string path = d.claimPath();
      io.afterOpenClaim = [path]() { rename(path.c_str(), (path + ".old").c_str()); close(::open(path.c_str(), O_CREAT | O_WRONLY, 0600)); };
    }},
  };
  for (auto &entry : cases) {
    begin(("BK04 " + entry.first).c_str());
    Directory d;
    FaultIO io;
    auto before = d.flag();
    entry.second(d, io);
    check(open(d, "a.sqlite", true, io) == "BackupPolicyFailure", "rejected");
    check(io.calls("setFlag") == 0 && io.calls("write") == 0, "no flag or reservation write");
    check(d.flag() == before, "flag unchanged");
    check(nativeOpens == 0, "no native open");
  }
}

static void preflight() {
  begin("BK04 preflight before any filesystem action");
  Directory d(false);
  FaultIO io;
  check(open(d, "a.sqlite", true, io, nil, key(31)) == "InvalidRequest", "wrong key length");
  check(open(d, "nested/a.sqlite", true, io) == "InvalidRequest", "multi-component name");
  check(io.total() == 0, "no seam call");
  check(access(d.path.c_str(), F_OK) != 0, "directory not created");
}

static int lockfStatus(const std::string &path) {
  pid_t pid;
  const char *arguments[] = {"/usr/bin/lockf", "-s", "-k", "-n", "-t", "0", path.c_str(), "/usr/bin/true", nullptr};
  if (posix_spawn(&pid, arguments[0], nullptr, nullptr, const_cast<char **>(arguments), environ) != 0) return -1;
  int status = 0;
  waitpid(pid, &status, 0);
  return WIFEXITED(status) ? WEXITSTATUS(status) : -1;
}

static void bk06() {
  begin("BK06 primitive characterization");
  Directory d;
  HoyaBackupPolicyIO io;
  int directory = io.openDirectory(d.path);
  int holder = io.openClaim(directory, claimName);
  check(holder >= 0, "held");
  check(io.openClaim(directory, claimName) == -EWOULDBLOCK, "same process EWOULDBLOCK");
  check(lockfStatus(d.claimPath()) == 75, "child EX_TEMPFAIL while held");
  io.close(holder);
  int second = io.openClaim(directory, claimName);
  check(second >= 0, "acquired after release");
  io.close(second);
  check(lockfStatus(d.claimPath()) == 0, "child acquires after release");
  io.close(directory);
}

static void busy() {
  begin("bounded acquisition yields Busy");
  Directory d;
  HoyaBackupPolicyIO real;
  int directory = real.openDirectory(d.path);
  int holder = real.openClaim(directory, claimName);
  FaultIO io;
  std::atomic<uint64_t> ticks{0};
  io.clock = [&ticks]() { return ticks += 500; };
  auto before = d.flag();
  check(open(d, "a.sqlite", true, io) == "Busy", "busy");
  check(io.calls("openClaim") > 1, "retried");
  check(io.calls("setFlag") == 0 && d.flag() == before && d.claim() == std::string(""), "no writes");
  real.close(holder);
  real.close(directory);
}

static void zeroByte() {
  for (Fault fault : {Fault::setFlag, Fault::readbackMismatch, Fault::truncate, Fault::writeNothing}) {
    begin(("BK-F1..F4a zero-byte failure " + std::to_string(static_cast<int>(fault))).c_str());
    Directory d;
    FaultIO io;
    io.fault = fault;
    check(open(d, "a.sqlite", true, io) == "BackupPolicyFailure", "fails closed");
    check(d.claim() == std::string(""), "claim empty");
    check(d.entries() == std::set<std::string>{claimName}, "no store");
    check(nativeOpens == 0, "no native open");
    io.fault = Fault::none;
    check(open(d, "a.sqlite", false, io) == "opened", "retry reserves");
    check(d.claim() == std::string("included\n") && d.flag() == false, "requested flag");
  }
}

static void partial() {
  begin("BK-F4b partial prefix");
  Directory d;
  FaultIO io;
  io.fault = Fault::writePartial;
  io.partial = 3;
  check(open(d, "a.sqlite", true, io) == "BackupPolicyFailure", "partial fails");
  io.fault = Fault::none;
  check(d.claim() == std::string("exc"), "prefix");
  for (bool excluded : {true, false}) check(open(d, "a.sqlite", excluded, io) == "BackupPolicyFailure", "fails closed");
  check(d.entries() == std::set<std::string>{claimName}, "no store");
  d.truncateClaimForManualRecovery();
  check(open(d, "a.sqlite", false, io) == "opened", "manual recovery");
  check(d.claim() == std::string("included\n") && d.flag() == false, "reserved");
}

static void fsyncFailure() {
  begin("BK-F5 fsync failure");
  Directory d;
  FaultIO io;
  io.fault = Fault::sync;
  check(open(d, "a.sqlite", true, io) == "BackupPolicyFailure", "fails closed");
  check(nativeOpens == 0, "no native open");
  io.fault = Fault::none;
  check(d.claim() == std::string("excluded\n"), "complete reservation");
  check(open(d, "a.sqlite", false, io) == "BackupPolicyFailure", "opposite rejected");
  check(open(d, "a.sqlite", true, io) == "opened", "same proceeds");
}

static void legacyWrites() {
  for (size_t prefix : {static_cast<size_t>(0), static_cast<size_t>(4)}) {
    begin(prefix == 0 ? "BK-F6a legacy zero-byte write" : "BK-F6b legacy partial write");
    Directory d;
    d.setFlag(true);
    d.write("legacy.sqlite", "legacy");
    ino_t inode = d.inode("legacy.sqlite");
    FaultIO io;
    io.fault = prefix == 0 ? Fault::writeNothing : Fault::writePartial;
    io.partial = prefix;
    check(open(d, "legacy.sqlite", true, io) == "BackupPolicyFailure", "write fails");
    io.fault = Fault::none;
    check(d.flag() == true, "flag kept");
    check(open(d, "legacy.sqlite", false, io) == "BackupPolicyFailure", "differing rejected");
    if (prefix == 0) {
      check(d.claim() == std::string(""), "empty");
    } else {
      check(d.claim() == std::string("excl"), "partial");
      check(open(d, "legacy.sqlite", true, io) == "BackupPolicyFailure", "partial fails closed");
      d.truncateClaimForManualRecovery();
    }
    check(open(d, "legacy.sqlite", true, io) == "opened", "matching legacy");
    check(d.claim() == std::string("excluded\n"), "reserved");
    check(io.calls("setFlag") == 0 && d.flag() == true, "flag never written");
    check(d.inode("legacy.sqlite") == inode && d.read("legacy.sqlite") == "legacy", "legacy untouched");
  }
}

static void listing() {
  begin("BK-F7 scan error versus end (ported primitive)");
  check(!HoyaScanEntries(claimName, []() -> const char *_Nullable { errno = EIO; return nullptr; }).has_value(), "error before any entry");
  std::vector<const char *> names = {".", "..", claimName};
  size_t index = 0;
  check(!HoyaScanEntries(claimName, [&]() -> const char *_Nullable {
    if (index == names.size()) { errno = EIO; return nullptr; }
    return names[index++];
  }).has_value(), "error before an existing store entry");
  errno = EBADF;
  check(HoyaScanEntries(claimName, []() -> const char *_Nullable { return nullptr; }) == false, "stale errno at end");
  index = 0;
  errno = EBADF;
  check(HoyaScanEntries(claimName, [&]() -> const char *_Nullable { return index < names.size() ? names[index++] : nullptr; }) == false, "true end");
  std::vector<const char *> store = {".", "store.sqlite"};
  index = 0;
  check(HoyaScanEntries(claimName, [&]() -> const char *_Nullable { return index < store.size() ? store[index++] : nullptr; }) == true, "entry");
  begin("BK-F7 listing failure rejects without writes");
  Directory d;
  FaultIO io;
  io.fault = Fault::listing;
  auto before = d.flag();
  check(open(d, "a.sqlite", true, io) == "BackupPolicyFailure", "rejected");
  check(io.calls("setFlag") == 0 && io.calls("write") == 0 && d.flag() == before, "no writes");
  check(nativeOpens == 0, "no native open");
}

static void bk09() {
  begin("BK09 reservation survives native-open failure");
  Directory d;
  FaultIO io;
  nativeFails = true;
  check(open(d, "a.sqlite", true, io) == "native:BackendFailure", "native failure reply");
  nativeFails = false;
  check(d.claim() == std::string("excluded\n") && d.flag() == true, "reservation kept");
  check(open(d, "a.sqlite", false, io) == "BackupPolicyFailure", "opposite rejected");
  check(open(d, "a.sqlite", true, io) == "opened", "same proceeds");
}

struct Barrier {
  std::mutex lock;
  std::condition_variable ready;
  int waiting;
  explicit Barrier(int parties) : waiting(parties) {}
  void arrive() {
    std::unique_lock<std::mutex> guard(lock);
    if (--waiting <= 0) ready.notify_all();
    ready.wait_for(guard, std::chrono::seconds(2), [this] { return waiting <= 0; });
  }
};

static void bk07Threads() {
  begin("BK07 threads");
  Directory d;
  FaultIO io;
  Barrier barrier(2);
  io.afterOpenDirectory = [&barrier](const std::string &) { barrier.arrive(); };
  std::string results[2];
  std::thread first([&] { results[0] = open(d, "a.sqlite", true, io); });
  std::thread second([&] { results[1] = open(d, "b.sqlite", false, io); });
  first.join();
  second.join();
  int opened = (results[0] == "opened") + (results[1] == "opened");
  check(opened == 1, "exactly one reserves: " + results[0] + "/" + results[1]);
  for (auto &result : results)
    check(result == "opened" || result == "BackupPolicyFailure" || result == "Busy", "loser kind " + result);
  auto reserved = d.claim();
  check(reserved == std::string("excluded\n") || reserved == std::string("included\n"), "reservation");
  check(d.flag() == (reserved == std::string("excluded\n")), "flag agrees");
}

static int claimant(const char *parent, bool excluded, int ready, int go) {
  Directory d{std::string(parent)};
  FaultIO io;
  io.afterOpenDirectory = [ready, go](const std::string &) {
    char byte = 'r';
    (void)::write(ready, &byte, 1);
    (void)::read(go, &byte, 1);
  };
  std::string result = open(d, excluded ? "a.sqlite" : "b.sqlite", excluded, io);
  if (result == "opened") return 0;
  if (result == "BackupPolicyFailure") return 10;
  if (result == "Busy") return 11;
  return 12;
}

static void bk07Processes(const char *self) {
  begin("BK07 processes");
  Directory d;
  int ready[2], go[2];
  check(pipe(ready) == 0 && pipe(go) == 0, "pipes");
  std::string readyText = std::to_string(ready[1]), goText = std::to_string(go[0]);
  pid_t pids[2];
  for (int i = 0; i < 2; i++) {
    const char *arguments[] = {self, "--claimant", d.parent.c_str(), i == 0 ? "1" : "0", readyText.c_str(), goText.c_str(), nullptr};
    check(posix_spawn(&pids[i], self, nullptr, nullptr, const_cast<char **>(arguments), environ) == 0, "spawn");
  }
  int arrived = 0;
  while (arrived < 2) {
    struct pollfd entry = {ready[0], POLLIN, 0};
    if (poll(&entry, 1, 5000) <= 0) break;
    char byte;
    if (::read(ready[0], &byte, 1) == 1) arrived++;
  }
  check(arrived == 2, "both claimants passed directory validation");
  char release[2] = {'g', 'g'};
  (void)::write(go[1], release, 2);
  int codes[2];
  for (int i = 0; i < 2; i++) {
    int status = 0;
    waitpid(pids[i], &status, 0);
    codes[i] = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
  }
  for (int descriptor : {ready[0], ready[1], go[0], go[1]}) close(descriptor);
  printf("BK07 processes exit codes %d/%d\n", codes[0], codes[1]);
  check((codes[0] == 0) + (codes[1] == 0) == 1,
        "exactly one reserves: " + std::to_string(codes[0]) + "/" + std::to_string(codes[1]));
  for (int code : codes) check(code == 0 || code == 10 || code == 11, "loser kind " + std::to_string(code));
  auto reserved = d.claim();
  check(reserved == std::string(codes[0] == 0 ? "excluded\n" : "included\n"), "winner's reservation");
  check(d.flag() == (reserved == std::string("excluded\n")), "flag agrees");
}

static HoyaOpenCancelled cancellation(std::shared_ptr<std::atomic<bool>> flag) {
  return ^BOOL { return flag->load(); };
}

static void invalidatedDuringRetry() {
  begin("SW-L1 analog: invalidation during claim-lock retry");
  Directory d;
  d.setFlag(true);
  d.write(claimName, "excluded\n");
  HoyaBackupPolicyIO real;
  int directory = real.openDirectory(d.path);
  int holder = real.openClaim(directory, claimName);
  FaultIO io;
  auto cancelled = std::make_shared<std::atomic<bool>>(false);
  io.duringPause = [cancelled, &real, &holder]() {
    cancelled->store(true);
    if (holder >= 0) { real.close(holder); holder = -1; }
  };
  check(open(d, "a.sqlite", true, io, cancellation(cancelled)) == "Closed", "closed");
  check(nativeOpens == 0, "no key-bearing open after invalidation");
  check(d.claim() == std::string("excluded\n") && d.flag() == true, "claim retained");
  real.close(directory);
}

static void cancelledBeforeEntry() {
  begin("SW-L4 analog: invalidated before claim entry");
  Directory d;
  FaultIO io;
  auto cancelled = std::make_shared<std::atomic<bool>>(true);
  auto before = d.flag();
  check(open(d, "a.sqlite", true, io, cancellation(cancelled)) == "Closed", "closed");
  check(io.calls("openDirectory") == 0 && io.calls("setFlag") == 0, "claim not entered");
  check(d.entries().empty() && d.flag() == before && nativeOpens == 0, "nothing touched");
}

static void cancelledWhileListing() {
  begin("SW-L4 analog: invalidated while listing");
  Directory d;
  FaultIO io;
  auto cancelled = std::make_shared<std::atomic<bool>>(false);
  io.duringListing = [cancelled]() { cancelled->store(true); };
  auto before = d.flag();
  check(open(d, "a.sqlite", true, io, cancellation(cancelled)) == "Closed", "closed");
  check(io.calls("setFlag") == 0 && io.calls("truncate") == 0 && io.calls("write") == 0, "no mutation");
  check(d.claim() == std::string("") && d.flag() == before && nativeOpens == 0, "nothing written");
}

static void cancelledBeforeSubmission() {
  begin("SW-L1 analog: invalidated between claim and native submission");
  Directory d;
  FaultIO io;
  auto cancelled = std::make_shared<std::atomic<bool>>(false);
  io.afterSync = [cancelled]() { cancelled->store(true); };
  check(open(d, "a.sqlite", true, io, cancellation(cancelled)) == "Closed", "closed");
  check(nativeOpens == 0, "no native submission");
  check(d.claim() == std::string("excluded\n") && d.flag() == true, "written reservation retained");
}

static void flagFailureWhileCancelled() {
  begin("SW-L5 analog: claim failure during invalidation reports Closed");
  Directory d;
  FaultIO io;
  auto cancelled = std::make_shared<std::atomic<bool>>(false);
  std::atomic<bool> ran{false};
  io.duringSetFlag = [cancelled, &ran]() { cancelled->store(true); ran = true; return false; };
  check(open(d, "a.sqlite", true, io, cancellation(cancelled)) == "Closed", "closed");
  check(ran.load(), "held flag syscall ran");
  check(io.calls("write") == 0 && nativeOpens == 0, "no write or native open");
}

static dispatch_queue_t moduleQueue() {
  dispatch_queue_t queue = dispatch_queue_create("harness.module", DISPATCH_QUEUE_SERIAL);
  dispatch_queue_set_specific(queue, &moduleQueueKey, &moduleQueueKey, nullptr);
  return queue;
}

static HoyaModuleHost *moduleHost(const Directory &d, std::shared_ptr<FaultIO> io, dispatch_queue_t queue) {
  std::string parent = d.parent;
  return [[HoyaModuleHost alloc] initWithParent:^NSString *(BOOL) { return @(parent.c_str()); } io:io queue:queue];
}

/// "ok" or "native:<kind>" for a reply, the error kind, or "timeout" after 10 s.
static std::string settle(void (^start)(HoyaHostCompletion)) {
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  __block NSString *result = nil;
  start(^(NSString *reply, NSString *kind) {
    if (reply) {
      NSDictionary *value = [NSJSONSerialization JSONObjectWithData:[reply dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
      result = [value[@"ok"] boolValue] ? @"ok" : [@"native:" stringByAppendingString:value[@"error"][@"kind"]];
    } else {
      result = kind;
    }
    dispatch_semaphore_signal(done);
  });
  if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC)) != 0) return "timeout";
  return result.UTF8String;
}

static std::string hostOpen(HoyaModuleHost *host, const std::string &name, bool excluded, NSArray *secret = key()) {
  NSString *input = frame(name, excluded);
  return settle(^(HoyaHostCompletion completion) { [host open:input key:secret completion:completion]; });
}

static NSString *callFrame() {
  return @"{\"abi\":\"1\",\"op\":\"schema\",\"owner\":\"1\",\"args\":{}}";
}

static void drain(dispatch_queue_t queue) { dispatch_sync(queue, ^{}); }

static void moduleHostCharacterization() {
  begin("RN module host open, close and invalidation (extraction characterization)");
  Directory d;
  auto io = std::make_shared<FaultIO>();
  dispatch_queue_t queue = moduleQueue();
  HoyaModuleHost *host = moduleHost(d, io, queue);
  check(hostOpen(host, "a.sqlite", true) == "ok", "opens");
  check(d.claim() == std::string("excluded\n"), "reserved");
  check(settle(^(HoyaHostCompletion completion) { [host call:callFrame() completion:completion]; }) == "ok", "call");
  [host invalidate];
  drain(queue);
  check(nativeCloses == 1, "invalidation closes the registered owner");
  check(hostOpen(host, "a.sqlite", true) == "Closed", "open after invalidation");
  check(settle(^(HoyaHostCompletion completion) { [host call:callFrame() completion:completion]; }) == "Closed", "call after invalidation");
  check(nativeOpens == 1, "one native open");
}

static void rnL2() {
  begin("RN-L2 a contended claim does not block other module calls");
  Directory d;
  HoyaBackupPolicyIO real;
  int directory = real.openDirectory(d.path);
  int holder = real.openClaim(directory, claimName);
  auto io = std::make_shared<FaultIO>();
  dispatch_queue_t queue = moduleQueue();
  HoyaModuleHost *host = moduleHost(d, io, queue);
  std::atomic<bool> fired{false}, callInTime{false};
  auto callReachedAbi = std::make_shared<std::atomic<bool>>(false);
  auto second = std::make_shared<std::string>("unsettled");
  dispatch_semaphore_t secondDone = dispatch_semaphore_create(0);
  io->duringPause = [&]() {
    if (fired.exchange(true)) return;
    dispatch_semaphore_t called = dispatch_semaphore_create(0);
    [host call:callFrame() completion:^(NSString *reply, NSString *) {
      {
        std::lock_guard<std::mutex> guard(abiLock);
        for (auto &entry : abiEvents) if (entry.first == "call") callReachedAbi->store(true);
      }
      if (reply) dispatch_semaphore_signal(called);
    }];
    [host open:frame("b.sqlite", true) key:key() completion:^(NSString *reply, NSString *kind) {
      *second = reply ? "ok" : kind.UTF8String;
      dispatch_semaphore_signal(secondDone);
    }];
    // Deadlock watchdog, not a latency target: the claim lock is still held here.
    callInTime = dispatch_semaphore_wait(called, dispatch_time(DISPATCH_TIME_NOW, 500 * NSEC_PER_MSEC)) == 0;
    real.close(holder);
    holder = -1;
  };
  check(hostOpen(host, "a.sqlite", true) == "ok", "first open reserves after release");
  check(fired.load(), "the open waited on the held claim");
  check(callInTime.load() && callReachedAbi->load(), "queued call reached the C ABI and returned before the claim was released");
  check(dispatch_semaphore_wait(secondDone, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC)) == 0, "second open settles");
  check(*second == "ok", "second open is not Busy: " + *second);
  auto log = io->log();
  auto firstSync = std::find(log.begin(), log.end(), "sync");
  auto secondDirectory = std::find(firstSync == log.end() ? log.end() : firstSync, log.end(), "openDirectory");
  check(firstSync != log.end() && secondDirectory != log.end(), "second policy phase starts after the first finished");
  check(d.claim() == std::string("excluded\n"), "reservation");
  drain(queue);
  std::lock_guard<std::mutex> guard(abiLock);
  for (auto &entry : abiEvents)
    check(entry.first == "validate" || entry.second, "only read-only preflights run off the module queue: " + entry.first);
  check(nativeOpens == 2, "both key-bearing opens");
  if (holder >= 0) real.close(holder);
  real.close(directory);
}

static std::mutex wipeLock;
static std::vector<std::pair<size_t, bool>> wipes;
static void observeWipe(size_t length, bool zero) {
  std::lock_guard<std::mutex> guard(wipeLock);
  wipes.push_back({length, zero});
}

static void rnL3BeforeAdmission() {
  begin("RN-L3 invalidation before the queued submit block's admission");
  {
    std::lock_guard<std::mutex> guard(wipeLock);
    wipes.clear();
  }
  HoyaHarnessWipeObserver = observeWipe;
  Directory d;
  auto io = std::make_shared<FaultIO>();
  dispatch_queue_t queue = moduleQueue();
  HoyaModuleHost *host = moduleHost(d, io, queue);
  dispatch_semaphore_t enqueued = dispatch_semaphore_create(0);
  HoyaHarnessSubmitEnqueued = ^{ dispatch_semaphore_signal(enqueued); };
  dispatch_suspend(queue);
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  __block NSString *result = nil;
  [host open:frame("a.sqlite", true) key:key() completion:^(NSString *reply, NSString *kind) {
    result = reply ? @"ok" : kind;
    dispatch_semaphore_signal(done);
  }];
  bool queued = dispatch_semaphore_wait(enqueued, dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC)) == 0;
  check(queued, "submit block was queued behind the suspended module queue");
  [host invalidate];
  dispatch_resume(queue);
  check(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC)) == 0, "settles");
  HoyaHarnessSubmitEnqueued = nil;
  check(result && [result isEqual:@"Closed"], "Closed");
  check(nativeOpens == 0, "no key-bearing native open");
  check(d.claim() == std::string("excluded\n"), "reservation retained");
  // Added after GREEN: the zeroization table's invalidation-before-admission path.
  HoyaHarnessWipeObserver = nullptr;
  std::lock_guard<std::mutex> guard(wipeLock);
  check(wipes.size() == 1 && wipes[0].first == 32 && wipes[0].second, "submit block wiped the prepared secret once");
}

static void rnL3AfterAdmission() {
  begin("RN-L3 invalidation after admission (existing post-reply cleanup, characterization)");
  Directory d;
  auto io = std::make_shared<FaultIO>();
  dispatch_queue_t queue = moduleQueue();
  HoyaModuleHost *host = moduleHost(d, io, queue);
  onNativeOpen = [host]() { [host invalidate]; };
  check(hostOpen(host, "a.sqlite", true) == "Closed", "Closed");
  drain(queue);
  check(nativeOpens == 1, "admitted open completed");
  check(nativeCloses == 1, "late owner closed exactly once and never registered");
}

static void zeroization() {
  HoyaHarnessWipeObserver = observeWipe;
  struct Path { const char *name; std::function<std::string(Directory &, std::shared_ptr<FaultIO>, HoyaModuleHost *, dispatch_queue_t)> run; std::string expected; };
  std::vector<Path> paths = {
    {"preflight rejection", [](Directory &, std::shared_ptr<FaultIO>, HoyaModuleHost *host, dispatch_queue_t) { return hostOpen(host, "nested/a.sqlite", true); }, "InvalidRequest"},
    {"policy rejection", [](Directory &d, std::shared_ptr<FaultIO>, HoyaModuleHost *host, dispatch_queue_t) { d.write(claimName, "included\n"); return hostOpen(host, "a.sqlite", true); }, "BackupPolicyFailure"},
    {"Busy", [](Directory &d, std::shared_ptr<FaultIO> io, HoyaModuleHost *host, dispatch_queue_t) {
      HoyaBackupPolicyIO real;
      int directory = real.openDirectory(d.path), holder = real.openClaim(directory, claimName);
      auto ticks = std::make_shared<std::atomic<uint64_t>>(0);
      io->clock = [ticks]() { return *ticks += 500; };
      std::string result = hostOpen(host, "a.sqlite", true);
      real.close(holder);
      real.close(directory);
      return result;
    }, "Busy"},
    {"invalidation after admission", [](Directory &, std::shared_ptr<FaultIO>, HoyaModuleHost *host, dispatch_queue_t) {
      onNativeOpen = [host]() { [host invalidate]; };
      return hostOpen(host, "a.sqlite", true);
    }, "Closed"},
    {"failed native open", [](Directory &, std::shared_ptr<FaultIO>, HoyaModuleHost *host, dispatch_queue_t) { nativeFails = true; return hostOpen(host, "a.sqlite", true); }, "native:BackendFailure"},
    {"success", [](Directory &, std::shared_ptr<FaultIO>, HoyaModuleHost *host, dispatch_queue_t) { return hostOpen(host, "a.sqlite", true); }, "ok"},
  };
  for (auto &path : paths) {
    begin((std::string("RN-L3 prepared secret zeroized: ") + path.name).c_str());
    {
      std::lock_guard<std::mutex> guard(wipeLock);
      wipes.clear();
    }
    Directory d;
    auto io = std::make_shared<FaultIO>();
    dispatch_queue_t queue = moduleQueue();
    HoyaModuleHost *host = moduleHost(d, io, queue);
    check(path.run(d, io, host, queue) == path.expected, "outcome");
    drain(queue);
    std::lock_guard<std::mutex> guard(wipeLock);
    check(!wipes.empty(), "wiped");
    for (auto &wipe : wipes) check(wipe.first == 32 && wipe.second, "every wipe leaves 32 zero bytes");
  }
  HoyaHarnessWipeObserver = nullptr;
}

int main(int argc, char **argv) {
  @autoreleasepool {
    if (argc == 6 && strcmp(argv[1], "--claimant") == 0)
      return claimant(argv[2], strcmp(argv[3], "1") == 0, atoi(argv[4]), atoi(argv[5]));
    if (argc != 2) {
      fprintf(stderr, "usage: %s <owned canonical directory>\n", argv[0]);
      return 2;
    }
    char resolved[PATH_MAX];
    if (!realpath(argv[1], resolved)) return 2;
    base = resolved;
    bk01(); bk02(); bk03(); bk04(); preflight(); bk06(); busy(); zeroByte(); partial();
    fsyncFailure(); legacyWrites(); listing(); bk09(); bk07Threads(); bk07Processes(argv[0]);
    invalidatedDuringRetry(); cancelledBeforeEntry(); cancelledWhileListing();
    cancelledBeforeSubmission(); flagFailureWhileCancelled();
    moduleHostCharacterization(); rnL2(); rnL3BeforeAdmission(); rnL3AfterAdmission(); zeroization();
    if (failures) {
      fprintf(stderr, "HOYA_RN_APPLE_BACKUP_POLICY_FAILED %d\n", failures);
      return 1;
    }
    puts("HOYA_RN_APPLE_BACKUP_POLICY_GREEN");
    return 0;
  }
}
