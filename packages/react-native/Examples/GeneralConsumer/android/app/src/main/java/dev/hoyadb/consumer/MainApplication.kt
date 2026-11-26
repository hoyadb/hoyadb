package dev.hoyadb.consumer

import android.app.Application
import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative
import com.facebook.react.defaults.DefaultReactHost.getDefaultReactHost

class MainApplication : Application(), ReactApplication {
  override val reactHost by lazy {
    getDefaultReactHost(applicationContext,
      PackageList(this).packages.apply { add(HoyaConsumerPackage()) },
      useDevSupport = false)
  }

  override fun onCreate() {
    super.onCreate()
    loadReactNative(this)
  }
}
