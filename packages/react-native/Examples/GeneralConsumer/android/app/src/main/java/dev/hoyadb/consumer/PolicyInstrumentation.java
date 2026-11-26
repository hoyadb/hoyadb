// Installed only in this owned app, using Android's built-in instrumentation.
package dev.hoyadb.consumer;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;

public final class PolicyInstrumentation extends Instrumentation {
  @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }
  @Override public void onStart() {
    Bundle result=new Bundle();
    try {
      PolicyProbe.run(getTargetContext());
      result.putBoolean("ok",true);
      finish(Activity.RESULT_OK,result);
    } catch(Throwable error) {
      result.putBoolean("ok",false);
      result.putString("label",error instanceof AssertionError?error.getMessage():"PolicyProbeFailure");
      finish(Activity.RESULT_CANCELED,result);
    }
  }
}
