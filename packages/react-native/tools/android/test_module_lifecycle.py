#!/usr/bin/env python3
"""DEF-0047 / TC-0050: run the actual RN module on a host JVM with controlled replies.

The Android, React Native, JSON and native transport stand-ins only supply the
module's dependencies. These tests exercise Java lifecycle interleavings, not
JNI, Android devices, the JSON parser or React Native integration.

Run: python3 -I packages/react-native/tools/android/test_module_lifecycle.py
Requires javac/java on PATH; no downloaded dependencies or shared stores.
"""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
MODULE = HERE.parents[1] / 'android/src/main/java/dev/hoyadb/HoyaDBModule.java'
STUBS = {
    'android/os/SystemClock.java': '''package android.os;
public class SystemClock { public static long elapsedRealtime() { return 0; } }''',
    'android/content/Context.java': '''package android.content;
public class Context {
  public static volatile java.io.File noBackup = new java.io.File("/tmp"), files = new java.io.File("/tmp");
  public java.io.File getNoBackupFilesDir() { return noBackup; }
  public java.io.File getFilesDir() { return files; }
}''',
    'com/facebook/react/bridge/ReactApplicationContext.java': '''package com.facebook.react.bridge;
public class ReactApplicationContext extends android.content.Context {}''',
    # SPEC-0036 item 6 stand-in for the shared AAR lock: a real one-permit semaphore, so a release
    # that needed the module's own policy thread would deadlock the next open here too.
    'dev/hoyadb/HoyaException.java': '''package dev.hoyadb;
public class HoyaException extends RuntimeException {
  private final String kind;
  public HoyaException(String kind) { super(kind); this.kind = kind; }
  public String getKind() { return kind; }
}''',
    'dev/hoyadb/BackupPolicyLock.java': '''package dev.hoyadb;
import java.util.concurrent.Semaphore;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
public final class BackupPolicyLock {
  public static final AtomicInteger acquired = new AtomicInteger(), released = new AtomicInteger(), checks = new AtomicInteger();
  public static final java.util.List<String> checked = new java.util.concurrent.CopyOnWriteArrayList<>();
  public static volatile String conflict;
  private static final Semaphore process = new Semaphore(1);
  public static final class Held implements AutoCloseable {
    private final AtomicBoolean held = new AtomicBoolean(true);
    public void close() { if (held.getAndSet(false)) { released.incrementAndGet(); process.release(); } }
  }
  public static Held acquire(android.content.Context context) {
    process.acquireUninterruptibly();
    acquired.incrementAndGet();
    return new Held();
  }
  public static void requireNoOppositeStore(android.content.Context context, String name, boolean excluded) {
    checks.incrementAndGet(); checked.add(name + ":" + excluded + ":" + acquired.get() + ":" + released.get());
    if (conflict != null) throw new HoyaException(conflict);
  }
}''',
    'com/facebook/react/bridge/Promise.java': '''package com.facebook.react.bridge;
public interface Promise { void reject(String code, String message); void resolve(Object value); }''',
    'com/facebook/react/bridge/ReadableArray.java': '''package com.facebook.react.bridge;
public interface ReadableArray { int size(); ReadableType getType(int i); double getDouble(int i); }''',
    'com/facebook/react/bridge/ReadableType.java': '''package com.facebook.react.bridge;
public enum ReadableType { Number }''',
    'com/facebook/react/bridge/WritableArray.java': '''package com.facebook.react.bridge;
public interface WritableArray { void pushInt(int value); }''',
    'com/facebook/react/bridge/Arguments.java': '''package com.facebook.react.bridge;
public class Arguments { public static WritableArray createArray() { return value -> {}; } }''',
    'org/json/JSONObject.java': '''package org.json;
public class JSONObject {
  public JSONObject() {} public JSONObject(String input) {}
  public JSONObject put(String key, Object value) { return this; }
  public static final java.util.Map<String, String> strings = new java.util.concurrent.ConcurrentHashMap<>();
  public boolean getBoolean(String key) { return true; }
  public String getString(String key) { return strings.getOrDefault(key, "owner1"); }
  public String optString(String key) { return "close"; }
  public JSONObject getJSONObject(String key) { return this; }
  public String toString() { return "{}"; }
}''',
    'dev/hoyadb/NativeHoyaDBSpec.java': '''package dev.hoyadb;
import com.facebook.react.bridge.*;
public abstract class NativeHoyaDBSpec {
  private final ReactApplicationContext context;
  protected Object mEventEmitterCallback;
  public NativeHoyaDBSpec(ReactApplicationContext value) { context = value; }
  public ReactApplicationContext getReactApplicationContext() { return context; }
  protected void emitOnAdmissionReleased() {}
  public abstract void open(String input, ReadableArray key, Promise promise);
  public abstract void call(String input, Promise promise);
  public abstract void namespace(Promise promise);
  public abstract String uptime();
  public void invalidate() {}
}''',
    'dev/hoyadb/HoyaNativeTransport.java': '''package dev.hoyadb;
import java.util.concurrent.Executor;
import java.util.concurrent.atomic.AtomicInteger;
public class HoyaNativeTransport {
  public interface Reply { void onSuccess(byte[] value); void onFailure(String value); }
  public static final AtomicInteger closes = new AtomicInteger();
  public static final AtomicInteger opens = new AtomicInteger();
  public static final AtomicInteger releases = new AtomicInteger();
  private static Reply pending;
  public static AutoCloseable subscribeRelease(Executor executor, Runnable callback) {
    return () -> releases.incrementAndGet();
  }
  public static volatile boolean admit;
  public static void open(byte[] frame, byte[] key, Reply reply) { pending = reply; opens.incrementAndGet(); }
  public static void completeOpen() { pending.onSuccess("{}".getBytes(java.nio.charset.StandardCharsets.UTF_8)); }
  public static void failOpen() { pending.onFailure("BoundaryFailure"); }
  public static void call(byte[] frame, Reply reply) { closes.incrementAndGet(); reply.onSuccess(new byte[0]); }
  public static void validateOpen(byte[] frame, int length, Reply reply) {
    if (!admit) throw new AssertionError("retired admission");
    reply.onSuccess("{}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
  }
}''',
}


class ModuleLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scratch = tempfile.TemporaryDirectory(prefix='hoyadb-rn-lifecycle-')
        cls.addClassCleanup(cls.scratch.cleanup)
        root = Path(cls.scratch.name)
        for name, source in STUBS.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(source)
        shutil.copyfile(MODULE, root / 'dev/hoyadb/HoyaDBModule.java')
        shutil.copyfile(HERE / 'testdata/ModuleLifecycleProbe.java', root / 'dev/hoyadb/ModuleLifecycleProbe.java')
        shutil.copyfile(HERE / 'testdata/ModulePolicyProbe.java', root / 'dev/hoyadb/ModulePolicyProbe.java')
        cls.classes = root / 'classes'
        subprocess.run(['javac', '-d', str(cls.classes), *map(str, sorted(root.rglob('*.java')))],
                       check=True, capture_output=True, text=True, timeout=60)

    def check_case(self, case, probe='ModuleLifecycleProbe'):
        with tempfile.TemporaryDirectory(prefix='hoyadb-rn-policy-') as owned:
            result = subprocess.run(['java', '-cp', str(self.classes), 'dev.hoyadb.' + probe, case, owned],
                                    capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    # SPEC-0036 item 6 / API-0011: the RN Android host shares the Kotlin SDK's BackupPolicyLock.
    def test_protected_open_holds_policy_lock_from_check_until_reply(self):
        self.check_case('held', 'ModulePolicyProbe')

    def test_opposite_store_rejects_before_directory_or_native_open(self):
        self.check_case('conflict', 'ModulePolicyProbe')

    def test_failed_open_reply_releases_policy_lock(self):
        self.check_case('failed', 'ModulePolicyProbe')

    def test_second_open_waits_for_first_reply_without_deadlock(self):
        self.check_case('queued', 'ModulePolicyProbe')

    def test_memory_open_takes_no_policy_lock(self):
        self.check_case('memory', 'ModulePolicyProbe')

    def test_open_after_retirement_never_reaches_native_admission(self):
        self.check_case('retired')

    def test_pending_native_open_closes_late_owner_and_rejects(self):
        self.check_case('pending')

    def test_invalidation_during_publication_never_leaks_or_resolves_retired(self):
        self.check_case('publishing')

    def test_invalidation_after_publication_closes_once(self):
        self.check_case('published')


if __name__ == '__main__':
    unittest.main()
