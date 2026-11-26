// API-0011 §7: original strict preflight, controlled child, one binary-key open.
#import "HoyaNativeHost.h"
#import "HoyaBackupPolicy.h"
#import <Security/Security.h>
#import <hoyadb.h>
#include <atomic>
#include <cmath>
#include <memory>
#include <limits.h>
#include <sys/stat.h>
#include <time.h>

static void fail(NSString *kind) {
  @throw [NSException exceptionWithName:kind reason:nil userInfo:nil];
}

struct HoyaCollector { NSData *bytes; int count = 0; };
static void collect(void *context, const uint8_t *bytes, size_t length) {
  HoyaCollector *reply = static_cast<HoyaCollector *>(context);
  reply->count++;
  if (bytes && length <= 1048576) reply->bytes = [NSData dataWithBytes:bytes length:length];
}

static NSString *invoke(NSString *input, NSData *key, BOOL validate, size_t keyLength) {
  NSData *json = [input dataUsingEncoding:NSUTF8StringEncoding];
  if (!json || json.length > 1048576) fail(@"InvalidRequest");
  HoyaCollector collector;
  int32_t status;
  if (validate) status = hoya_v1_validate_open(static_cast<const uint8_t *>(json.bytes),json.length,keyLength,collect,&collector);
  else if (key) status = hoya_v1_open(static_cast<const uint8_t *>(json.bytes),json.length,static_cast<const uint8_t *>(key.bytes),key.length,collect,&collector);
  else status = hoya_v1_call(static_cast<const uint8_t *>(json.bytes),json.length,collect,&collector);
  if (status != 0) fail(status == 1 ? @"InvalidRequest" : @"BoundaryFailure");
  if (collector.count != 1 || !collector.bytes) fail(@"BoundaryFailure");
  NSString *result = [[NSString alloc] initWithData:collector.bytes encoding:NSUTF8StringEncoding];
  if (!result) fail(@"BoundaryFailure");
  return result;
}

static NSDictionary *object(NSString *input) {
  id value = [NSJSONSerialization JSONObjectWithData:[input dataUsingEncoding:NSUTF8StringEncoding] options:NSJSONReadingMutableContainers error:nil];
  if (![value isKindOfClass:NSDictionary.class]) fail(@"InvalidRequest");
  return value;
}

static void preflight(NSString *input, size_t length) {
  if (![object(invoke(input,nil,YES,length))[@"ok"] boolValue]) fail(@"InvalidRequest");
}

static NSString *canonical(NSString *path) {
  char resolved[PATH_MAX];
  if (!realpath(path.fileSystemRepresentation,resolved)) fail(@"InvalidRequest");
  return [NSString stringWithUTF8String:resolved];
}

NSString *HoyaHostOpen(NSString *input, NSArray *key, HoyaParentProvider parent) {
  return HoyaHostOpenCancellable(input,key,parent,nil);
}

NSString *HoyaHostOpenCancellable(NSString *input, NSArray *key, HoyaParentProvider parent, HoyaOpenCancelled cancelled) {
  HoyaBackupPolicyIO io;
  return HoyaHostOpenWithPolicyIO(input,key,parent,cancelled,io);
}

#ifdef HOYA_HARNESS
void (*HoyaHarnessWipeObserver)(size_t, bool)=nullptr;
void (^HoyaHarnessSubmitEnqueued)(void)=nil;
#endif

static void wipe(NSMutableData *secret) {
  volatile uint8_t *bytes=static_cast<volatile uint8_t *>(secret.mutableBytes);
  for (NSUInteger i=0;i<secret.length;i++) bytes[i]=0;
#ifdef HOYA_HARNESS
  // Reads the wiped owned bytes back; never the caller's key array.
  bool zero=true;
  for (NSUInteger i=0;i<secret.length;i++) zero=zero && bytes[i]==0;
  if (HoyaHarnessWipeObserver) HoyaHarnessWipeObserver(secret.length,zero);
#endif
}

