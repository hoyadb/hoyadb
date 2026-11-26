// API-0011: native host capabilities over the one shared AAR transport.
package dev.hoyadb;

import android.os.SystemClock;
import com.facebook.react.bridge.Arguments;
import com.facebook.react.bridge.Promise;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.bridge.ReadableArray;
import com.facebook.react.bridge.ReadableType;
import com.facebook.react.bridge.WritableArray;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONObject;

public final class HoyaDBModule extends NativeHoyaDBSpec {
  private static final ExecutorService POLICY = Executors.newSingleThreadExecutor(work -> {
    Thread thread = new Thread(work, "hoyadb-rn-policy"); thread.setDaemon(true); return thread;
  });
  private final AtomicBoolean live = new AtomicBoolean(true);
  private final Object lifecycle = new Object();
  private final Set<String> owners = ConcurrentHashMap.newKeySet();
  private final AutoCloseable release;

  public HoyaDBModule(ReactApplicationContext context) {
    super(context);
    release = HoyaNativeTransport.subscribeRelease(POLICY, () -> {
      if (live.get() && mEventEmitterCallback != null) emitOnAdmissionReleased();
    });
  }

  private static byte[] bytes(String frame) { return frame.getBytes(StandardCharsets.UTF_8); }
  private static void fail(Promise promise, String kind) { promise.reject(kind, kind); }
  private boolean alive(Promise promise) { if (live.get()) return true; fail(promise,"Closed"); return false; }
  private static boolean accepted(byte[] reply) throws Exception {
    return new JSONObject(new String(reply,StandardCharsets.UTF_8)).getBoolean("ok");
  }
  private interface Success { void run(byte[] reply) throws Exception; }
  private HoyaNativeTransport.Reply reply(Promise promise, Success success, byte[] secret) {
    return new HoyaNativeTransport.Reply() {
      @Override public void onSuccess(byte[] result) { POLICY.execute(() -> {
        try { success.run(result); } catch (Exception error) { if(secret!=null) Arrays.fill(secret,(byte)0); fail(promise,"InvalidRequest"); }
      }); }
      @Override public void onFailure(String kind) { if(secret!=null) Arrays.fill(secret,(byte)0); fail(promise,kind); }
    };
  }

  @Override public void open(String input, ReadableArray key, Promise promise) {
    final byte[] secret;
    try {
      if (key == null || (key.size()!=0 && key.size()!=32)) { fail(promise,"InvalidRequest"); return; }
      secret = new byte[key.size()];
      for(int i=0;i<key.size();i++) {
        if(key.getType(i)!=ReadableType.Number) { Arrays.fill(secret,(byte)0); fail(promise,"InvalidRequest"); return; }
        double value=key.getDouble(i);
        if(!Double.isFinite(value) || value<0 || value>255 || value!=Math.floor(value)) { Arrays.fill(secret,(byte)0); fail(promise,"InvalidRequest"); return; }
        secret[i]=(byte)((int)value);
      }
    } catch(RuntimeException error) { fail(promise,"InvalidRequest"); return; }
    POLICY.execute(() -> {
      if(!alive(promise)) { Arrays.fill(secret,(byte)0); return; }
      // Original strict Rust parser runs before JSONObject or OS root getters.
      HoyaNativeTransport.validateOpen(bytes(input),secret.length,reply(promise,validated -> {
        if(!alive(promise)) { Arrays.fill(secret,(byte)0); return; }
        if(!accepted(validated)) { Arrays.fill(secret,(byte)0); promise.resolve(new String(validated,StandardCharsets.UTF_8)); return; }
        JSONObject frame=new JSONObject(input),args=frame.getJSONObject("args"),engine=args.getJSONObject("engine");
        if(engine.getString("kind").equals("memory")) { openAccepted(input,secret,promise); return; }
        String name=engine.getString("name");
        if(!engine.getString("kind").equals("native") || !engine.getString("directory").equals("@hoyadb/default") || secret.length!=32 || name.isEmpty() || !new File(name).getName().equals(name) || name.equals(".") || name.equals("..")) throw new IllegalArgumentException();
        boolean excluded=engine.getBoolean("backup_excluded");
        // Android getters can create the OS-managed parent, after validation.
        File parent=(excluded?getReactApplicationContext().getNoBackupFilesDir():getReactApplicationContext().getFilesDir()).getCanonicalFile();
        File directory=new File(parent,"hoyadb");
        if(!directory.getAbsolutePath().equals(directory.getCanonicalPath())) throw new IllegalArgumentException();
        engine.put("directory",directory.getAbsolutePath());
        String rewritten=frame.toString();byte[] original=bytes(rewritten);
        if(original.length>1_048_576) throw new IllegalArgumentException();
        HoyaNativeTransport.validateOpen(original,secret.length,reply(promise,rechecked -> {
          if(!alive(promise)) { Arrays.fill(secret,(byte)0); return; }
          if(!accepted(rechecked)) { Arrays.fill(secret,(byte)0); promise.resolve(new String(rechecked,StandardCharsets.UTF_8)); return; }
          // SPEC-0036 item 6: the Kotlin SDK's BackupPolicyLock (process-wide semaphore, then the
          // no-follow file lock) is held from the opposite-parent check until the key-bearing open
          // replies. A policy-rejected open creates no store directory.
          BackupPolicyLock.Held policy=null;
          try {
            policy=BackupPolicyLock.acquire(getReactApplicationContext());
            BackupPolicyLock.requireNoOppositeStore(getReactApplicationContext(),name,excluded);
            if(!parent.isDirectory() || !parent.equals((excluded?getReactApplicationContext().getNoBackupFilesDir():getReactApplicationContext().getFilesDir()).getCanonicalFile()) || (!directory.isDirectory() && !directory.mkdir()) || !directory.isDirectory() || !directory.getAbsolutePath().equals(directory.getCanonicalPath())) throw new IllegalStateException();
          } catch(Exception error) {
            if(policy!=null) policy.close();
            Arrays.fill(secret,(byte)0);fail(promise,"BackupPolicyFailure");return;
          }
          openAccepted(rewritten,secret,promise,policy);
        },secret));
      },secret));
    });
  }

