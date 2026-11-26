// SPEC-0036 / TC-0040 BK10: real channel + runtime; scripted TurboModule only.
import test from 'node:test';
import assert from 'node:assert/strict';
import {nativeChannel,nativeNamespace} from '../src/channel.js';
import {createRuntime,Database} from '../../client/src/index.js';
import {schemas} from '../../client/test/schemas.mjs';

const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const tick=()=>new Promise(resolve=>setImmediate(resolve));
const nativeError=code=>Object.assign(new Error('secret native path and key'),{code,userInfo:{secret:true},nativeStackAndroid:'secret native stack',cause:'secret cause'});
const native=overrides=>({open:()=>ok('1'),call:()=>ok(null),namespace:()=>Array(16).fill(0),onAdmissionReleased:()=>({remove(){}}),...overrides});
function redacted(actual,expected,original) {
 assert.ok(actual instanceof Error);
 assert.notEqual(actual,original);
 assert.equal(actual.kind,expected);
 assert.equal(actual.message,expected);
 assert.deepEqual(Object.keys(actual),['kind']);
 for(const field of ['code','userInfo','nativeStackAndroid','cause'])assert.equal(Object.hasOwn(actual,field),false);
 assert.ok(!actual.stack.includes('secret'),'native exception details are not copied');
 return true;
}
async function rejected(operation,error,synchronous=false) {
 const fail=()=>{if(synchronous)throw error;return Promise.reject(error);};
 const module=native({[operation]:fail});
 const channel=nativeChannel(module),runtime=createRuntime(channel);
 if(operation==='namespace')return nativeNamespace(module);
 if(operation==='open')return runtime.open({},new Uint8Array(32));
 return runtime.call('schema');
}

test('BK10 BackupPolicyFailure survives native code translation and the public kind allowlist',async()=>{
 const original=nativeError('BackupPolicyFailure');
 await assert.rejects(rejected('open',original),actual=>redacted(actual,'BackupPolicyFailure',original));
});

test('BK10 an opaque failure frame retains its already-public BackupPolicyFailure kind',async()=>{
 const frame=JSON.stringify({abi:'1',ok:false,error:{kind:'BackupPolicyFailure'}});
 const runtime=createRuntime(nativeChannel(native({open:()=>frame})));
 await assert.rejects(runtime.open({},new Uint8Array(32)),{kind:'BackupPolicyFailure',message:'BackupPolicyFailure'});
});

test('BK10 native rejection kinds are operation-specific on synchronous and Promise routes',async()=>{
 for(const synchronous of [false,true])for(const operation of ['open','call','namespace']) {
  for(const code of ['InvalidRequest','BackupPolicyFailure','Busy','Closed','BoundaryFailure','Unknown',null,42]) {
   const allowed=operation==='open'?['InvalidRequest','BackupPolicyFailure','Busy','Closed']:operation==='call'?['InvalidRequest','Closed']:['Closed'];
   const error=nativeError(code);
   await assert.rejects(rejected(operation,error,synchronous),actual=>redacted(actual,allowed.includes(code)?code:'BackendFailure',error));
  }
 }
});

test('BK10 unreadable native rejection codes are redacted without invoking other getters',async()=>{
 for(const synchronous of [false,true])for(const operation of ['open','call','namespace']) {
  const error=Object.defineProperties({}, {code:{get(){throw new Error('secret code getter');}},kind:{get(){throw new Error('kind must not be read');}},message:{get(){throw new Error('message must not be read');}}});
  await assert.rejects(rejected(operation,error,synchronous),actual=>redacted(actual,'BackendFailure',error));
 }
});

test('BK10 pre-native validation and successful opaque replies are preserved',async()=>{
 let opens=0;
 const channel=nativeChannel(native({open:()=>{opens++;return 'opaque reply';},call:input=>input}));
 assert.throws(()=>channel.open(42,new Uint8Array(32)),{kind:'InvalidRequest'});
 assert.throws(()=>channel.call(42),{kind:'InvalidRequest'});
 assert.equal(opens,0);
 assert.equal(await channel.open('exact frame',new Uint8Array(32)),'opaque reply');
 assert.equal(await channel.call('exact reply'),'exact reply');
 await assert.rejects(nativeNamespace(native({namespace:()=>[]})),{kind:'InvalidRequest'});
});

test('BK10 native Closed during a managed drain produces one Closed fact',async()=>{
 let drains=0;const work=[],reports=[];
 const module=native({call:input=>{const frame=JSON.parse(input);if(frame.op==='observe')return ok('11');if(frame.op==='drain'){drains++;throw nativeError('Closed');}return ok(null);}});
 const db=new Database(createRuntime(nativeChannel(module)),{namespace:'00'.repeat(16),monotonic:()=>0,timers:{set(fn){work.push(fn);return work.length;},clear(){}},onBackgroundError:fact=>reports.push(fact)});
 try {
  await db.open({engine:{kind:'memory',store:null},schema:schemas[1]});
  await db.query('sensors').observe(()=>{});
  assert.equal(work.length,1);work.shift()();await tick();await tick();
  assert.deepEqual(reports,[{source:'observation',kind:'Closed',token:null}]);
  assert.equal(drains,1);assert.equal(work.length,0,'no retry timer');
 }finally{await db.close();}
});

test('BK10 native call Busy is BackendFailure and does not wait for admission release',async()=>{
 let attempts=0;const reports=[];
 const module=native({call:input=>{const frame=JSON.parse(input);if(frame.op==='prepare_push'){attempts++;return Promise.reject(nativeError('Busy'));}return ok(null);}});
 const db=new Database(createRuntime(nativeChannel(module)),{namespace:'00'.repeat(16),clock:()=>0,jitter:()=>0,onBackgroundError:fact=>reports.push(fact)});
 let timeout;
 try {
  await db.open({engine:{kind:'memory',store:null},schema:schemas[1]});
  db.transport={push(){assert.fail('no envelope should be sent');}};
  const completed=await Promise.race([db.sync().then(()=>true),new Promise(resolve=>{timeout=setTimeout(()=>resolve(false),500);})]);
  assert.equal(completed,true,'native call Busy is not core admission Busy');
  assert.equal(attempts,1);
  assert.deepEqual(reports,[{source:'sync',kind:'BackendFailure',token:null}]);
 }finally{clearTimeout(timeout);await db.close();}
});
