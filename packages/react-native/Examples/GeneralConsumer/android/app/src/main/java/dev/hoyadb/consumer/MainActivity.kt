package dev.hoyadb.consumer

import android.os.Bundle
import com.facebook.react.ReactActivity
import com.facebook.react.ReactActivityDelegate
import com.facebook.react.defaults.DefaultReactActivityDelegate

class MainActivity : ReactActivity() {
  override fun getMainComponentName() = "HoyaConsumer"
  override fun createReactActivityDelegate(): ReactActivityDelegate =
    object : DefaultReactActivityDelegate(this, mainComponentName, true) {
      override fun getLaunchOptions() = Bundle().apply {
        putString("mode", intent.getStringExtra("mode") ?: "all")
        putString("reportURL", "http://10.0.2.2:8781/report")
      }
    }
}
