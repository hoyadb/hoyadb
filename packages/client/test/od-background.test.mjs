// TC-0040 OD03/OD04/OD07/OD08 (SPEC-0036 items 2-4): JS terminal delivery,
// non-awaited ordered callbacks and the redacted exactly-once background sink.
import test from 'node:test';import assert from 'node:assert/strict';import {Database,createRuntime,j} from '../src/index.js';import {schemas} from './schemas.mjs';
const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const bad=kind=>JSON.stringify({abi:'1',ok:false,error:{kind}});
const rows=(subscription,n)=>({kind:'data',subscription,value:{kind:'rows',rows:Array.from({length:n},(_,i)=>({id:`r${i}`,fields:{label:j.string('x')}}))}});
const terminal=subscription=>({kind:'error',subscription,value:{kind:'LimitExceeded'}});
const settle=()=>new Promise(resolve=>setImmediate(resolve));
const bounded=promise=>Promise.race([promise,new Promise(resolve=>setTimeout(()=>resolve('blocked'),200))]);
function timers(){let next=0;const pending=new Map();return {now:()=>0,set(work){const id=++next;pending.set(id,work);return id;},clear(id){pending.delete(id);},pending,async run(){for(const [id,work]of [...pending]){pending.delete(id);work();}await settle();await settle();}};}
async function fixture({pages=[],unsubscribe=()=>ok(true),drainFailure=null}={}){
 const timer=timers(),frames=[],reports=[];let handle=10;
 const runtime=createRuntime({open:()=>ok('1'),call:input=>{const f=JSON.parse(input);frames.push(f);
  if(f.op==='observe'||f.op==='observe_status')return ok(String(++handle));
  if(f.op==='unsubscribe'||f.op==='unsubscribe_sync')return unsubscribe(f);
  if(f.op==='drain'){if(drainFailure)return bad(drainFailure);const events=pages.shift()??[];return ok({events,has_more:false});}
  if(f.op==='insert')return ok({id:'n',fields:{label:j.string('n')}});
  if(f.op==='close')return ok(null);
  if(f.op==='prepare_push')return ok({envelopes:['{}'],next_due_ms:null,has_more:false});
  return ok(null);}});
 const db=new Database(runtime,{namespace:'00'.repeat(16),monotonic:timer.now,timers:timer,jitter:()=>0,onBackgroundError:report=>reports.push(report)});
 await db.open({schema:schemas[1],engine:{kind:'memory',store:null}});
 return {db,timer,frames,reports};
}

test('OD04 a generic terminal event reaches only its listener once, retires it and reports T',async()=>{
 const pages=[[rows('11',1),terminal('11'),rows('12',0)],[rows('11',2),rows('12',1)]];
 const {db,frames,reports}=await fixture({pages});const a=[],b=[];
 try{await db.query('sensors').observe(event=>a.push(event));await db.query('readings').observe(event=>b.push(event));
  await db.drain();await db.drain();
  assert.deepEqual(a,[rows('11',1),terminal('11')],'terminal delivered once, later events suppressed');
  assert.deepEqual(b,[rows('12',0),rows('12',1)],'healthy listener continues');
  assert.deepEqual(reports,[{source:'observation',kind:'LimitExceeded',token:'11'}]);
  assert.equal(frames.filter(f=>f.op==='unsubscribe').length,0,'no extra core call for a terminated handle');
 }finally{await db.close();}
});

test('OD08 a recoverable data error keeps its listener and is not reported',async()=>{
 const error={kind:'data',subscription:'11',value:{kind:'error',error:{kind:'BackendFailure'}}};
 const {db,reports}=await fixture({pages:[[error],[rows('11',1)]]});const a=[];
 try{await db.query('sensors').observe(event=>a.push(event));await db.drain();await db.drain();
  assert.deepEqual(a,[error,rows('11',1)]);assert.deepEqual(reports,[]);
 }finally{await db.close();}
});

test('OD04 a throwing callback retires only its listener, reports C once and unsubscribes in core',async()=>{
 const {db,frames,reports}=await fixture({pages:[[rows('11',0),rows('12',0)],[rows('11',1),rows('12',1)]]});const b=[];let calls=0;
 try{await db.query('sensors').observe(()=>{calls++;throw new Error('application secret');});await db.query('readings').observe(event=>b.push(event));
  await db.drain();await db.drain();await settle();
  assert.equal(calls,1,'retired after its first failure');assert.equal(b.length,2,'other listener unaffected');
  assert.deepEqual(reports,[{source:'callback',kind:'CallbackFailure',token:'11'}]);
  assert.deepEqual(frames.filter(f=>f.op==='unsubscribe').map(f=>f.args.subscription),['11']);
  assert.ok(!JSON.stringify(reports).includes('secret'),'no message forwarded');
 }finally{await db.close();}
});

test('OD07 callbacks run in event order and a pending Promise never delays later callbacks',async()=>{
 const {db,reports}=await fixture({pages:[[rows('11',0),rows('12',0),rows('11',1)]]});const order=[];let release;
 try{await db.query('sensors').observe(event=>{order.push(`a${event.value.rows.length}`);if(!release)return new Promise(resolve=>{release=resolve;});});
  await db.query('readings').observe(()=>{order.push('b');});
  const delivered=await bounded(db.drain());
  assert.equal(delivered,3,'drain completes while the first callback Promise is pending');
  assert.deepEqual(order,['a0','b','a1']);release();await settle();assert.deepEqual(reports,[]);
 }finally{release?.();await db.close();}
});

