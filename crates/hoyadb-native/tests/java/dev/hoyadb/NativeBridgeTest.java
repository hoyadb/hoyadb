package dev.hoyadb;

import java.nio.charset.StandardCharsets;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.concurrent.atomic.AtomicReference;

/** Same source runs with existing host JDK and existing Android platform runtime. */
public final class NativeBridgeTest {
  private static byte[] bytes(String text) { return text.getBytes(StandardCharsets.UTF_8); }
  private static String text(byte[] value) {
    if (value == null) throw new AssertionError("native returned null without an exception");
    return new String(value, StandardCharsets.UTF_8);
  }
  private static void check(boolean value) { if (!value) throw new AssertionError("JNI contract"); }
  private static String frame(String op, String owner, String args) {
    return "{\"abi\":\"1\",\"op\":\""+op+"\",\"owner\":"+owner+",\"args\":"+args+"}";
  }
  private static String open(String table, String column, String namespace) {
    String schema="{\"version\":\"1\",\"tables\":{\""+table+"\":{\"columns\":{\""+column+"\":{\"kind\":\"String\",\"optional\":false}},\"relations\":{}}}}";
    String args="{\"engine\":{\"kind\":\"memory\",\"store\":null},\"schema\":"+schema+",\"migrations\":[],\"namespace\":\""+namespace+"\",\"options\":{\"capacity\":\"1000\",\"history\":\"100\",\"initial_delay_ms\":\"1000\",\"multiplier\":\"2\",\"max_delay_ms\":\"60000\"}}";
    return frame("open","null",args);
  }
  private static String owner(String reply) {
    Matcher match=Pattern.compile("\\\"value\\\":\\\"([0-9]+)\\\"").matcher(reply);
    check(reply.contains("\"ok\":true") && match.find());
    return "\""+match.group(1)+"\"";
  }
  private static void invalid(Runnable work) {
    try { work.run(); throw new AssertionError("missing static IllegalArgumentException"); }
    catch (IllegalArgumentException expected) { check(expected.getMessage().equals("InvalidRequest")); }
  }
  public static void main(String[] args) throws Exception {
    check(NativeBridge.abiVersion()==1);
    String original=open("preflight","name","66666666666666666666666666666666");
    check(text(NativeBridge.validateOpen(bytes(original),0)).contains("\"value\":null"));
    check(text(NativeBridge.validateOpen(bytes(original.replace("\"abi\":\"1\"", "\"abi\":\"1\",\"abi\":\"1\"")),0)).contains("\"ok\":false"));
    check(text(NativeBridge.validateOpen(bytes(original.replace("\"version\":\"1\"", "\"version\":1")),0)).contains("\"ok\":false"));
    invalid(()->NativeBridge.validateOpen(null,0));
    invalid(()->NativeBridge.validateOpen(bytes(original),-1));
    invalid(()->NativeBridge.validateOpen(bytes(original),31));
    invalid(()->NativeBridge.validateOpen(new byte[]{(byte)0xff},0));
    String first=owner(text(NativeBridge.open(bytes(open("galaxies","name","64646464646464646464646464646464")),new byte[0])));
    byte[] snapshot=NativeBridge.call(bytes(frame("schema",first,"{}")));
    String expected=text(snapshot);
    check(expected.contains("galaxies") && expected.contains("name"));
    String second=owner(text(NativeBridge.open(bytes(open("concertos","title","65656565656565656565656565656565")),new byte[0])));
    check(text(NativeBridge.call(bytes(frame("schema",second,"{}")))).contains("concertos"));
    check(text(snapshot).equals(expected)); // The returned array owns its bytes.
    AtomicReference<Throwable> failure=new AtomicReference<>();
    Thread[] workers=new Thread[4];
    for(int i=0;i<workers.length;i++) {
      workers[i]=new Thread(()->{ try { for(int n=0;n<8;n++) check(text(NativeBridge.call(bytes(frame("schema",first,"{}")))).equals(expected)); } catch(Throwable error) { failure.set(error); } });
      workers[i].start();
    }
    for(Thread worker:workers) worker.join();
    if(failure.get()!=null) throw new AssertionError("managed thread JNI ownership",failure.get());
    invalid(()->NativeBridge.call(null));
    invalid(()->NativeBridge.call(new byte[]{(byte)0xff}));
    invalid(()->NativeBridge.call(new byte[1048577]));
    invalid(()->NativeBridge.open(new byte[0],new byte[31]));
    check(text(NativeBridge.call(new byte[0])).contains("InvalidPayload"));
    check(text(NativeBridge.call(bytes(frame("close",first,"{}")))).contains("\"ok\":true"));
    check(text(NativeBridge.call(bytes(frame("schema",first,"{}")))).contains("InvalidHandle"));
    check(text(NativeBridge.call(bytes(frame("close",second,"{}")))).contains("\"ok\":true"));
    System.out.println("HOYA_JNI_GENERAL_GREEN schemas=2 threads=4 calls=32 static_errors=8 owned_replies=true pure_preflight=true");
  }
}
