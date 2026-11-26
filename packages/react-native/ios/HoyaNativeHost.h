// API-0011 restricted native root profile; functions run on a non-UI executor.
#import <Foundation/Foundation.h>
NS_ASSUME_NONNULL_BEGIN
typedef NSString *_Nonnull (^HoyaParentProvider)(BOOL backupExcluded);
NSString *HoyaHostOpen(NSString *input, NSArray *key, HoyaParentProvider parent);
/// SPEC-0036 item 5: `cancelled` reports bridge invalidation; it is checked at claim entry,
/// during claim-lock retry, before initialization mutations and before native submission.
typedef BOOL (^HoyaOpenCancelled)(void);
NSString *HoyaHostOpenCancellable(NSString *input, NSArray *key, HoyaParentProvider parent,
                                  HoyaOpenCancelled _Nullable cancelled);
NSString *HoyaHostCall(NSString *input);

typedef void (^HoyaHostCompletion)(NSString *_Nullable reply, NSString *_Nullable kind);

/// React-free module host: the module serial queue, live flag and owner registry behind the
/// TurboModule. Completions receive a reply or a static error kind.
@interface HoyaModuleHost : NSObject
- (instancetype)initWithParent:(HoyaParentProvider)parent;
- (instancetype)init NS_UNAVAILABLE;
- (void)open:(NSString *)input key:(NSArray *)key completion:(HoyaHostCompletion)completion;
- (void)call:(NSString *)input completion:(HoyaHostCompletion)completion;
- (void)namespace:(void (^)(NSArray<NSNumber *> *_Nullable value, NSString *_Nullable kind))completion;
- (void)invalidate;
@end
NSArray<NSNumber *> *HoyaHostNamespace(void);
NSString *HoyaHostUptime(void);
NS_ASSUME_NONNULL_END