test('OD07 a rejected returned Promise retires its listener once; a second rejection is not reported',async()=>{
 const {db,reports}=await fixture({pages:[[rows('11',0),rows('11',1)],[rows('11',2)]]});const rejectors=[];let calls=0;
 try{await db.query('sensors').observe(()=>{calls++;return new Promise((_,reject)=>rejectors.push(reject));});
  assert.equal(await bounded(db.drain()),2);assert.equal(calls,2);
  rejectors[0](new Error('one'));await settle();rejectors[1](new Error('two'));await settle();
  await bounded(db.drain());assert.equal(calls,2,'retired listener receives nothing more');
  assert.deepEqual(reports,[{source:'callback',kind:'CallbackFailure',token:'11'}]);
 }finally{await db.close();}
});

test('OD07 a terminal callback that throws yields T then C and no core unsubscribe',async()=>{
 const {db,frames,reports}=await fixture({pages:[[terminal('11')]]});
 try{await db.query('sensors').observe(()=>{throw new Error('x');});await db.drain();await settle();
  assert.deepEqual(reports,[{source:'observation',kind:'LimitExceeded',token:'11'},{source:'callback',kind:'CallbackFailure',token:'11'}]);
  assert.equal(frames.filter(f=>f.op==='unsubscribe').length,0);
 }finally{await db.close();}
});

test('OD07 a failed core unsubscribe after a callback retirement is reported once as U',async()=>{
 const {db,reports}=await fixture({pages:[[rows('11',0)]],unsubscribe:()=>bad('Busy')});
 try{await db.query('sensors').observe(()=>{throw new Error('x');});await db.drain();await settle();await settle();
  assert.deepEqual(reports,[{source:'callback',kind:'CallbackFailure',token:'11'},{source:'observation',kind:'Busy',token:'11'}]);
 }finally{await db.close();}
});

test('OD07 an explicit drain returns its result while callback failures are C reports',async()=>{
 const {db,reports}=await fixture({pages:[[rows('11',0)]]});
 try{await db.query('sensors').observe(()=>{throw new Error('x');});
  assert.equal(await db.drain(),1);await settle();
  assert.deepEqual(reports.map(r=>r.source),['callback']);
 }finally{await db.close();}
});

test('OD04 a failed managed drain reaches the sink once as D and is not broadcast to listeners',async()=>{
 const {db,timer,reports}=await fixture({drainFailure:'BackendFailure'});const a=[];
 try{await db.query('sensors').observe(event=>a.push(event));await timer.run();
  assert.deepEqual(reports,[{source:'observation',kind:'BackendFailure',token:null}]);assert.deepEqual(a,[]);
 }finally{await db.close();}
});

test('OD07 late rejections after close are suppressed',async()=>{
 const {db,reports}=await fixture({pages:[[rows('11',0)]]});let reject;
 await db.query('sensors').observe(()=>new Promise((_,no)=>{reject=no;}));assert.equal(await bounded(db.drain()),1);
 await db.close();reject(new Error('late'));await settle();
 assert.deepEqual(reports,[]);
});

test('OD04 automatic sync failures reach the sink as S',async()=>{
 const {db,reports}=await fixture();
 try{db.startSync(async()=>{throw Object.assign(new Error('offline'),{kind:'BackendFailure'});});
  for(let i=0;i<20&&!reports.length;i++)await new Promise(resolve=>setTimeout(resolve,5));
  db.stopSync();
  assert.deepEqual(reports.filter(r=>r.source==='sync').length,1);
  assert.deepEqual(Object.keys(reports[0]).sort(),['kind','source','token']);assert.equal(reports[0].token,null);
 }finally{db.stopSync();await db.close();}
});

test('OD07 sink failures are contained and legacy diagnostics stay bounded to 64',async()=>{
 let thrown=0;
 const timer=timers();let handle=10;
 const runtime=createRuntime({open:()=>ok('1'),call:input=>{const f=JSON.parse(input);if(f.op==='observe')return ok(String(++handle));if(f.op==='drain')return ok({events:[rows(String(handle),0)],has_more:false});return ok(true);}});
 const db=new Database(runtime,{namespace:'00'.repeat(16),monotonic:timer.now,timers:timer,onBackgroundError:()=>{thrown++;throw new Error('sink');}});
 await db.open({schema:schemas[1],engine:{kind:'memory',store:null}});
 try{for(let i=0;i<70;i++){await db.query('sensors').observe(()=>{throw new Error('x');});await db.drain();}
  await settle();assert.equal(thrown,70);assert.equal(db.callbackErrors.length,64);
 }finally{await db.close();}
});

test('OD03 a real JS write returns its committed row while its automatic drain handles a terminal event',async()=>{
 const pages=[[],[terminal('11')]];
 const {db,reports}=await fixture({pages});const a=[];
 try{await db.query('sensors').observe(event=>a.push(event));await db.drain();
  const row=await db.insert('sensors',{label:j.string('n')});
  assert.deepEqual(row,{id:'n',fields:{label:j.string('n')}},'committed row returned unchanged');
  await settle();
  assert.deepEqual(a,[terminal('11')],'terminal delivered by the automatic post-write drain');
  assert.deepEqual(reports,[{source:'observation',kind:'LimitExceeded',token:'11'}],'only the terminal report');
 }finally{await db.close();}
});
