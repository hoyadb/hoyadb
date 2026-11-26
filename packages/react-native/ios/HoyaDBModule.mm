#import "HoyaDBModule.h"
#import "HoyaNativeHost.h"
#import <React/RCTInvalidating.h>
#include <atomic>
#include <mutex>

@interface HoyaDBModule () <RCTInvalidating> {
  HoyaModuleHost *_host;
  id _releaseObserver;
  std::atomic<bool> _live;
  std::mutex _eventMutex;
}
@end

@implementation HoyaDBModule
RCT_EXPORT_MODULE(HoyaDB)

+ (BOOL)requiresMainQueueSetup { return NO; }

- (instancetype)init {
  if ((self=[super init])) {
    _live.store(true);
    _host=[[HoyaModuleHost alloc] initWithParent:^NSString *(BOOL) {
      NSURL *url=[NSFileManager.defaultManager URLForDirectory:NSApplicationSupportDirectory inDomain:NSUserDomainMask appropriateForURL:nil create:YES error:nil];
      if (!url) @throw [NSException exceptionWithName:@"BackupPolicyFailure" reason:nil userInfo:nil];
      return url.path;
    }];
    __weak HoyaDBModule *weakSelf=self;
    _releaseObserver=[NSNotificationCenter.defaultCenter addObserverForName:@"dev.hoyadb.admissionReleased.v1" object:nil queue:nil usingBlock:^(NSNotification *) {
      HoyaDBModule *strongSelf=weakSelf;
      if (!strongSelf || !strongSelf->_live.load()) return;
      facebook::react::EventEmitterCallback callback;
      {
        std::lock_guard<std::mutex> lock(strongSelf->_eventMutex);
        callback=strongSelf->_eventEmitterCallback;
      }
      // Copy before invoking: no native guard or host lock spans JS dispatch.
      if (strongSelf->_live.load() && callback) callback("onAdmissionReleased",nil);
    }];
  }
  return self;
}

- (void)setEventEmitterCallback:(EventEmitterCallbackWrapper *)wrapper {
  std::lock_guard<std::mutex> lock(_eventMutex);
  [super setEventEmitterCallback:wrapper];
}

- (void)dealloc {
  if (_releaseObserver) [NSNotificationCenter.defaultCenter removeObserver:_releaseObserver];
}

static NSString *staticKind(NSString *kind) {
  return [@[@"InvalidRequest",@"BackupPolicyFailure",@"BoundaryFailure",@"Busy",@"Closed"] containsObject:kind]?kind:@"BoundaryFailure";
}

static HoyaHostCompletion settle(RCTPromiseResolveBlock resolve, RCTPromiseRejectBlock reject) {
  return ^(NSString *reply, NSString *kind) {
    if (reply) { resolve(reply);return; }
    NSString *code=staticKind(kind);reject(code,code,nil);
  };
}

- (void)open:(NSString *)input key:(NSArray *)key resolve:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
  [_host open:input key:key completion:settle(resolve,reject)];
}

- (void)call:(NSString *)input resolve:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
  [_host call:input completion:settle(resolve,reject)];
}

- (void)namespace:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
  [_host namespace:^(NSArray<NSNumber *> *value, NSString *kind) {
    if (value) { resolve(value);return; }
    NSString *code=staticKind(kind);reject(code,code,nil);
  }];
}

- (NSString *)uptime { return HoyaHostUptime(); }

- (void)invalidate {
  if (!_live.exchange(false)) return;
  if (_releaseObserver) {[NSNotificationCenter.defaultCenter removeObserver:_releaseObserver];_releaseObserver=nil;}
  [_host invalidate];
}

- (std::shared_ptr<facebook::react::TurboModule>)getTurboModule:(const facebook::react::ObjCTurboModule::InitParams &)params {
  return std::make_shared<facebook::react::NativeHoyaDBSpecJSI>(params);
}
@end
