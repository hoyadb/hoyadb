// TC-0040 OD03/OD07 follow-up (SPEC-0036 item 3 clarifications): host timer failures,
// contained asynchronous sinks, a closed kind allowlist and retired-invocation suppression.
import test from 'node:test';import assert from 'node:assert/strict';import {Database,createRuntime,j} from '../src/index.js';import {schemas} from './schemas.mjs';
const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const bad=kind=>JSON.stringify({abi:'1',ok:false,error:{kind}});
const rows=(subscription,n)=>({kind:'data',subscription,value:{kind:'rows',rows:Array.from({length:n},(_,i)=>({id:`r${i}`,fields:{label:j.string('x')}}))}});
const count=(subscription,n)=>({kind:'data',subscription,value:{kind:'count',count:String(n)}});
const terminal=(subscription,kind='LimitExceeded')=>({kind:'error',subscription,value:{kind}});
const settle=()=>new Promise(resolve=>setImmediate(resolve));
const committed={id:'n',fields:{label:j.string('n')}};
const timerFact={source:'timer',kind:'BackendFailure',token:null};
function timers({failSet=false,failClear=false}={}){let next=0;const pending=new Map();return {pending,
 set(work){if(failSet)throw new Error('host timer set secret');const id=++next;pending.set(id,work);return id;},
 clear(id){if(failClear)throw new Error('host timer clear secret');pending.delete(id);}};}
async function fixture({timer=timers(),pages=[],hold=null,drainFailure=null,sink}={}){
 const frames=[],reports=[];let handle=10;
 const runtime=createRuntime({open:()=>ok('1'),call:async input=>{const f=JSON.parse(input);frames.push(f);
  if(f.op==='observe')return ok(String(++handle));
  if(f.op==='unsubscribe')return ok(true);
  if(f.op==='drain'){if(hold)await hold.promise;if(drainFailure)return bad(drainFailure);return ok({events:pages.shift()??[],has_more:false});}
  if(f.op==='insert')return ok(committed);
  if(f.op==='request')return ok('5');if(f.op==='begin')return ok(true);
  if(f.op==='scope_args')return ok({kind:'null'});if(f.op==='scope_context')return ok({identity:{kind:'null'},now_ms:'0'});
  if(f.op==='finish')return ok({key:'k',result:{kind:'null'}});
  return ok(null);}});
 const db=new Database(runtime,{namespace:'00'.repeat(16),monotonic:()=>0,timers:timer,context:()=>j.null(),clock:()=>'0',
  onBackgroundError:sink??(report=>reports.push(report))});
 await db.open({schema:schemas[1],engine:{kind:'memory',store:null}});
 return {db,frames,reports};
}
function unhandled(){const seen=[];const listener=reason=>seen.push(reason);process.on('unhandledRejection',listener);return {seen,stop:()=>process.off('unhandledRejection',listener)};}

test('OD03 a throwing timer set cannot mask a committed registration or write and reports one timer fact each',async()=>{
 const {db,reports}=await fixture({timer:timers({failSet:true})});
 try{const token=await db.query('sensors').observe(()=>{},'count');
  assert.equal(typeof token,'string','committed registration returns its handle');
  const row=await db.insert('sensors',{label:j.string('n')});
  assert.deepEqual(row,committed,'exact committed row returned');
  assert.deepEqual(reports.filter(r=>r.source==='timer'),[timerFact,timerFact],'one redacted timer fact per failed set');
  assert.ok(!JSON.stringify(reports).includes('secret'));
 }finally{await db.close().catch(()=>{});}
});

test('OD03 a throwing timer set after a committed handler keeps the committed result and reports a timer fact',async()=>{
 const timer=timers();const {db,reports}=await fixture({timer});
 try{await db.query('sensors').observe(()=>{},'count');timer.set=()=>{throw new Error('late set');};
  const result=await db.execute({name:'noop',arguments:{},reference_paths:[]},j.null(),()=>j.null());
  assert.deepEqual(result,{key:'k',result:{kind:'null'}});await settle();
  assert.deepEqual(reports.filter(r=>r.source==='timer'),[timerFact]);
 }finally{await db.close().catch(()=>{});}
});

test('OD03 a throwing timer clear cannot interrupt delivery of the rest of a page',async()=>{
 const timer=timers();
 const {db,reports}=await fixture({timer,pages:[[count('11',0),rows('12',0)],[],[terminal('11'),rows('12',1)]]});const b=[];
 try{await db.query('sensors').observe(()=>{},'count');await db.query('readings').observe(event=>b.push(event));
  await db.drain();await db.insert('sensors',{label:j.string('n')});
  timer.clear=()=>{throw new Error('host timer clear secret');};
  assert.equal(await db.drain(),2,'the explicit drain returns its delivered count');
  assert.deepEqual(b,[rows('12',0),rows('12',1)],'healthy subscription receives every later event of its page');
  assert.deepEqual(reports,[{source:'observation',kind:'LimitExceeded',token:'11'},timerFact]);
 }finally{await db.close().catch(()=>{});}
});

