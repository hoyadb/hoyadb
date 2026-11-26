// TC-0036 G16: run inside the ordinary task-owned Android consumer process.
package dev.hoyadb.consumer;

import android.content.Context;
import android.content.ContextWrapper;
import com.facebook.react.bridge.Promise;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.bridge.ReadableArray;
import com.facebook.react.bridge.ReadableType;
import dev.hoyadb.HoyaDBModule;
import java.io.File;
import java.lang.reflect.Proxy;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;

public final class PolicyProbe {
  private static final class Roots extends ContextWrapper {
    int calls;
    Roots(Context base) { super(base); }
    @Override public File getNoBackupFilesDir() { calls++; return super.getNoBackupFilesDir(); }
    @Override public File getFilesDir() { calls++; return super.getFilesDir(); }
  }
  private static final class Answer {
    final CountDownLatch done=new CountDownLatch(1);
    volatile String value;
    volatile String error;
    Promise promise() {
      return (Promise)Proxy.newProxyInstance(Promise.class.getClassLoader(),new Class<?>[]{Promise.class},(self,method,args)->{
        if(method.getName().equals("resolve")){value=(String)args[0];done.countDown();}
        else if(method.getName().equals("reject")){error=args[0] instanceof String?(String)args[0]:"BoundaryFailure";done.countDown();}
        return null;
      });
    }
    void waitFor() throws Exception { check(done.await(20,TimeUnit.SECONDS),"bounded native reply"); }
    boolean rejected() throws Exception {
      waitFor();
      if(error!=null){check(error.equals("InvalidRequest"),"static validation rejection, not boundary failure");return true;}
      JSONObject reply=new JSONObject(value);
      if(reply.getBoolean("ok"))return false;
      String kind=reply.getJSONObject("error").getString("kind");
      check(kind.equals("InvalidRequest")||kind.equals("InvalidPayload")||kind.equals("InvalidSchema"),"wire validation rejection");
      return true;
    }
  }
  private static ReadableArray key(int size) {
    return (ReadableArray)Proxy.newProxyInstance(ReadableArray.class.getClassLoader(),new Class<?>[]{ReadableArray.class},(self,method,args)->{
      if(method.getName().equals("size"))return size;
      if(method.getName().equals("getType"))return ReadableType.Number;
      if(method.getName().equals("getDouble")){int index=(Integer)args[0];return (double)(index==31?255:index);}
      throw new UnsupportedOperationException("Test byte-array method");
    });
  }
  private static void check(boolean value,String label) { if(!value)throw new AssertionError(label); }
  private static String frame(String name,String root) throws Exception {
    JSONObject column=new JSONObject().put("kind","String").put("optional",false);
    JSONObject table=new JSONObject().put("columns",new JSONObject().put("label",column)).put("relations",new JSONObject());
    JSONObject schema=new JSONObject().put("version","1").put("tables",new JSONObject().put("comets",table));
    JSONObject engine=new JSONObject().put("kind","native").put("directory",root).put("name",name).put("backup_excluded",true);
    JSONObject options=new JSONObject().put("capacity","1000").put("history","100").put("initial_delay_ms","1000").put("multiplier","2").put("max_delay_ms","60000");
    JSONObject args=new JSONObject().put("engine",engine).put("schema",schema).put("migrations",new org.json.JSONArray()).put("namespace","98989898989898989898989898989898").put("options",options);
    return new JSONObject().put("abi","1").put("op","open").put("owner",JSONObject.NULL).put("args",args).toString();
  }
  public static void run(Context context) throws Exception {
    Roots roots=new Roots(context);
    HoyaDBModule module=new HoyaDBModule(new ReactApplicationContext(roots));
    roots.calls=0;
    try {
      String valid=frame("rn-policy-owned.sqlite","@hoyadb/default");
      String duplicate="{\"abi\":\"1\","+valid.substring(1);
      Answer bad=new Answer();module.open(duplicate,key(32),bad.promise());check(bad.rejected(),"duplicate original frame");check(roots.calls==0,"duplicate before OS getter");
      String malformed=valid.replace("rn-policy-owned.sqlite","bad"+Character.toString((char)0xD800));
      bad=new Answer();module.open(malformed,key(32),bad.promise());check(bad.rejected(),"malformed UTF16 frame rejected without replacement");check(roots.calls==0,"malformed original string before OS getter");
      bad=new Answer();module.open(valid,key(0),bad.promise());check(bad.rejected(),"missing key");check(roots.calls==0,"missing key before OS getter");
      for(String name:new String[]{"../outside","back\\slash","nul"+Character.toString((char)0)+"name"}){
        bad=new Answer();module.open(frame(name,"@hoyadb/default"),key(32),bad.promise());check(bad.rejected(),"component name rejected");check(roots.calls==0,"invalid name before OS getter");
      }
      bad=new Answer();module.open(frame("rn-policy-owned.sqlite",context.getNoBackupFilesDir().getCanonicalPath()),key(32),bad.promise());check(bad.rejected(),"arbitrary root rejected");check(roots.calls==0,"arbitrary root before OS getter");
      JSONObject memory=new JSONObject(valid);memory.getJSONObject("args").put("engine",new JSONObject().put("kind","memory").put("store",JSONObject.NULL));
      Answer memoryOpen=new Answer();module.open(memory.toString(),key(0),memoryOpen.promise());memoryOpen.waitFor();JSONObject memoryReply=new JSONObject(memoryOpen.value);check(memoryReply.getBoolean("ok"),"real memory open");check(roots.calls==0,"memory bypasses OS parent policy");
      Answer memoryClose=new Answer();module.call(new JSONObject().put("abi","1").put("op","close").put("owner",memoryReply.getString("value")).put("args",new JSONObject()).toString(),memoryClose.promise());memoryClose.waitFor();check(new JSONObject(memoryClose.value).getBoolean("ok"),"memory close");
      Answer opened=new Answer();module.open(valid,key(32),opened.promise());opened.waitFor();check(opened.error==null,"valid transport reply");JSONObject reply=new JSONObject(opened.value);check(reply.getBoolean("ok"),"protected native open");
      check(roots.calls>0,"selected OS parent checked");
      File parent=context.getNoBackupFilesDir().getCanonicalFile(),directory=new File(parent,"hoyadb");check(directory.isDirectory()&&directory.getCanonicalPath().equals(directory.getAbsolutePath()),"real no-backup canonical child");
      String owner=reply.getString("value");Answer closed=new Answer();module.call(new JSONObject().put("abi","1").put("op","close").put("owner",owner).put("args",new JSONObject()).toString(),closed.promise());closed.waitFor();check(new JSONObject(closed.value).getBoolean("ok"),"owned close");
    } finally { module.invalidate(); }
  }
}
