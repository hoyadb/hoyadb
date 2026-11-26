// SPEC-0036 item 5: RN iOS copy of the Swift SDK backup policy claim (BackupPolicy.swift).
// The filesystem and Foundation calls are virtual so tests can inject faults; the base class
// is the production Darwin implementation. Objective-C++ only: CocoaPods may list this header
// in an umbrella header compiled as Objective-C.
#ifdef __cplusplus
#import "HoyaNativeHost.h"
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <sys/types.h>
#include <vector>

NS_ASSUME_NONNULL_BEGIN

struct HoyaFileInfo {
  uint64_t device;
  uint64_t inode;
  mode_t mode;
  nlink_t links;
  uid_t owner;
  off_t size;
  bool directory() const;
  bool regular() const;
};

class HoyaBackupPolicyIO {
public:
  virtual ~HoyaBackupPolicyIO() = default;
  virtual std::optional<HoyaFileInfo> lstatInfo(const std::string &path);
  virtual int openDirectory(const std::string &path);
  virtual std::optional<HoyaFileInfo> fstatInfo(int descriptor);
  virtual std::optional<HoyaFileInfo> fstatatInfo(int directory, const char *name);
  /// openat(O_RDWR|O_CREAT|O_NOFOLLOW|O_CLOEXEC|O_EXLOCK|O_NONBLOCK, 0600): descriptor or -errno.
  virtual int openClaim(int directory, const char *name);
  virtual std::optional<std::vector<uint8_t>> read(int descriptor, size_t maximum);
  /// Whether the directory has any entry other than `name`; nullopt when listing fails.
  virtual std::optional<bool> hasOtherEntries(int directory, const char *name);
  virtual std::optional<bool> excludedFlag(const std::string &path);
  virtual bool setExcludedFlag(const std::string &path, bool excluded);
  virtual bool truncate(int descriptor);
  /// Bytes written, or -1 when nothing could be written.
  virtual ssize_t write(int descriptor, const std::vector<uint8_t> &bytes);
  virtual bool sync(int descriptor);
  virtual void close(int descriptor);
  virtual uint64_t milliseconds();
  virtual void pause(uint32_t milliseconds);
};

/// The readdir loop over `next`, which returns an entry name or null like readdir(3): null with
/// errno still 0 ends the listing, null with errno set is a listing error.
std::optional<bool> HoyaScanEntries(
    const char *name, const std::function<const char *_Nullable()> &next);

enum class HoyaClaimResult { reserved, busy, failure, cancelled };

/// SPEC-0036 item 5: `.hoyadb-backup-policy` in the validated directory reserves one backup flag
/// for every store there. The claim is held only while deciding and is released before the
/// key-bearing native open, which stays path-based. Nothing is unlinked, renamed or re-truncated.
/// `cancelled` is checked at entry, during lock retry and before initialization mutations.
HoyaClaimResult HoyaBackupPolicyClaim(const std::string &path, const HoyaFileInfo &validated,
                                      bool excluded, HoyaBackupPolicyIO &io,
                                      const std::function<bool()> &cancelled);

/// HoyaHostOpenCancellable over an injectable policy filesystem seam (tests inject faults).
NSString *HoyaHostOpenWithPolicyIO(
    NSString *input, NSArray *key, HoyaParentProvider parent,
    HoyaOpenCancelled _Nullable cancelled, HoyaBackupPolicyIO &io);

@interface HoyaModuleHost (PolicySeam)
/// Test seam: an injected policy filesystem and module queue.
- (instancetype)initWithParent:(HoyaParentProvider)parent
                            io:(std::shared_ptr<HoyaBackupPolicyIO>)io
                         queue:(dispatch_queue_t)queue;
@end

#ifdef HOYA_HARNESS
/// Harness builds only: called right after each prepared-secret wipe with its length and
/// whether every wiped owned byte reads zero. Absent from normal builds.
extern void (*_Nullable HoyaHarnessWipeObserver)(size_t length, bool zero);
/// Harness builds only: called on the policy queue right after a submit block is enqueued.
extern void (^_Nullable HoyaHarnessSubmitEnqueued)(void);
#endif

NS_ASSUME_NONNULL_END
#endif
