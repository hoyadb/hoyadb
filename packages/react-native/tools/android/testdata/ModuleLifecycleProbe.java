package dev.hoyadb;

import com.facebook.react.bridge.*;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.AbstractSet;
import java.util.Iterator;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

/** Deterministic host-only interleavings in the unchanged production module. */
public final class ModuleLifecycleProbe {
  private static void require(boolean condition, String message) {
    if (!condition) throw new AssertionError(message);
  }

  private static void await(CountDownLatch latch) throws InterruptedException {
    require(latch.await(5, TimeUnit.SECONDS), "lifecycle gate timeout");
  }

  private static Object field(HoyaDBModule module, String name) throws Exception {
    Field field = HoyaDBModule.class.getDeclaredField(name);
    field.setAccessible(true);
    return field.get(module);
  }

  private static final class ReplyPromise implements Promise {
    final AtomicInteger resolved = new AtomicInteger(), rejected = new AtomicInteger();
    final AtomicInteger retiredSuccess = new AtomicInteger();
    final CountDownLatch done = new CountDownLatch(1);
    final AtomicBoolean live;
    volatile String error;

    ReplyPromise(HoyaDBModule module) throws Exception {
      live = (AtomicBoolean) field(module, "live");
    }

    public void resolve(Object value) {
      if (!live.get()) retiredSuccess.incrementAndGet();
      resolved.incrementAndGet();
      done.countDown();
    }

    public void reject(String code, String message) {
      error = code;
      rejected.incrementAndGet();
      done.countDown();
    }

    void check(int successes, int failures) throws Exception {
      await(done);
      require(resolved.get() == successes && rejected.get() == failures, "unexpected promise settlement");
      require(retiredSuccess.get() == 0, "open resolved after module retirement");
      if (failures > 0) require("Closed".equals(error), "retired open must reject Closed");
    }
  }

  // Pause at the actual registration boundary without changing production code.
  private static final class HeldSet extends AbstractSet<String> {
    final Set<String> backing = ConcurrentHashMap.newKeySet();
    final CountDownLatch adding = new CountDownLatch(1), resume = new CountDownLatch(1);
    public boolean add(String owner) {
      adding.countDown();
      try { await(resume); } catch (InterruptedException error) { throw new AssertionError(error); }
      return backing.add(owner);
    }
    public Iterator<String> iterator() { return backing.iterator(); }
    public int size() { return backing.size(); }
    public void clear() { backing.clear(); }
  }

  private static void begin(HoyaDBModule module, ReplyPromise promise) throws Exception {
    Method method = HoyaDBModule.class.getDeclaredMethod("openAccepted", String.class, byte[].class, Promise.class);
    method.setAccessible(true);
    method.invoke(module, "{}", new byte[0], promise);
  }

  private static void awaitBlockedOrFinished(Thread thread) {
    long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
    while (thread.isAlive() && thread.getState() != Thread.State.BLOCKED) {
      require(System.nanoTime() < deadline, "invalidate neither completed nor waited for publication");
      Thread.yield();
    }
  }

  public static void main(String[] args) throws Exception {
    HoyaDBModule module = new HoyaDBModule(new ReactApplicationContext());
    ReplyPromise promise = new ReplyPromise(module);
    if (args[0].equals("retired")) {
      module.invalidate();
      module.open("{}", new ReadableArray() {
        public int size() { return 0; }
        public ReadableType getType(int index) { throw new AssertionError(); }
        public double getDouble(int index) { throw new AssertionError(); }
      }, promise);
      promise.check(0, 1);
      require(HoyaNativeTransport.opens.get() == 0, "retired module opened an owner");
      require(HoyaNativeTransport.closes.get() == 0, "retired module fabricated an owner");
    } else if (args[0].equals("pending")) {
      begin(module, promise);
      module.invalidate();
      HoyaNativeTransport.completeOpen();
      promise.check(0, 1);
      require(HoyaNativeTransport.closes.get() == 1, "late owner not closed exactly once");
    } else if (args[0].equals("publishing")) {
      HeldSet owners = new HeldSet();
      Field slot = HoyaDBModule.class.getDeclaredField("owners");
      slot.setAccessible(true);
      slot.set(module, owners);
      begin(module, promise);
      HoyaNativeTransport.completeOpen();
      await(owners.adding);
      Thread invalidator = new Thread(module::invalidate);
      invalidator.start();
      try { awaitBlockedOrFinished(invalidator); } finally { owners.resume.countDown(); }
      invalidator.join(5000);
      require(!invalidator.isAlive(), "invalidate did not complete");
      await(promise.done);
      require(HoyaNativeTransport.closes.get() == 1, "publication race leaked or double-closed its owner");
      require(owners.isEmpty(), "retired module retained an owner");
      require(promise.resolved.get() + promise.rejected.get() == 1, "open settled more than once");
      require(promise.retiredSuccess.get() == 0, "open resolved after module retirement");
    } else if (args[0].equals("published")) {
      begin(module, promise);
      HoyaNativeTransport.completeOpen();
      promise.check(1, 0);
      module.invalidate();
      require(HoyaNativeTransport.closes.get() == 1, "published owner not closed exactly once");
    } else {
      throw new AssertionError("unknown case");
    }
    module.invalidate();
    require(((Set<?>) field(module, "owners")).isEmpty(), "retired owner remains registered");
    require(HoyaNativeTransport.releases.get() == 1, "release subscription not closed exactly once");
    require(HoyaNativeTransport.closes.get() == (args[0].equals("retired") ? 0 : 1), "repeated retirement double-closed owner");
    System.out.println("PASS " + args[0]);
  }
}
