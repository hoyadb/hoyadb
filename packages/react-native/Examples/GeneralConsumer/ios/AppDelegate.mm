#import "AppDelegate.h"
#import <React-RCTAppDelegate/RCTReactNativeFactory.h>
#import <React-RCTAppDelegate/RCTDefaultReactNativeFactoryDelegate.h>
#import <ReactAppDependencyProvider/RCTAppDependencyProvider.h>

@interface HoyaFactoryDelegate : RCTDefaultReactNativeFactoryDelegate
@end
@implementation HoyaFactoryDelegate
- (NSURL *)bundleURL { return [NSBundle.mainBundle URLForResource:@"main" withExtension:@"jsbundle"]; }
- (NSURL *)sourceURLForBridge:(RCTBridge *)bridge { return [self bundleURL]; }
@end

@interface AppDelegate ()
@property(nonatomic,strong) HoyaFactoryDelegate *delegate;
@property(nonatomic,strong) RCTReactNativeFactory *factory;
@end
@implementation AppDelegate
- (BOOL)application:(UIApplication *)application didFinishLaunchingWithOptions:(NSDictionary *)options {
 self.delegate=[HoyaFactoryDelegate new];self.delegate.dependencyProvider=[RCTAppDependencyProvider new];
 self.factory=[[RCTReactNativeFactory alloc]initWithDelegate:self.delegate];
 self.window=[[UIWindow alloc]initWithFrame:UIScreen.mainScreen.bounds];
 NSDictionary *environment=NSProcessInfo.processInfo.environment;
 NSString *mode=environment[@"HOYA_RN_MODE"]?:@"all";
 [self.factory startReactNativeWithModuleName:@"HoyaConsumer" inWindow:self.window initialProperties:@{@"mode":mode,@"reportURL":@"http://127.0.0.1:8781/report"} launchOptions:options];
 return YES;
}
@end
