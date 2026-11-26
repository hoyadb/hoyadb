// TC-0036 G10/G16: actual pure C validator before any controlled-root action.
#import "../ios/HoyaNativeHost.h"
#include <cstdio>
#include <cstdlib>

static void check(BOOL value, const char *message) { if(!value){fprintf(stderr,"FAIL %s\n",message);exit(1);} }
static NSString *frame(NSDictionary *engine) {
 NSDictionary *value=@{@"abi":@"1",@"op":@"open",@"owner":[NSNull null],@"args":@{
 @"engine":engine,@"schema":@{@"version":@"1",@"tables":@{@"comets":@{@"columns":@{@"label":@{@"kind":@"String",@"optional":@NO}},@"relations":@{}}}},
 @"migrations":@[],@"namespace":@"96969696969696969696969696969696",@"options":@{@"capacity":@"1000",@"history":@"100",@"initial_delay_ms":@"1000",@"multiplier":@"2",@"max_delay_ms":@"60000"}}};
 return [[NSString alloc]initWithData:[NSJSONSerialization dataWithJSONObject:value options:0 error:nil] encoding:NSUTF8StringEncoding];
}
static NSDictionary *json(NSString *value) {return [NSJSONSerialization JSONObjectWithData:[value dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];}
static NSString *caught(NSString *(^work)(void)){@try{return work();}@catch(NSException *e){return e.name;}}
int main(int argc,char **argv){@autoreleasepool{
 check(argc==2,"owned parent argument");NSString *parent=[NSString stringWithUTF8String:argv[1]];
 __block int parents=0;HoyaParentProvider provider=^NSString*(BOOL excluded){parents++;check(excluded,"default exclusion");return parent;};
 NSMutableArray *key=[NSMutableArray array];for(int i=0;i<32;i++)[key addObject:@(i==31?255:i)];
 NSString *input=frame(@{@"kind":@"native",@"directory":@"@hoyadb/default",@"name":@"host-v1",@"backup_excluded":@YES});
 NSString *duplicate=[input stringByReplacingCharactersInRange:NSMakeRange(1,0) withString:@"\"abi\":\"1\","];
 check([caught(^NSString*{return HoyaHostOpen(duplicate,key,provider);}) isEqual:@"InvalidRequest"],"strict original duplicate rejected");
 check(parents==0,"duplicate before parent getter");
 check([caught(^NSString*{return HoyaHostOpen(input,@[],provider);}) isEqual:@"InvalidRequest"],"missing key rejected");check(parents==0,"missing key before parent getter");
 for(NSString *invalidName in @[@"../outside",@"back\\slash",[NSString stringWithFormat:@"nul%Cname",(unichar)0]]) {
  NSString *bad=frame(@{@"kind":@"native",@"directory":@"@hoyadb/default",@"name":invalidName,@"backup_excluded":@YES});
  check([caught(^NSString*{return HoyaHostOpen(bad,key,provider);}) isEqual:@"InvalidRequest"],"invalid component name rejected");
  check(parents==0,"invalid name before parent getter");
 }
 NSString *other=frame(@{@"kind":@"native",@"directory":parent,@"name":@"host-v1",@"backup_excluded":@YES});
 check([caught(^NSString*{return HoyaHostOpen(other,key,provider);}) isEqual:@"InvalidRequest"],"arbitrary root rejected");check(parents==0,"arbitrary root before parent getter");
 NSUInteger baseBytes=[input dataUsingEncoding:NSUTF8StringEncoding].length;
 NSUInteger count=1048570-baseBytes+5;
 NSString *largeName=[@"" stringByPaddingToLength:count withString:@"x" startingAtIndex:0];
 NSString *near=[input stringByReplacingOccurrencesOfString:@"\"label\"" withString:[NSString stringWithFormat:@"\"%@\"",largeName]];
 check([near dataUsingEncoding:NSUTF8StringEncoding].length==1048570,"near-limit original exact bytes");
 check([caught(^NSString*{return HoyaHostOpen(near,key,provider);}) isEqual:@"InvalidRequest"],"expanded path rejects wire overflow");
 check(parents==1,"bounded original preflight permits one OS parent resolution");
 check(![NSFileManager.defaultManager fileExistsAtPath:[parent stringByAppendingPathComponent:@"hoyadb"]],"expanded overflow before child creation");
 parents=0;
 NSString *opened=caught(^NSString*{return HoyaHostOpen(input,key,provider);});NSDictionary *reply=json(opened);
 check([reply[@"ok"] boolValue],"valid protected open");check(parents==1,"single parent resolution");
 NSString *owner=reply[@"value"];check([owner isKindOfClass:NSString.class],"owned owner reply");
 NSURL *child=[NSURL fileURLWithPath:[parent stringByAppendingPathComponent:@"hoyadb"] isDirectory:YES];NSNumber *excluded=nil;[child getResourceValue:&excluded forKey:NSURLIsExcludedFromBackupKey error:nil];check(excluded.boolValue,"Foundation backup flag readback");
 NSDictionary *close=@{@"abi":@"1",@"op":@"close",@"owner":owner,@"args":@{}};
 NSString *closed=HoyaHostCall([[NSString alloc]initWithData:[NSJSONSerialization dataWithJSONObject:close options:0 error:nil] encoding:NSUTF8StringEncoding]);check([json(closed)[@"ok"] boolValue],"close");
 NSArray *entropy=HoyaHostNamespace();check(entropy.count==16,"OS entropy16");for(NSNumber *byte in entropy)check(byte.integerValue>=0&&byte.integerValue<=255,"byte value");
 check(HoyaHostUptime().length>0,"monotonic string");puts("HOYA_RN_APPLE_HOST_GREEN preflight/protected/root/backup/entropy/uptime");
}}