  private void openAccepted(String frame, byte[] secret, Promise promise) { openAccepted(frame,secret,promise,null); }

  private void openAccepted(String frame, byte[] secret, Promise promise, BackupPolicyLock.Held policy) {
    final HoyaNativeTransport.Reply settled=reply(promise,result -> {
      JSONObject value=new JSONObject(new String(result,StandardCharsets.UTF_8));
      String owner=value.getBoolean("ok")?value.getString("value"):null;
      // Publication and retirement have one ordering, including promise settlement.
      synchronized(lifecycle) {
        if(live.get()) {
          if(owner!=null) owners.add(owner);
          promise.resolve(new String(result,StandardCharsets.UTF_8));
          return;
        }
      }
      if(owner!=null) closeOwner(owner);
      fail(promise,"Closed");
    },null);
    // Release on the transport's reply thread, on every reply: the policy executor may already be
    // waiting for this lock in the next open, and module teardown does not cancel a submitted open.
    final HoyaNativeTransport.Reply released=policy==null?settled:new HoyaNativeTransport.Reply() {
      @Override public void onSuccess(byte[] result) { policy.close(); settled.onSuccess(result); }
      @Override public void onFailure(String kind) { policy.close(); settled.onFailure(kind); }
    };
    try { HoyaNativeTransport.open(bytes(frame),secret,released); }
    catch(RuntimeException error) { if(policy!=null) policy.close(); throw error; }
    // The common AAR copied its owned key synchronously before returning.
    finally { Arrays.fill(secret,(byte)0); }
  }

  @Override public void call(String input, Promise promise) {
    if(!alive(promise)) return;
    HoyaNativeTransport.call(bytes(input),reply(promise,result -> {
      JSONObject frame=new JSONObject(input);
      if(frame.optString("op").equals("close") && accepted(result)) owners.remove(frame.getString("owner"));
      if(alive(promise)) promise.resolve(new String(result,StandardCharsets.UTF_8));
    },null));
  }

  @Override public void namespace(Promise promise) {
    POLICY.execute(() -> {
      if(!alive(promise)) return;
      try { byte[] entropy=new byte[16];new SecureRandom().nextBytes(entropy);WritableArray result=Arguments.createArray();for(byte value:entropy) result.pushInt(value&255);promise.resolve(result); }
      catch(RuntimeException error) {fail(promise,"BoundaryFailure");}
    });
  }

  @Override public String uptime() { return Long.toString(SystemClock.elapsedRealtime()); }

  private static void closeOwner(String owner) {
    try {
      String frame=new JSONObject().put("abi","1").put("op","close").put("owner",owner).put("args",new JSONObject()).toString();
      HoyaNativeTransport.call(bytes(frame),new HoyaNativeTransport.Reply(){public void onSuccess(byte[] ignored){} public void onFailure(String ignored){}});
    } catch(Exception ignored) { /* Fixed valid owned frame cannot fail serialization. */ }
  }

  @Override public void invalidate() {
    final List<String> closing;
    synchronized(lifecycle) {
      if(!live.getAndSet(false)) return;
      closing=new ArrayList<>(owners);
      owners.clear();
    }
    try {release.close();} catch(Exception ignored) { /* Idempotent owned registration. */ }
    for(String owner:closing) closeOwner(owner);
    super.invalidate();
  }
}
