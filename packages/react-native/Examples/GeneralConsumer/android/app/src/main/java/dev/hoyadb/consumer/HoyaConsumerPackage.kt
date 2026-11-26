package dev.hoyadb.consumer

import com.facebook.react.BaseReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.module.model.ReactModuleInfo
import com.facebook.react.module.model.ReactModuleInfoProvider

class HoyaConsumerPackage : BaseReactPackage() {
  override fun getModule(name: String, context: ReactApplicationContext): NativeModule? =
    if (name == NativeHoyaConsumerSpec.NAME) HoyaConsumerModule(context) else null

  override fun getReactModuleInfoProvider() = ReactModuleInfoProvider {
    mapOf(NativeHoyaConsumerSpec.NAME to ReactModuleInfo(
      NativeHoyaConsumerSpec.NAME, HoyaConsumerModule::class.java.name,
      false, false, false, true))
  }
}
