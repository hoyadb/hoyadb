#import "HoyaConsumerModule.h"
#import "HoyaConsumer-Swift.h"
@implementation HoyaConsumerModule
RCT_EXPORT_MODULE(HoyaConsumerControl)
+ (BOOL)requiresMainQueueSetup { return NO; }
- (void)prepareForeign:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
 [HoyaForeignControl prepare:^(NSString *error){if(error)reject(@"ForeignFailure",@"ForeignFailure",nil);else resolve(nil);}];
}
- (void)startForeign:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
 [HoyaForeignControl start:^(NSString *error){if(error)reject(@"ForeignFailure",@"ForeignFailure",nil);else resolve(nil);}];
}
- (void)releaseForeign:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
 [HoyaForeignControl releaseScope:^(NSString *error){if(error)reject(@"ForeignFailure",@"ForeignFailure",nil);else resolve(nil);}];
}
- (void)closeForeign:(RCTPromiseResolveBlock)resolve reject:(RCTPromiseRejectBlock)reject {
 [HoyaForeignControl close:^(NSString *error){if(error)reject(@"ForeignFailure",@"ForeignFailure",nil);else resolve(nil);}];
}
- (std::shared_ptr<facebook::react::TurboModule>)getTurboModule:(const facebook::react::ObjCTurboModule::InitParams &)params {return std::make_shared<facebook::react::NativeHoyaConsumerSpecJSI>(params);}
@end
