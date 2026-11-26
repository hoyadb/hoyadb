// SPEC-0036 item 5: RN iOS copy of the Swift SDK backup policy claim (BackupPolicy.swift).
#import "HoyaBackupPolicy.h"
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static HoyaFileInfo info(const struct stat &value) {
  return {static_cast<uint64_t>(static_cast<uint32_t>(value.st_dev)), value.st_ino, value.st_mode,
          value.st_nlink, value.st_uid, value.st_size};
}

bool HoyaFileInfo::directory() const { return S_ISDIR(mode); }
bool HoyaFileInfo::regular() const { return S_ISREG(mode); }

std::optional<HoyaFileInfo> HoyaBackupPolicyIO::lstatInfo(const std::string &path) {
  struct stat value;
  if (::lstat(path.c_str(), &value) != 0) return std::nullopt;
  return info(value);
}

int HoyaBackupPolicyIO::openDirectory(const std::string &path) {
  return ::open(path.c_str(), O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
}

std::optional<HoyaFileInfo> HoyaBackupPolicyIO::fstatInfo(int descriptor) {
  struct stat value;
  if (::fstat(descriptor, &value) != 0) return std::nullopt;
  return info(value);
}

std::optional<HoyaFileInfo> HoyaBackupPolicyIO::fstatatInfo(int directory, const char *name) {
  struct stat value;
  if (::fstatat(directory, name, &value, AT_SYMLINK_NOFOLLOW) != 0) return std::nullopt;
  return info(value);
}

int HoyaBackupPolicyIO::openClaim(int directory, const char *name) {
  int descriptor = ::openat(
      directory, name, O_RDWR | O_CREAT | O_NOFOLLOW | O_CLOEXEC | O_EXLOCK | O_NONBLOCK, 0600);
  return descriptor >= 0 ? descriptor : -errno;
}

std::optional<std::vector<uint8_t>> HoyaBackupPolicyIO::read(int descriptor, size_t maximum) {
  std::vector<uint8_t> buffer(maximum);
  ssize_t count = ::pread(descriptor, buffer.data(), maximum, 0);
  if (count < 0) return std::nullopt;
  buffer.resize(static_cast<size_t>(count));
  return buffer;
}

std::optional<bool> HoyaScanEntries(
    const char *name, const std::function<const char *_Nullable()> &next) {
  while (true) {
    errno = 0;
    const char *entry = next();
    if (!entry) {
      if (errno == 0) return false;
      return std::nullopt;
    }
    if (strcmp(entry, ".") != 0 && strcmp(entry, "..") != 0 && strcmp(entry, name) != 0)
      return true;
  }
}

std::optional<bool> HoyaBackupPolicyIO::hasOtherEntries(int directory, const char *name) {
  // fdopendir takes ownership of its descriptor, so list through a duplicate.
  int duplicate = ::dup(directory);
  if (duplicate < 0) return std::nullopt;
  DIR *stream = ::fdopendir(duplicate);
  if (!stream) {
    ::close(duplicate);
    return std::nullopt;
  }
  ::rewinddir(stream);
  std::optional<bool> result = HoyaScanEntries(name, [stream]() -> const char *_Nullable {
    struct dirent *entry = ::readdir(stream);
    return entry ? entry->d_name : nullptr;
  });
  ::closedir(stream);
  return result;
}

std::optional<bool> HoyaBackupPolicyIO::excludedFlag(const std::string &path) {
  NSURL *url = [NSURL fileURLWithPath:@(path.c_str()) isDirectory:YES];
  NSNumber *value = nil;
  if (![url getResourceValue:&value forKey:NSURLIsExcludedFromBackupKey error:nil] || !value)
    return std::nullopt;
  return static_cast<bool>(value.boolValue);
}

bool HoyaBackupPolicyIO::setExcludedFlag(const std::string &path, bool excluded) {
  NSURL *url = [NSURL fileURLWithPath:@(path.c_str()) isDirectory:YES];
  return [url setResourceValue:@(excluded) forKey:NSURLIsExcludedFromBackupKey error:nil];
}

bool HoyaBackupPolicyIO::truncate(int descriptor) { return ::ftruncate(descriptor, 0) == 0; }

ssize_t HoyaBackupPolicyIO::write(int descriptor, const std::vector<uint8_t> &bytes) {
  return ::pwrite(descriptor, bytes.data(), bytes.size(), 0);
}

bool HoyaBackupPolicyIO::sync(int descriptor) { return ::fsync(descriptor) == 0; }

void HoyaBackupPolicyIO::close(int descriptor) { ::close(descriptor); }

uint64_t HoyaBackupPolicyIO::milliseconds() {
  return clock_gettime_nsec_np(CLOCK_UPTIME_RAW) / 1000000;
}

void HoyaBackupPolicyIO::pause(uint32_t milliseconds) { ::usleep(milliseconds * 1000); }

static const char *const claimName = ".hoyadb-backup-policy";
static const uint64_t acquisitionMilliseconds = 2000;

static bool same(const HoyaFileInfo &left, const HoyaFileInfo &right) {
  return left.device == right.device && left.inode == right.inode;
}

namespace {
struct Descriptor {
  HoyaBackupPolicyIO &io;
  int value;
  ~Descriptor() {
    if (value >= 0) io.close(value);
  }
};
}  // namespace

HoyaClaimResult HoyaBackupPolicyClaim(const std::string &path, const HoyaFileInfo &validated,
                                      bool excluded, HoyaBackupPolicyIO &io,
                                      const std::function<bool()> &cancelled) {
  if (cancelled()) return HoyaClaimResult::cancelled;
  Descriptor directory{io, io.openDirectory(path)};
  if (directory.value < 0) return HoyaClaimResult::failure;
  auto opened = io.fstatInfo(directory.value);
  if (!opened || !opened->directory() || !same(*opened, validated)) return HoyaClaimResult::failure;
  uint64_t deadline = io.milliseconds() + acquisitionMilliseconds;
  int descriptor;
  while ((descriptor = io.openClaim(directory.value, claimName)) < 0) {
    if (descriptor != -EWOULDBLOCK) return HoyaClaimResult::failure;
    if (io.milliseconds() >= deadline) return HoyaClaimResult::busy;
    io.pause(10);
    if (cancelled()) return HoyaClaimResult::cancelled;
  }
  Descriptor claim{io, descriptor};
  auto held = io.fstatInfo(claim.value);
  auto named = io.fstatatInfo(directory.value, claimName);
  if (!held || !held->regular() || held->links != 1 || held->owner != geteuid() ||
      held->size > 16 || !named || !same(*named, *held))
    return HoyaClaimResult::failure;
  auto contents = io.read(claim.value, 17);
  if (!contents || contents->size() > 16) return HoyaClaimResult::failure;
  const std::string text = excluded ? "excluded\n" : "included\n";
  const std::vector<uint8_t> requested(text.begin(), text.end());
  // Re-validates the path before each Foundation flag access, which is path-based.
  auto current = [&]() {
    auto now = io.lstatInfo(path);
    return now && same(*now, *opened);
  };
  if (!contents->empty()) {
    if (*contents != requested || !current() || io.excludedFlag(path) != excluded)
      return HoyaClaimResult::failure;
    return HoyaClaimResult::reserved;
  }
  auto others = io.hasOtherEntries(directory.value, claimName);
  if (!others) return HoyaClaimResult::failure;
  // A cancelled attempt never begins initialization (SPEC-0036 SW-L4).
  if (cancelled()) return HoyaClaimResult::cancelled;
  if (!*others && (!current() || !io.setExcludedFlag(path, excluded)))
    return HoyaClaimResult::failure;
  // A legacy directory is reserved only when its existing flag already matches.
  if (!current() || io.excludedFlag(path) != excluded || !io.truncate(claim.value) ||
      io.write(claim.value, requested) != static_cast<ssize_t>(requested.size()) ||
      !io.sync(claim.value))
    return HoyaClaimResult::failure;
  return HoyaClaimResult::reserved;
}