test('OD03 a throwing timer clear does not block unsubscribe or close',async()=>{
 const timer=timers();const {db,frames,reports}=await fixture({timer});
 const token=await db.query('sensors').observe(()=>{},'count');await db.insert('sensors',{label:j.string('n')});
 timer.clear=()=>{throw new Error('clear');};
 assert.equal(await db.unsubscribe(token),true);
 assert.ok(frames.some(f=>f.op==='unsubscribe'),'core unsubscribe still issued');
 assert.deepEqual(reports.filter(r=>r.source==='timer'),[timerFact]);
 await db.close();assert.equal(db.owner,null,'close completes despite a failing clear');
});

test('OD07 an asynchronous sink rejection is contained without an unhandled rejection',async()=>{
 const probe=unhandled();let calls=0;
 const {db}=await fixture({pages:[[terminal('11')]],sink:async()=>{calls++;throw new Error('async sink');}});
 try{await db.query('sensors').observe(()=>{});await db.drain();await settle();await settle();
  assert.equal(calls,1,'reported once, no recursive report of the sink failure');
  assert.deepEqual(probe.seen,[]);
 }finally{probe.stop();await db.close();}
});

test('OD07 a malformed terminal kind is reported as BackendFailure and callback facts keep CallbackFailure',async()=>{
 const {db,reports}=await fixture({pages:[[terminal('11','NotAKind')],[rows('12',0)]]});
 try{await db.query('sensors').observe(()=>{});await db.query('readings').observe(()=>{throw new Error('x');});
  await db.drain();await db.drain();await settle();
  assert.deepEqual(reports,[{source:'observation',kind:'BackendFailure',token:'11'},{source:'callback',kind:'CallbackFailure',token:'12'}]);
 }finally{await db.close();}
});

test('OD07 an earlier pending Promise rejecting after terminal retirement is suppressed',async()=>{
 const {db,reports}=await fixture({pages:[[rows('11',0)],[terminal('11')]]});const rejectors=[];
 try{await db.query('sensors').observe(()=>new Promise((_,reject)=>rejectors.push(reject)));
  await db.drain();await db.drain();assert.equal(rejectors.length,2);
  rejectors[0](new Error('earlier'));await settle();
  assert.deepEqual(reports,[{source:'observation',kind:'LimitExceeded',token:'11'}],'earlier invocation suppressed');
  rejectors[1](new Error('terminal invocation'));await settle();
  assert.deepEqual(reports,[{source:'observation',kind:'LimitExceeded',token:'11'},{source:'callback',kind:'CallbackFailure',token:'11'}]);
 }finally{await db.close();}
});

test('OD07 two coalesced managed drain requests sharing one failure yield one drain report',async()=>{
 let release;const hold={promise:new Promise(resolve=>{release=resolve;})};const timer=timers();
 const {db,reports}=await fixture({timer,hold,drainFailure:'BackendFailure'});
 try{await db.query('sensors').observe(()=>{});
  for(const [id,work]of [...timer.pending]){timer.pending.delete(id);work();}
  await settle();
  const first=db.insert('sensors',{label:j.string('a')});const second=db.insert('sensors',{label:j.string('b')});
  await settle();release();
  assert.deepEqual(await first,committed);assert.deepEqual(await second,committed);await settle();
  assert.deepEqual(reports.filter(r=>r.source==='observation'),[{source:'observation',kind:'BackendFailure',token:null}]);
 }finally{await db.close();}
});

test('OD04 a throwing clock while scheduling the next sync wake is contained as one S fact',async()=>{
 const probe=unhandled();let clocks=0;const reports=[];
 const runtime=createRuntime({open:()=>ok('1'),call:input=>{const f=JSON.parse(input);
  if(f.op==='prepare_push')return ok({envelopes:[],next_due_ms:'1000',has_more:false});
  if(f.op==='receive_wire')return ok([]);
  if(f.op==='drain')return ok({events:[],has_more:false});
  return ok(null);}});
 const db=new Database(runtime,{namespace:'00'.repeat(16),monotonic:()=>0,jitter:()=>0,
  clock:()=>{if(++clocks>1)throw new Error('clock secret');return '0';},onBackgroundError:report=>reports.push(report)});
 await db.open({schema:schemas[1],engine:{kind:'memory',store:null}});
 try{db.startSync(async()=>'[]');
  for(let i=0;i<40&&clocks<2;i++)await new Promise(resolve=>setTimeout(resolve,5));
  await settle();await settle();
  assert.equal(clocks,2,'the finally clock sample was taken');
  assert.deepEqual(reports,[{source:'sync',kind:'BackendFailure',token:null}]);
  assert.deepEqual(probe.seen,[],'no unhandled rejection from the timer-driven sync');
 }finally{db.stopSync();probe.stop();await db.close();}
});
