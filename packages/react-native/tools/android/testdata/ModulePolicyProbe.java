package dev.hoyadb;

import com.facebook.react.bridge.*;
import java.io.File;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/** SPEC-0036 item 6: host-only ordering of the shared backup-policy lock in the production module. */
public final class ModulePolicyProbe {
  private static void require(boolean condition, String message) {
    if (!condition) throw new AssertionError(message);
  }

  private interface Condition { boolean reached(); }

  private static void eventually(Condition condition, String message) throws InterruptedException {
    long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
    while (!condition.reached()) { require(System.nanoTime() < deadline, message); Thread.sleep(1); }
  }

  private static final class Settled implements Promise {
    final AtomicReference<String> outcome = new AtomicReference<>();
    public void resolve(Object value) { require(outcome.compareAndSet(null, "resolved"), "settled twice"); }
    public void reject(String code, String message) { require(outcome.compareAndSet(null, code), "settled twice"); }
  }

  private static ReadableArray key(int size) {
    return new ReadableArray() {
      public int size() { return size; }
      public ReadableType getType(int index) { return ReadableType.Number; }
      public double getDouble(int index) { return 7; }
    };
  }

  public static void main(String[] args) throws Exception {
    File owned = new File(args[1]).getCanonicalFile();
    File noBackup = new File(owned, "no_backup"), files = new File(owned, "files");
    require(noBackup.mkdirs() && files.mkdirs(), "owned roots");
    android.content.Context.noBackup = noBackup;
    android.content.Context.files = files;
    HoyaNativeTransport.admit = true;
    JSONObject.strings.put("kind", args[0].equals("memory") ? "memory" : "native");
    JSONObject.strings.put("directory", "@hoyadb/default");
    JSONObject.strings.put("name", "store");
    HoyaDBModule module = new HoyaDBModule(new ReactApplicationContext());
    Settled first = new Settled();
    if (args[0].equals("memory")) {
      module.open("{}", key(0), first);
      eventually(() -> HoyaNativeTransport.opens.get() == 1, "memory open not submitted");
      require(BackupPolicyLock.acquired.get() == 0 && BackupPolicyLock.checks.get() == 0, "memory open touched the backup policy");
      HoyaNativeTransport.completeOpen();
      eventually(() -> first.outcome.get() != null, "memory open unsettled");
    } else if (args[0].equals("held")) {
      module.open("{}", key(32), first);
      eventually(() -> HoyaNativeTransport.opens.get() == 1, "protected open not submitted");
      // The stub excludes backups: the check ran once, under the lock, before the open.
      require(BackupPolicyLock.checked.equals(java.util.Arrays.asList("store:true:1:0")), "opposite-parent check not made under the lock: " + BackupPolicyLock.checked);
      require(new File(noBackup, "hoyadb").isDirectory(), "selected child not prepared");
      require(BackupPolicyLock.released.get() == 0, "policy lock released before the key-bearing open replied");
      HoyaNativeTransport.completeOpen();
      eventually(() -> first.outcome.get() != null, "open unsettled");
      require("resolved".equals(first.outcome.get()), "open did not resolve");
      require(BackupPolicyLock.acquired.get() == 1 && BackupPolicyLock.released.get() == 1, "policy lock not released exactly once on reply");
    } else if (args[0].equals("conflict")) {
      BackupPolicyLock.conflict = "BackupPolicyFailure";
      module.open("{}", key(32), first);
      eventually(() -> first.outcome.get() != null, "conflicting open unsettled");
      require("BackupPolicyFailure".equals(first.outcome.get()), "conflict must reject BackupPolicyFailure, not " + first.outcome.get());
      require(HoyaNativeTransport.opens.get() == 0, "conflicting open reached the key-bearing native open");
      require(!new File(noBackup, "hoyadb").exists(), "a policy-rejected open created the store directory");
      require(BackupPolicyLock.acquired.get() == 1 && BackupPolicyLock.released.get() == 1, "policy lock not released on rejection");
    } else if (args[0].equals("failed")) {
      module.open("{}", key(32), first);
      eventually(() -> HoyaNativeTransport.opens.get() == 1, "protected open not submitted");
      HoyaNativeTransport.failOpen();
      eventually(() -> first.outcome.get() != null, "failed open unsettled");
      require("BoundaryFailure".equals(first.outcome.get()), "native failure kind changed");
      require(BackupPolicyLock.released.get() == 1, "policy lock not released on a failed reply");
    } else if (args[0].equals("queued")) {
      Settled second = new Settled();
      module.open("{}", key(32), first);
      eventually(() -> HoyaNativeTransport.opens.get() == 1, "first open not submitted");
      module.open("{}", key(32), second);
      Thread.sleep(100);
      require(HoyaNativeTransport.opens.get() == 1 && BackupPolicyLock.acquired.get() == 1, "second open did not wait for the first reply");
      // The first reply must release the lock without needing the module's blocked policy thread.
      HoyaNativeTransport.completeOpen();
      eventually(() -> HoyaNativeTransport.opens.get() == 2, "second open deadlocked behind the first reply");
      HoyaNativeTransport.completeOpen();
      eventually(() -> first.outcome.get() != null && second.outcome.get() != null, "opens unsettled");
      require(BackupPolicyLock.acquired.get() == 2 && BackupPolicyLock.released.get() == 2, "locks not balanced");
    } else {
      throw new AssertionError("unknown case");
    }
    module.invalidate();
    System.out.println("PASS " + args[0]);
    System.exit(0);
  }
}
