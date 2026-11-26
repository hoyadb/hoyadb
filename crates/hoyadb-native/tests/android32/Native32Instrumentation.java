package dev.hoyadb;

import android.app.Instrumentation;
import android.content.Context;
import android.os.Bundle;
import android.os.Process;
import android.system.Os;
import android.system.OsConstants;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.util.Arrays;
import org.json.JSONArray;
import org.json.JSONObject;

/** TC-0036 G23: actual packaged JNI, disposable app files and public test keys only. */
public final class Native32Instrumentation extends Instrumentation {
  private String phase;
  private String abi;
  private String owner;

  @Override public void onCreate(Bundle args) {
    super.onCreate(args);
    phase = args.getString("phase");
    abi = args.getString("abi");
    start();
  }

  private static JSONObject object(Object... values) throws Exception {
    JSONObject result = new JSONObject();
    for (int i = 0; i < values.length; i += 2)
      result.put((String) values[i], values[i + 1] == null ? JSONObject.NULL : values[i + 1]);
    return result;
  }
  private static JSONArray array(Object... values) {
    JSONArray result = new JSONArray();
    for (Object value : values) result.put(value);
    return result;
  }
  private static JSONObject cell(String kind, String value) throws Exception {
    return object("kind", kind, "value", value);
  }
  private static void check(boolean result, String label) {
    if (!result) throw new AssertionError(label);
  }
  private static byte[] read(File file) throws Exception {
    ByteArrayOutputStream output = new ByteArrayOutputStream();
    try (FileInputStream input = new FileInputStream(file)) {
      byte[] buffer = new byte[4096]; int count;
      while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
    }
    return output.toByteArray();
  }
  private static void write(File file, JSONObject value) throws Exception {
    try (FileOutputStream output = new FileOutputStream(file)) {
      output.write(value.toString().getBytes("UTF-8")); output.getFD().sync();
    }
  }
  private static JSONObject reply(byte[] value) throws Exception {
    return new JSONObject(new String(value, "UTF-8"));
  }
  private JSONObject raw(String operation, JSONObject args) throws Exception {
    byte[] frame = object("abi", "1", "op", operation, "owner", owner, "args", args).toString().getBytes("UTF-8");
    return reply(NativeBridge.call(frame));
  }
  private Object call(String operation, JSONObject args) throws Exception {
    JSONObject response = raw(operation, args);
    check(response.getBoolean("ok"), operation + " rejected: " + response.optJSONObject("error"));
    return response.get("value");
  }
  private static JSONObject column(String kind) throws Exception {
    return object("kind", kind, "optional", false);
  }
  private static JSONObject schema() throws Exception {
    JSONObject notes = object("columns", object("label", column("String")), "relations", object());
    JSONObject children = object("columns", object("label", column("String"), "large", column("Integer")),
        "relations", object("parent", object("kind", "ToOne", "table", "notes", "foreign_key", "note_id", "optional", false)));
    JSONObject markers = object("columns", object("label", column("String")), "relations", object());
    return object("version", "1", "tables", object("notes", notes, "children", children, "markers", markers));
  }
  private static byte[] key() {
    byte[] key = new byte[32];
    for (int i = 0; i < key.length; i++) key[i] = (byte) (i * 7);
    return key;
  }
  private byte[] openFrame(File directory) throws Exception {
    return object("abi", "1", "op", "open", "owner", null, "args", object(
        "engine", object("kind", "native", "directory", directory.getCanonicalPath(), "name", "sealed.sqlite", "backup_excluded", true),
        "schema", schema(), "migrations", array(), "namespace", "32323232323232323232323232323232",
        "options", object("capacity", "1000", "history", "100", "initial_delay_ms", "1000", "multiplier", "2", "max_delay_ms", "60000")))
        .toString().getBytes("UTF-8");
  }
  private void open(File directory) throws Exception {
    JSONObject opened = reply(NativeBridge.open(openFrame(directory), key()));
    check(opened.getBoolean("ok"), "protected open");
    owner = opened.getString("value");
    call("entity_configure_durable", object("config", object("scope", "android32", "tables", array("notes", "children"))));
  }
  private void close() throws Exception { call("close", object()); owner = null; }
  private JSONObject context() throws Exception { return object("identity", object("kind", "null"), "now_ms", "1000"); }
  private JSONObject batch(JSONArray writes) throws Exception {
    return (JSONObject) call("entity_batch_aliased", object("writes", writes, "context", context()));
  }
  private JSONObject insert(String table, String alias, String id, JSONObject fields, JSONObject references) throws Exception {
    return object("op", "insert", "table", table, "alias", alias, "proposed_id", id, "fields", fields, "references", references);
  }
  private JSONArray fetch(String table) throws Exception {
    return (JSONArray) call("fetch", object("query", object("table", table, "predicates", array(), "order", null, "limit", null, "source", null)));
  }
  private void live(String count) throws Exception {
    check(((JSONObject) call("queue_status", object())).getString("live").equals(count), "pending write count");
  }
  private void resolve(String table, String id, String expected) throws Exception {
    check(call("resolve_entity_id", object("table", table, "id", id)).equals(expected), "durable alias");
  }
  private JSONObject push(String recordId) throws Exception {
    JSONArray envelopes = ((JSONObject) call("prepare_push", object("now_ms", "1000", "jitter_basis_points", "0"))).getJSONArray("envelopes");
    check(envelopes.length() == 1, "dependency ordering");
    JSONObject envelope = new JSONObject(envelopes.getString(0));
    check(envelope.getString("recordId").equals(recordId), "canonical push identity");
    return envelope;
  }
  private void settle(JSONObject envelope, JSONObject mappings) throws Exception {
    String key = envelope.getString("writeId");
    call("receive", object("key", key, "outcome", object("status", "applied", "result", object("kind", "null"))));
    call("reconcile", object("key", key, "plan", object("writes", array(), "mappings", mappings)));
    call("forget", object("key", key));
  }
  private void verifyChild(String id, String parent) throws Exception {
    JSONArray rows = fetch("children"); check(rows.length() == 1, "one child");
    JSONObject row = rows.getJSONObject(0); check(row.getString("id").equals(id), "child identity");
    JSONObject fields = row.getJSONObject("fields");
    check(fields.getJSONObject("label").getString("value").equals("patched"), "partial patch");
    check(fields.getJSONObject("large").getString("value").equals("9223372036854775807"), "i64 on32bit");
    check(fields.getJSONObject("note_id").getString("value").equals(parent), "remapped reference");
  }
  private void protectedBytes(File directory) throws Exception {
    byte[] original = read(new File(directory, "sealed.sqlite"));
    check(original.length > 16 && !Arrays.equals(Arrays.copyOf(original, 16), "SQLite format 3\0".getBytes("UTF-8")), "cipher header");
    byte[] wrong = key(); wrong[0] ^= 127;
    JSONObject rejected = reply(NativeBridge.open(openFrame(directory), wrong));
    check(!rejected.getBoolean("ok") && rejected.getJSONObject("error").getString("kind").equals("BackendFailure"), "wrong key rejection");
    check(Arrays.equals(original, read(new File(directory, "sealed.sqlite"))), "wrong key preserved bytes");
  }