namespace {
/// The policy phase's result. Until handoff the policy phase owns `secret`; after it, only
/// the submit phase does, and it wipes `secret` on every exit.
struct HoyaPreparedOpen { NSString *accepted; NSMutableData *secret; };
}

static HoyaPreparedOpen prepareOpen(NSString *input, NSArray *key, HoyaParentProvider parent, HoyaOpenCancelled cancelled, HoyaBackupPolicyIO &io) {
  if (![input isKindOfClass:NSString.class] || ![key isKindOfClass:NSArray.class] || (key.count != 0 && key.count != 32)) fail(@"InvalidRequest");
  NSMutableData *secret = [NSMutableData dataWithLength:key.count];
  @try {
    uint8_t *bytes = static_cast<uint8_t *>(secret.mutableBytes);
    for (NSUInteger i=0;i<key.count;i++) {
      id value=key[i];
      if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value)==CFBooleanGetTypeID()) fail(@"InvalidRequest");
      double number=[value doubleValue];
      if (!std::isfinite(number) || number<0 || number>255 || number!=std::floor(number)) fail(@"InvalidRequest");
      bytes[i]=static_cast<uint8_t>(number);
    }
    // Parse only after the original Rust parser has rejected duplicate keys,
    // numeric tokens, invalid schema/options and incorrect key/engine shape.
    preflight(input,secret.length);
    NSMutableDictionary *frame=[object(input) mutableCopy];
    NSMutableDictionary *args=[frame[@"args"] mutableCopy];
    NSMutableDictionary *engine=[args[@"engine"] mutableCopy];
    if ([engine[@"kind"] isEqual:@"memory"]) return {input,secret};
    NSString *name=engine[@"name"];
    if (![engine[@"kind"] isEqual:@"native"] || ![engine[@"directory"] isEqual:@"@hoyadb/default"] || secret.length!=32 || ![name isKindOfClass:NSString.class] || name.length==0 || ![name.lastPathComponent isEqual:name] || [name containsString:@"\\"] || [name containsString:@"\0"] || [name isEqual:@"."] || [name isEqual:@".."]) fail(@"InvalidRequest");
    BOOL excluded=[engine[@"backup_excluded"] boolValue];
    // The supplied production provider resolves only Application Support. OS
    // parent getters may create their managed root, after validation above.
    NSString *root=canonical(parent(excluded));
    struct stat parentInfo;
    if (lstat(root.fileSystemRepresentation,&parentInfo)!=0 || !S_ISDIR(parentInfo.st_mode) || S_ISLNK(parentInfo.st_mode)) fail(@"InvalidRequest");
    NSString *directory=[root stringByAppendingPathComponent:@"hoyadb"];
    engine[@"directory"]=directory;args[@"engine"]=engine;frame[@"args"]=args;
    NSData *rewritten=[NSJSONSerialization dataWithJSONObject:frame options:0 error:nil];
    if (!rewritten || rewritten.length>1048576) fail(@"InvalidRequest");
    NSString *accepted=[[NSString alloc]initWithData:rewritten encoding:NSUTF8StringEncoding];
    preflight(accepted,secret.length);
    struct stat info;
    if (lstat(directory.fileSystemRepresentation,&info)!=0) {
      if (errno!=ENOENT || ![NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:NO attributes:@{NSFilePosixPermissions:@0700} error:nil]) fail(@"BackupPolicyFailure");
    }
    if (lstat(directory.fileSystemRepresentation,&info)!=0 || !S_ISDIR(info.st_mode) || S_ISLNK(info.st_mode) || ![canonical(directory) isEqual:directory]) fail(@"BackupPolicyFailure");
    std::string path(directory.fileSystemRepresentation);
    auto validated=io.lstatInfo(path);
    if (!validated || !validated->directory()) fail(@"BackupPolicyFailure");
    auto invalidated=[cancelled]() { return cancelled!=nil && cancelled(); };
    // SPEC-0036 item 5: the directory's reserved policy decides before the key-bearing open.
    HoyaClaimResult claim=HoyaBackupPolicyClaim(path,*validated,excluded,io,invalidated);
    // A claim outcome after invalidation reports the closed attempt, not the claim.
    if (claim==HoyaClaimResult::cancelled || invalidated()) fail(@"Closed");
    if (claim==HoyaClaimResult::busy) fail(@"Busy");
    if (claim!=HoyaClaimResult::reserved) fail(@"BackupPolicyFailure");
    return {accepted,secret};
  } @catch (NSException *error) {
    wipe(secret);
    @throw;
  }
}

