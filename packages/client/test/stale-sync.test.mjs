// DEF-0011 / TC-0036 G09: controlled transport timing, real Database lifecycle.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime} from '../src/index.js';
import {schemas} from './schemas.mjs';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return{promise,resolve,reject};};
const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function until(condition){for(let n=0;n<100;n++){if(condition())return;await tick();}assert.fail('expected suspended operation did not start');}
function start(db,transport,mapper=null){db.startSync(transport,mapper);clearTimeout(db.timer);db.timer=null;const pending=db.sync();void pending.catch(()=>{});return pending;}
async function fixture(intercept){
 let nextOwner=0;const calls=[];
 const db=new Database(createRuntime({open:()=>reply(String(++nextOwner)),async call(input){const frame=JSON.parse(input);calls.push(frame);const custom=intercept(frame);if(custom!==undefined)return reply(await custom);if(frame.op==='prepare_push')return reply({envelopes:[],has_more:false,next_due_ms:null});if(frame.op==='pending_reconciliations'||frame.op==='receive_wire')return reply([]);if(frame.op==='reconcile_resolving')return reply({kind:'completed'});if(frame.op==='drain')return reply({events:[],has_more:false});return reply(null);}}),{namespace:'00'.repeat(16),clock:()=>100,jitter:()=>0});
 const open=()=>db.open({schema:schemas[1],engine:{kind:'memory',store:null}});await open();return{db,open,calls};
}
const batch=envelopes=>({envelopes,has_more:false,next_due_ms:null});
test('G09 delayed prepare cannot send old bytes to replacement transport or clear its run',async()=>{
 const prepared=deferred(),response=deferred();let oldPreparing=false;
 const f=await fixture(frame=>{if(frame.op==='prepare_push'){if(frame.owner==='1'){oldPreparing=true;return prepared.promise;}return batch(['{"generation":"new"}']);}});
 const old=start(f.db,async()=>assert.fail('old transport must not run after close'));let current;
 try{
  await until(()=>oldPreparing);await f.db.close();await f.open();const sent=[];
  current=start(f.db,body=>{sent.push(body);return body==='[{"generation":"new"}]'?response.promise:'[]';});await until(()=>sent.length===1);const run=f.db.syncRun;
  prepared.resolve(batch(['{"generation":"old"}']));await old;
  assert.deepEqual(sent,['[{"generation":"new"}]'],'late old prepare reached replacement transport');
  assert.equal(f.db.syncRun,run);assert.equal(f.db.syncing,true);assert.equal(f.db.syncWakePending,false);
  f.db.stopSync();response.resolve('[]');await current;
  assert.equal(f.calls.filter(frame=>frame.op==='receive_wire'&&frame.owner==='2').length,1,'current response still follows its own receive path');
 }finally{prepared.resolve(batch([]));response.resolve('[]');f.db.stopSync();await Promise.allSettled([old,current]);await f.db.close();}
});
test('G09 delayed pending list cannot call replacement mapper or alter its active run',async()=>{
 const pendingList=deferred(),response=deferred();let listing=false;
 const f=await fixture(frame=>{if(frame.op==='prepare_push')return batch(frame.owner==='1'?[]:['{"generation":"new"}']);if(frame.op==='pending_reconciliations'&&frame.owner==='1'){listing=true;return pendingList.promise;}if(frame.op==='pending_reconciliations'&&frame.owner==='2')return [{key:'current'}];});
 const old=start(f.db,async()=>assert.fail('empty batch must not send'),async()=>assert.fail('old mapper must not run after close'));let current;
 try{
  await until(()=>listing);await f.db.close();await f.open();const mapped=[];let sending=false;
  current=start(f.db,()=>{sending=true;return response.promise;},async item=>{mapped.push(item.key);return{writes:[],mappings:{}};});await until(()=>sending);const run=f.db.syncRun;
  pendingList.resolve([{key:'old'}]);await old;
  assert.deepEqual([...mapped],[],'late old list reached replacement mapper');
  assert.equal(f.db.syncRun,run);assert.equal(f.db.syncing,true);assert.equal(f.db.syncWakePending,false);
  f.db.stopSync();response.resolve('[]');await current;
  assert.deepEqual(mapped,['current'],'replacement mapper still processes its own list');
  assert.deepEqual(f.calls.filter(frame=>frame.op==='reconcile_resolving').map(frame=>[frame.owner,frame.args.key]),[['2','current']]);
 }finally{pendingList.resolve([]);response.resolve('[]');f.db.stopSync();await Promise.allSettled([old,current]);await f.db.close();}
});

for(const operation of ['prepare_push','pending_reconciliations'])test('G09 late '+operation+' rejection cannot publish errors into replacement owner',async()=>{
 const suspended=deferred(),response=deferred();let waiting=false;
 const f=await fixture(frame=>{if(frame.owner==='1'&&frame.op===operation){waiting=true;return suspended.promise;}if(frame.op==='prepare_push')return batch(frame.owner==='1'?[]:['{"generation":"new"}']);});
 const old=start(f.db,async()=>assert.fail('old empty push must not send'),async()=>assert.fail('old mapper must not run'));let current;
 try{
  await until(()=>waiting);await f.db.close();await f.open();let sending=false;
  current=start(f.db,()=>{sending=true;return response.promise;});await until(()=>sending);const run=f.db.syncRun;
  suspended.reject({kind:'BackendFailure'});await old;
  assert.deepEqual(f.db.syncErrors.map(error=>error.kind),[],'stale rejection reached current sync error collection');
  assert.equal(f.db.syncRun,run);assert.equal(f.db.syncing,true);assert.equal(f.db.syncWakePending,false);
  f.db.stopSync();response.reject({kind:'Protocol'});await current;
  assert.deepEqual(f.db.syncErrors.map(error=>error.kind),['Protocol'],'current-owner transport errors stay observable');
 }finally{suspended.resolve(operation==='prepare_push'?batch([]):[]);response.resolve('[]');f.db.stopSync();await Promise.allSettled([old,current]);await f.db.close();}
});

test('G09/G19 late external receipt cannot wake or drain a replacement owner',async()=>{
 const receipt=deferred();let receiving=false;
 const f=await fixture(frame=>{if(frame.op==='receive_wire'&&frame.owner==='1'){receiving=true;return receipt.promise;}});
 const pending=f.db.receiveWire('[]');
 try{
  await until(()=>receiving);await f.db.close();await f.open();let wakes=0,drains=0;
  f.db.wakeSync=()=>{wakes++;};f.db.managedDrain=async()=>{drains++;};f.db.callbacks.set('current',()=>{});
  receipt.resolve([]);await pending;
  assert.equal(wakes,0,'old receipt must not wake replacement sync driver');
  assert.equal(drains,0,'old receipt must not drain replacement subscriptions');
 }finally{receipt.resolve([]);await pending;await f.db.close();}
});
