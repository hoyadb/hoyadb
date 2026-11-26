// TC-0050 / DEF-0044..0046: real WASM owner and admission regressions.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import init,{Facade} from '../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {createRuntime,openDatabase as openHost} from '../src/index.js';
import {openApplicationDatabase,defineCommand} from '../src/application.js';
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});
const schema={version:'1',tables:{notes:{columns:{body:{kind:'String',optional:false}},relations:{}}}};
const deferred=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};};
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const until=async condition=>{for(let n=0;n<100;n++){if(await condition())return;await pause(2);}assert.fail('expected host operation did not settle');};
async function settled(promise){let timer;try{return await Promise.race([promise.then(()=> 'completed',error=>error.kind),new Promise(resolve=>{timer=setTimeout(()=>resolve('timeout'),1000);})]);}finally{clearTimeout(timer);}}
const command=(db,name,handler=()=>null)=>db.command(defineCommand(name,{args:{},handler})).executeWithReceipt({});
const outcomes=envelopes=>envelopes.map(e=>({idempotencyKey:e.idempotencyKey,status:'applied',result:null}));
async function fixture(extra={}){
 const facade=new Facade(),listeners=new Set();let now=1000;
 const runtime=createRuntime({open:(s,k)=>facade.open(s,k),call:s=>facade.call(s),subscribeRelease(listener){listeners.add(listener);return()=>listeners.delete(listener);}},{namespace:()=>new Uint8Array(16).fill(4)});
 const options={runtime,schema,engine:'memory',clock:()=>now,jitter:()=>0,...extra};
 const db=await openApplicationDatabase(options);
 return {db,runtime,options,listeners,advance(){now=100000;},async close(){await db.close();facade.free();}};
}
for(const phase of ['transport','mapper'])for(const action of ['stopSync','reset','close'])test(`DEF-0044 explicit ${phase} cancellation on ${action} settles and suppresses late work`,async()=>{
 const f=await fixture({entities:{scope:'app',tables:['notes']}}),entered=deferred(),late=deferred();let pushes=0,mappings=0;
 try{
  const receipt=await command(f.db,'cancel-explicit-v1');
  f.db.transport={push:envelopes=>{pushes++;if(phase==='transport'){entered.resolve();return late.promise;}return outcomes(envelopes);}};
  f.db.mapper=()=>{mappings++;if(phase==='mapper'){entered.resolve();return late.promise;}return {writes:[],mappings:{}};};
  const pending=f.db.sync();void pending.catch(()=>{});await entered.promise;
  await f.db[action](...(action==='reset'?['discard_recovery']:[]));
  assert.equal(await settled(pending),'Closed');assert.equal(f.listeners.size,0);
  if(action!=='close'){
   const state=JSON.stringify(await f.db.queueStatus());
   late.resolve(phase==='transport'?[{idempotencyKey:receipt.key,status:'applied',result:null}]:{writes:[],mappings:{}});await pause(5);
   assert.equal(JSON.stringify(await f.db.queueStatus()),state);
   f.advance();f.db.transport={push:outcomes};f.db.mapper=()=>({writes:[],mappings:{}});
   assert.equal(await settled(f.db.sync()),'completed');
   // A stopped explicit run must not keep the subsequent background owner busy.
   await command(f.db,'fresh-v1');let fresh=0;
   f.db.startSync({push:envelopes=>{fresh++;return outcomes(envelopes);}},()=>({writes:[],mappings:{}}));await until(()=>fresh>0);f.db.stopSync();
  }else{late.resolve(phase==='transport'?[]:{writes:[],mappings:{}});await pause(5);}
  assert.equal(pushes,1);assert.equal(mappings,phase==='mapper'?1:0);
 }finally{late.resolve(phase==='transport'?[]:{writes:[],mappings:{}});await f.close();}
});
test('DEF-0044 close waits for an explicit admitted host call before disposing the owner',async()=>{
 const f=await fixture(),entered=deferred(),reply=deferred();let closes=0;
 try{
  const prepare=f.db.raw.preparePush.bind(f.db.raw),call=f.db.raw.call.bind(f.db.raw);
  f.db.raw.preparePush=async()=>{entered.resolve();await reply.promise;return prepare();};
  f.db.raw.call=(op,...args)=>{if(op==='close')closes++;return call(op,...args);};f.db.transport={push:outcomes};
  const pending=f.db.sync();void pending.catch(()=>{});await entered.promise;const closing=f.db.close();await pause(5);
  assert.equal(closes,0);reply.resolve();await closing;assert.equal(await settled(pending),'Closed');assert.equal(closes,1);
 }finally{reply.resolve();await f.close();}
});
for(const phase of ['receipt','reconcile'])test(`DEF-0045 ${phase} admission retains response and mapper plan without polling`,async()=>{
 const f=await fixture(),held=deferred(),entered=deferred();let other,blocking;
 try{
  other=await openApplicationDatabase(f.options);const receipt=await command(f.db,'retained-v1');let pushes=0,mappings=0,attempts=0;
  const hold=()=>{blocking=command(other,'hold-v1',async()=>{entered.resolve();await held.promise;return null;});return entered.promise;};
  const operation=phase==='receipt'?'receiveWire':'reconcile',original=f.db.raw[operation].bind(f.db.raw),seen=[];
  f.db.raw[operation]=(...args)=>{attempts++;seen.push(JSON.stringify(args));return original(...args);};
  const plan={writes:[],mappings:{}};
  f.db.transport={push:async envelopes=>{pushes++;if(phase==='receipt')await hold();return outcomes(envelopes);}};
  f.db.mapper=async()=>{mappings++;if(phase==='reconcile')await hold();return plan;};
  const pending=f.db.sync();void pending.catch(()=>{});await entered.promise;await until(()=>attempts===1);await pause(20);
  assert.equal(attempts,1,'Busy waits for release rather than polling');
  if(phase==='reconcile')plan.writes.push({op:'invalid-late-mutation'});
  held.resolve();await blocking;assert.equal(await settled(pending),'completed');assert.equal(pushes,1);assert.equal(mappings,1);
  assert.equal(attempts,2);assert.equal(seen[0],seen[1]);assert.equal(f.listeners.size,0);
  assert.equal((await f.db.pendingReconciliations()).some(p=>p.key===receipt.key),false);
 }finally{held.resolve();await blocking;await other?.close();await f.close();}
});
test('DEF-0045 stop cancels a retained Busy proposal and unregisters its release listener',async()=>{
 const f=await fixture(),held=deferred(),entered=deferred();let other,blocking;
 try{
  other=await openApplicationDatabase(f.options);await command(f.db,'cancel-plan-v1');let attempts=0;
  const reconcile=f.db.raw.reconcile.bind(f.db.raw);f.db.raw.reconcile=(...args)=>{attempts++;return reconcile(...args);};
  f.db.transport={push:outcomes};f.db.mapper=async()=>{blocking=command(other,'held-v1',async()=>{entered.resolve();await held.promise;return null;});await entered.promise;return {writes:[],mappings:{}};};
  const pending=f.db.sync();void pending.catch(()=>{});await until(()=>attempts===1);f.db.stopSync();assert.equal(await settled(pending),'Closed');assert.equal(f.listeners.size,0);
  held.resolve();await blocking;await pause(5);assert.equal(attempts,1);assert.equal((await f.db.pendingReconciliations()).length,1);
 }finally{held.resolve();await blocking;await other?.close();await f.close();}
});
test('DEF-0045 release during a suspended Busy reply resumes the captured plan once',async()=>{
 const f=await fixture(),held=deferred(),entered=deferred(),busy=deferred(),reply=deferred();let other,blocking;
 try{
  other=await openApplicationDatabase(f.options);await command(f.db,'release-race-v1');let attempts=0,mappings=0;
  const reconcile=f.db.raw.reconcile.bind(f.db.raw);f.db.raw.reconcile=async(...args)=>{attempts++;try{return await reconcile(...args);}catch(error){if(error.kind==='Busy'){busy.resolve();await reply.promise;}throw error;}};
  f.db.transport={push:outcomes};f.db.mapper=async()=>{mappings++;blocking=command(other,'held-race-v1',async()=>{entered.resolve();await held.promise;return null;});await entered.promise;return {writes:[],mappings:{}};};
  const pending=f.db.sync();void pending.catch(()=>{});await busy.promise;held.resolve();await blocking;reply.resolve();assert.equal(await settled(pending),'completed');assert.equal(attempts,2);assert.equal(mappings,1);
 }finally{held.resolve();reply.resolve();await blocking;await other?.close();await f.close();}
});
test('DEF-0046 invalid factory sync configuration does not lease a retained store',async()=>{
 const f=await fixture();try{
  const store=await f.runtime.createMemoryStore();
  await assert.rejects(openHost({runtime:f.runtime,schema,engine:{kind:'memory',store},transport:{}}),{kind:'InvalidRequest'});
  const opened=await openHost({runtime:f.runtime,schema,engine:{kind:'memory',store}});await opened.close();await f.runtime.dropMemoryStore(store);
 }finally{await f.close();}
});
test('DEF-0045 stop retires the release listener while an admitted native reply is pending',async()=>{
 const f=await fixture(),entered=deferred(),reply=deferred();try{
  const prepare=f.db.raw.preparePush.bind(f.db.raw);f.db.raw.preparePush=async()=>{entered.resolve();await reply.promise;return prepare();};f.db.transport={push:outcomes};
  const pending=f.db.sync();void pending.catch(()=>{});await entered.promise;assert.equal(f.listeners.size,1);f.db.stopSync();await pause(5);assert.equal(f.listeners.size,0);
  reply.resolve();assert.equal(await settled(pending),'Closed');
 }finally{reply.resolve();await f.close();}
});
test('DEF-0045 same-owner background contention maps each receipt only once',async()=>{
 const f=await fixture(),held=deferred(),entered=deferred();let blocking;
 try{
  const receipt=await command(f.db,'background-retained-v1'),mapped=new Map(),sent=new Map();let attempts=0;
  const reconcile=f.db.raw.reconcile.bind(f.db.raw);f.db.raw.reconcile=(key,...args)=>{if(key===receipt.key)attempts++;return reconcile(key,...args);};
  f.db.startSync({push:envelopes=>{for(const e of envelopes)sent.set(e.idempotencyKey,(sent.get(e.idempotencyKey)??0)+1);return outcomes(envelopes);}},async pending=>{
   mapped.set(pending.key,(mapped.get(pending.key)??0)+1);
   if(pending.key===receipt.key&&mapped.get(pending.key)===1){blocking=command(f.db,'background-held-v1',async()=>{entered.resolve();await held.promise;return null;});await entered.promise;}
   return {writes:[],mappings:{}};
  });
  await entered.promise;await until(()=>attempts===1);await pause(20);assert.equal(attempts,1);
  held.resolve();await blocking;await until(async()=>!(await f.db.raw.pendingReconciliations()).some(p=>p.key===receipt.key));f.db.stopSync();
  assert.equal(mapped.get(receipt.key),1);assert.equal(sent.get(receipt.key),1);assert.equal(attempts,2);
 }finally{held.resolve();await blocking;await f.close();}
});
test('DEF-0044 reset cancels an explicit sync after its push has reached pull',async()=>{
 const f=await fixture({entities:{scope:'app',tables:['notes']}}),entered=deferred(),late=deferred();try{
  f.db.transport={push:outcomes,pull:()=>{entered.resolve();return late.promise;}};
  const pending=f.db.sync();void pending.catch(()=>{});await entered.promise;await f.db.reset('discard_recovery');assert.equal(await settled(pending),'Closed');
  late.resolve({checkpoint:'stale',complete:true,changes:[]});await pause(5);assert.equal((await f.db.syncProgress()).ready,false);
  f.db.transport={push:outcomes,pull:()=>({checkpoint:'fresh',complete:true,changes:[]})};assert.equal(await settled(f.db.sync()),'completed');assert.equal((await f.db.syncProgress()).ready,true);
 }finally{late.resolve({checkpoint:'stale',complete:true,changes:[]});await f.close();}
});
test('DEF-0045 mapper argument mutation cannot replace the persisted entity mapping context',async()=>{
 const f=await fixture({entities:{scope:'app',tables:['notes']}});try{
  const row=await f.db.table('notes').insert({body:'entity'});
  f.db.transport={push:envelopes=>envelopes.map(e=>({table:'notes',recordId:row.id,writeId:e.writeId,status:'applied',result:null}))};
  f.db.mapper=pending=>{pending.created={};pending.kind='command';return {writes:[],mappings:{}};};
  assert.equal(await settled(f.db.sync()),'completed');assert.equal((await f.db.pendingReconciliations()).length,0);
  assert.equal((await f.db.table('notes').findById(row.id).fetch()).body,'entity');
 }finally{await f.close();}
});
async function pendingPair(db){
 const first=await command(db,'context-first-v1'),second=await command(db,'context-second-v1');
 const batch=await db.preparePush(1000,0);await db.receive(outcomes(batch.envelopes));return [first.key,second.key];
}
function settleBeforeContext(db,key){
 const call=db.raw.call.bind(db.raw);let settled=false;
 db.raw.call=async(op,args,...rest)=>{if(op==='reconciliation_context'&&args.key===key&&!settled){settled=true;await db.raw.reconcile(key,{writes:[],mappings:{}});}return call(op,args,...rest);};
}
test('DEF-0054 pending listing omits a concurrently settled key and preserves other pending context',async()=>{
 const f=await fixture();try{
  const [first,second]=await pendingPair(f.db);settleBeforeContext(f.db,first);
  const pending=await f.db.pendingReconciliations();assert.deepEqual(pending.map(p=>p.key),[second]);assert.equal(pending[0].kind,'command');assert.deepEqual(pending[0].created,{});
 }finally{await f.close();}
});
test('DEF-0054 background sync skips concurrently settled context and maps the remaining receipt',async()=>{
 const facts=[],mapped=[],f=await fixture({onBackgroundError:fact=>facts.push(fact)});try{
  const [first,second]=await pendingPair(f.db);settleBeforeContext(f.db,first);
  f.db.startSync({push:outcomes},pending=>{mapped.push(pending.key);return {writes:[],mappings:{}};});
  await until(()=>mapped.length>0||facts.length>0);await until(async()=>facts.length>0||(await f.db.raw.pendingReconciliations()).length===0);f.db.stopSync();
  assert.deepEqual(facts,[]);assert.deepEqual(mapped,[second]);assert.equal((await f.db.pendingReconciliations()).length,0);
 }finally{await f.close();}
});
for(const kind of ['Protocol','BackendFailure','Closed'])test(`DEF-0054 pending listing preserves ${kind} when context cannot be confirmed settled`,async()=>{
 const f=await fixture();try{
  const [first]=await pendingPair(f.db),call=f.db.raw.call.bind(f.db.raw);let injected=false;
  f.db.raw.call=async(op,args,...rest)=>{if(op==='reconciliation_context'&&args.key===first&&!injected){injected=true;if(kind==='Closed')await f.db.raw.close();else throw Object.assign(Error('context failure'),{kind});}return call(op,args,...rest);};
  await assert.rejects(f.db.pendingReconciliations(),{kind});
 }finally{await f.close();}
});
test('DEF-0054 a reset between pending list and context is lifecycle invalidation',async()=>{
 const f=await fixture({entities:{scope:'app',tables:['notes']}});try{
  const [first]=await pendingPair(f.db),call=f.db.raw.call.bind(f.db.raw);let reset=false;
  f.db.raw.call=async(op,args,...rest)=>{if(op==='reconciliation_context'&&args.key===first&&!reset){reset=true;await f.db.raw.reset('discard_recovery');}return call(op,args,...rest);};
  await assert.rejects(f.db.pendingReconciliations(),{kind:'Closed'});
 }finally{await f.close();}
});