static NSString *submitOpen(const HoyaPreparedOpen &prepared, HoyaOpenCancelled cancelled) {
  @try {
    // SPEC-0036 RN iOS lifecycle: the admission boundary for the one key-bearing open.
    if (cancelled && cancelled()) fail(@"Closed");
    return invoke(prepared.accepted,prepared.secret,NO,0);
  } @finally {
    wipe(prepared.secret);
  }
}

NSString *HoyaHostOpenWithPolicyIO(NSString *input, NSArray *key, HoyaParentProvider parent, HoyaOpenCancelled cancelled, HoyaBackupPolicyIO &io) {
  return submitOpen(prepareOpen(input,key,parent,cancelled,io),cancelled);
}

NSString *HoyaHostCall(NSString *input) {
  NSString *reply=nil;
  @try { reply=invoke(input,nil,NO,0); }
  @finally {
    // Collector and Rust registry guard have returned before platform wake.
    id frame=[NSJSONSerialization JSONObjectWithData:[input dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
    NSString *operation=[frame isKindOfClass:NSDictionary.class]?frame[@"op"]:nil;
    if ([@[@"finish",@"abort",@"cancel_request",@"close",@"reset"] containsObject:operation]) [NSNotificationCenter.defaultCenter postNotificationName:@"dev.hoyadb.admissionReleased.v1" object:nil];
  }
  return reply;
}

NSArray<NSNumber *> *HoyaHostNamespace(void) {
  uint8_t bytes[16];
  if (SecRandomCopyBytes(kSecRandomDefault,sizeof(bytes),bytes)!=errSecSuccess) fail(@"BoundaryFailure");
  NSMutableArray *result=[NSMutableArray arrayWithCapacity:16];for (uint8_t byte:bytes) [result addObject:@(byte)];
  return result;
}

NSString *HoyaHostUptime(void) {
  struct timespec value;
  if (clock_gettime(CLOCK_UPTIME_RAW,&value)!=0 || value.tv_sec<0) fail(@"BoundaryFailure");
  uint64_t seconds=static_cast<uint64_t>(value.tv_sec);
  if (seconds>UINT64_MAX/1000) fail(@"BoundaryFailure");
  return [NSString stringWithFormat:@"%llu",static_cast<unsigned long long>(seconds*1000+value.tv_nsec/1000000)];
}

static NSDictionary *readObject(NSString *text) {
  id value=[NSJSONSerialization JSONObjectWithData:[text dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
  return [value isKindOfClass:NSDictionary.class]?value:nil;
}

static NSString *closeFrame(NSString *owner) {
  NSDictionary *frame=@{@"abi":@"1",@"op":@"close",@"owner":owner,@"args":@{}};
  return [[NSString alloc]initWithData:[NSJSONSerialization dataWithJSONObject:frame options:0 error:nil] encoding:NSUTF8StringEncoding];
}

@interface HoyaModuleHost () {
  dispatch_queue_t _queue;
  dispatch_queue_t _policyQueue;
  std::atomic<bool> _live;
  NSMutableSet<NSString *> *_owners;
  HoyaParentProvider _parent;
  std::shared_ptr<HoyaBackupPolicyIO> _io;
}
@end

@implementation HoyaModuleHost

- (instancetype)initWithParent:(HoyaParentProvider)parent {
  return [self initWithParent:parent io:std::make_shared<HoyaBackupPolicyIO>() queue:dispatch_queue_create("dev.hoyadb.react-native",DISPATCH_QUEUE_SERIAL)];
}

- (instancetype)initWithParent:(HoyaParentProvider)parent io:(std::shared_ptr<HoyaBackupPolicyIO>)io queue:(dispatch_queue_t)queue {
  if ((self=[super init])) {
    _live.store(true);
    _queue=queue;
    // SPEC-0036 RN iOS lifecycle: claim waits run here, never on the module queue.
    _policyQueue=dispatch_queue_create("dev.hoyadb.react-native.policy",DISPATCH_QUEUE_SERIAL);
    _owners=[NSMutableSet set];
    _parent=[parent copy];
    _io=io;
  }
  return self;
}

- (void)open:(NSString *)input key:(NSArray *)key completion:(HoyaHostCompletion)completion {
  NSString *owned=[input copy];NSArray *secret=[key copy];
  dispatch_async(_policyQueue,^{
    if (!self->_live.load()) { completion(nil,@"Closed");return; }
    HoyaOpenCancelled cancelled=^BOOL { return !self->_live.load(); };
    HoyaPreparedOpen prepared;
    @try { prepared=prepareOpen(owned,secret,self->_parent,cancelled,*self->_io); }
    @catch (NSException *error) { completion(nil,error.name);return; }
    // Handoff: from here only the submit block owns the prepared secret.
    dispatch_async(self->_queue,^{ [self submit:prepared cancelled:cancelled completion:completion]; });
#ifdef HOYA_HARNESS
    if (HoyaHarnessSubmitEnqueued) HoyaHarnessSubmitEnqueued();
#endif
  });
}

- (void)submit:(const HoyaPreparedOpen &)prepared cancelled:(HoyaOpenCancelled)cancelled completion:(HoyaHostCompletion)completion {
  @try {
    NSString *reply=submitOpen(prepared,cancelled);
    NSDictionary *value=readObject(reply);
    NSString *owner=[value[@"ok"] boolValue] && [value[@"value"] isKindOfClass:NSString.class]?value[@"value"]:nil;
    if (!_live.load()) {
      // Invalidated after admission: close the late owner; it is never registered.
      if (owner) HoyaHostCall(closeFrame(owner));
      completion(nil,@"Closed");return;
    }
    if (owner) [_owners addObject:owner];
    completion(reply,nil);
  } @catch (NSException *error) { completion(nil,error.name); }
}

- (void)call:(NSString *)input completion:(HoyaHostCompletion)completion {
  NSString *owned=[input copy];
  dispatch_async(_queue,^{
    if (!self->_live.load()) { completion(nil,@"Closed");return; }
    @try {
      NSString *reply=HoyaHostCall(owned);
      NSDictionary *frame=readObject(owned),*value=readObject(reply);
      if ([frame[@"op"] isEqual:@"close"] && [value[@"ok"] boolValue]) [self->_owners removeObject:frame[@"owner"]];
      if (self->_live.load()) completion(reply,nil);else completion(nil,@"Closed");
    } @catch (NSException *error) { completion(nil,error.name); }
  });
}

- (void)namespace:(void (^)(NSArray<NSNumber *> *_Nullable, NSString *_Nullable))completion {
  dispatch_async(_queue,^{
    if (!self->_live.load()) { completion(nil,@"Closed");return; }
    @try { completion(HoyaHostNamespace(),nil); }
    @catch (NSException *error) { completion(nil,error.name); }
  });
}

- (void)invalidate {
  if (!_live.exchange(false)) return;
  dispatch_async(_queue,^{
    for (NSString *owner in self->_owners) { @try { HoyaHostCall(closeFrame(owner)); } @catch (NSException *) {} }
    [self->_owners removeAllObjects];
  });
}

@end
