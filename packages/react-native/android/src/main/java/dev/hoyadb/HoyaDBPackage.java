package dev.hoyadb;

import com.facebook.react.BaseReactPackage;
import com.facebook.react.bridge.NativeModule;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.module.model.ReactModuleInfo;
import com.facebook.react.module.model.ReactModuleInfoProvider;
import java.util.Collections;

public final class HoyaDBPackage extends BaseReactPackage {
  @Override public NativeModule getModule(String name, ReactApplicationContext context) {
    return name.equals(NativeHoyaDBSpec.NAME)?new HoyaDBModule(context):null;
  }
  @Override public ReactModuleInfoProvider getReactModuleInfoProvider() {
    return () -> Collections.singletonMap(NativeHoyaDBSpec.NAME,
      new ReactModuleInfo(NativeHoyaDBSpec.NAME,HoyaDBModule.class.getName(),false,false,false,true));
  }
}