  @Override public void onStart() {
    Bundle result = new Bundle();
    try {
      check(!Process.is64Bit(), "must execute in32bit process");
      check(abi.equals("x86") || abi.equals("armeabi-v7a"), "explicit32bit ABI");
      if (!phase.equals("coexistence")) check(NativeBridge.abiVersion() == 1, "actual ABI1");
      Context app = getTargetContext();
      File directory = new File(app.getNoBackupFilesDir(), "hoya32");
      File stateFile = new File(app.getNoBackupFilesDir(), "probe-state.json");
      if (phase.equals("coexistence")) {
        // Reuse the consumer-provided default-visible SQLite probe when supplied.
        // Its static initializer loads the plaintext provider before Hoya's JNI class.
        Class.forName("dev.hoyadb.CoexistenceProbe").getMethod("run", Context.class).invoke(null, app);
      } else if (phase.equals("seed")) {
        check(!directory.exists() && directory.mkdir(), "fresh owned store"); open(directory);
        JSONObject created = batch(array(
            insert("notes", "parent", null, object("label", cell("string", "parent")), object()),
            insert("children", "child", null, object("label", cell("string", "initial"), "large", cell("integer", "9223372036854775807")), object("note_id", "parent"))));
        JSONObject aliases = created.getJSONObject("aliases");
        JSONObject bad = raw("entity_batch_aliased", object("context", context(), "writes", array(
            insert("markers", null, "marker", object("label", cell("string", "ready")), object()),
            object("op", "patch", "table", "children", "id", "absent", "fields", object("label", cell("string", "bad")), "references", object()))));
        check(!bad.getBoolean("ok") && bad.getJSONObject("error").getString("kind").equals("RowNotFound"), "atomic failure");
        check(fetch("markers").length() == 0, "no partial import marker");
        batch(array(object("op", "patch", "table", "children", "id", aliases.getString("child"), "fields", object("label", cell("string", "patched")), "references", object()),
            insert("markers", null, "marker", object("label", cell("string", "ready")), object())));
        verifyChild(aliases.getString("child"), aliases.getString("parent")); live("3");
        write(stateFile, aliases); close(); protectedBytes(directory);
      } else {
        JSONObject aliases = new JSONObject(new String(read(stateFile), "UTF-8"));
        protectedBytes(directory); open(directory);
        String parent = aliases.getString("parent"), child = aliases.getString("child");
        check(fetch("markers").length() == 1, "atomic marker retained");
        if (phase.equals("parent")) {
          live("3"); verifyChild(child, parent);
          settle(push(parent), object(parent, "canonical-parent"));
          resolve("notes", parent, "canonical-parent"); verifyChild(child, "canonical-parent"); live("2");
        } else if (phase.equals("child")) {
          resolve("notes", parent, "canonical-parent"); live("2"); verifyChild(child, "canonical-parent");
          settle(push(child), object(child, "canonical-child"));
          resolve("children", child, "canonical-child"); verifyChild("canonical-child", "canonical-parent"); live("1");
        } else if (phase.equals("restart")) {
          resolve("notes", parent, "canonical-parent"); resolve("children", child, "canonical-child");
          verifyChild("canonical-child", "canonical-parent"); live("1");
          settle(push("canonical-child"), object()); live("0");
          close(); protectedBytes(directory); open(directory); live("0");
          resolve("notes", parent, "canonical-parent"); resolve("children", child, "canonical-child");
        } else throw new AssertionError("unknown phase");
        close(); protectedBytes(directory);
      }
      result.putString("status", "PASS " + phase);
      result.putString("details", object("abi", abi, "pid", Process.myPid(), "is64Bit", Process.is64Bit(),
          "pageSize", Os.sysconf(OsConstants._SC_PAGESIZE), "osArch", System.getProperty("os.arch")).toString());
      finish(0, result);
    } catch (Throwable error) {
      result.putString("status", "FAIL " + phase + " " + error.getClass().getSimpleName() + ": " + error.getMessage());
      finish(1, result);
    }
  }
}
