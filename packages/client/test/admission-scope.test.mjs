// SPEC-0037 R3.5 / TC-0041 SI-J01..06: host queues never grant core admission.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime,j} from '../src/index.js';
import {schemas} from './schemas.mjs';

const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const busy=()=>JSON.stringify({abi:'1',ok:false,error:{kind:'Busy'}});
const definition={name:'admission-v1',arguments:{},reference_paths:[]};
const deferred=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};};
const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function until(check){for(let i=0;i<50;i++){if(check())return;await tick();}assert.fail('expected scheduled operation did not progress');}
async function turns(){for(let i=0;i<8;i++)await tick();}
function work(db,handler,options){const promise=db.execute(definition,j.object({}),handler,options);void promise.catch(()=>{});return promise;}

async function fixture(global=false){
 const f={calls:[],active:new Map(),queues:new Map(),listeners:new Set(),sequence:0};
 const scope=owner=>global?'shared':owner;
 const queued=key=>{if(!f.queues.has(key))f.queues.set(key,[]);return f.queues.get(key);};
 const release=owner=>{
  f.active.delete(scope(owner));
  queueMicrotask(()=>{for(const listener of [...f.listeners])listener();});
 };
 async function database(owner){
  const reports=[];
  const channel={open:()=>ok(owner),subscribeRelease(listener){f.listeners.add(listener);return()=>f.listeners.delete(listener);},call:input=>{
   const frame=JSON.parse(input),key=scope(frame.owner),queue=queued(key);f.calls.push(frame);
   switch(frame.op){
    case 'request':{const request=String(++f.sequence);queue.push({request,owner:frame.owner});return ok(request);}
    case 'begin':{
     const admitted=!f.active.has(key)&&queue[0]?.request===frame.args.request;
     if(admitted){queue.shift();f.active.set(key,{request:frame.args.request,owner:frame.owner});}
     return ok(admitted);
    }
    case 'scope_args':return ok(j.object({}));
    case 'scope_context':return ok({identity:j.null(),now_ms:'10'});
    case 'finish':assert.equal(f.active.get(key)?.request,frame.args.request);release(frame.owner);return ok({key:'k'+frame.args.request,result:frame.args.result});
    case 'abort':release(frame.owner);return ok(null);
    case 'cancel_request':f.queues.set(key,queue.filter(entry=>entry.request!==frame.args.request));return ok(null);
    case 'prepare_push':return f.active.has(key)||queue.length?busy():ok({envelopes:[],next_due_ms:null,has_more:false});
    case 'reset':return f.active.has(key)||queue.length?busy():ok(null);
    case 'drain':return ok({events:[],has_more:false});
    case 'close':{
     if(f.active.get(key)?.owner===frame.owner)release(frame.owner);
     f.queues.set(key,queue.filter(entry=>entry.owner!==frame.owner));return ok(null);
    }
    default:return ok(null);
   }
  }};
  const db=new Database(createRuntime(channel),{namespace:'00'.repeat(16),context:()=>j.null(),clock:()=>10,jitter:()=>0,onBackgroundError:r=>reports.push(r)});
  await db.open({schema:schemas[1],engine:{kind:'memory',store:null}});return {db,reports};
 }
 f.a=await database('1');f.b=await database('2');
 f.cleanup=()=>Promise.allSettled([f.a.db.close(),f.b.db.close()]);return f;
}

test('SI-J01 a suspended handler cannot stall an independent database handler',async()=>{
 const f=await fixture(),gate=deferred();let started=false,finished=false;
 const a=work(f.a.db,async()=>{started=true;await gate.promise;return j.null();});let b;
 try{
  await until(()=>started);
  b=work(f.b.db,async()=>j.null()).then(value=>{finished=true;return value;});
  await turns();assert.equal(finished,true,'B must commit while A is still suspended');
  assert.equal(f.active.get('1')?.owner,'1');
 }finally{gate.resolve();await Promise.allSettled([a,b]);await f.cleanup();}
});

test('SI-J02 reset and sync on B are independent of A host jobs',async()=>{
 const f=await fixture(),gate=deferred();let started=false;
 const a=work(f.a.db,async()=>{started=true;await gate.promise;return j.null();});
 try{
  await until(()=>started);
  await f.b.db.reset();
  f.b.db.transport=async()=>'[]';await f.b.db.sync();
  assert.ok(f.calls.some(frame=>frame.owner==='2'&&frame.op==='reset'));
  assert.ok(f.calls.some(frame=>frame.owner==='2'&&frame.op==='prepare_push'));
  assert.equal(f.active.get('1')?.owner,'1');assert.deepEqual(f.b.reports,[]);
 }finally{gate.resolve();await a;await f.cleanup();}
});

test('SI-J03 same-instance FIFO survives abort and suppresses a late handler result',async()=>{
 const f=await fixture(),gate=deferred(),cancel=new AbortController();let firstStarted=false,secondStarted=false;
 const a=work(f.a.db,async()=>{firstStarted=true;await gate.promise;return j.null();},{signal:cancel.signal});
 let second;
 try{
  await until(()=>firstStarted);
  second=work(f.a.db,async()=>{secondStarted=true;return j.null();});
  await turns();assert.equal(secondStarted,false);
  cancel.abort();await assert.rejects(a,error=>error.kind==='Cancelled');await second;
  gate.resolve();await turns();
  assert.equal(f.calls.filter(frame=>frame.owner==='1'&&frame.op==='finish').length,1);
  assert.equal(f.calls.filter(frame=>frame.owner==='1'&&frame.op==='abort').length,1);
 }finally{cancel.abort();gate.resolve();await Promise.allSettled([a,second]);await f.cleanup();}
});

test('SI-J04 a conservative shared core still gates handlers on real begin admission',async()=>{
 const f=await fixture(true),gate=deferred();let aStarted=false,bStarted=false;
 const a=work(f.a.db,async()=>{aStarted=true;await gate.promise;return j.null();});let b;
 try{
  await until(()=>aStarted);b=work(f.b.db,async()=>{bStarted=true;return j.null();});
  await turns();assert.equal(bStarted,false);
  gate.resolve();await Promise.all([a,b]);assert.equal(bStarted,true);
  assert.equal(f.active.size,0);assert.equal(f.listeners.size,0);
 }finally{gate.resolve();await Promise.allSettled([a,b]);await f.cleanup();}
});

test('SI-J05 close settles only its own jobs and never publishes a stale handler result',async()=>{
 const f=await fixture(),gateA=deferred(),gateB=deferred();let aStarted=false;
 const a=work(f.a.db,async()=>{aStarted=true;await gateA.promise;return j.null();});let b;
 try{
  await until(()=>aStarted);b=work(f.b.db,async()=>{await gateB.promise;return j.null();});
  // B may still be queued on the old host implementation; close must settle it either way.
  await turns();await f.b.db.close();await assert.rejects(b,error=>error.kind==='Cancelled');
  assert.equal(f.active.get('1')?.owner,'1');
  gateA.resolve();await a;gateB.resolve();await turns();
  assert.equal(f.calls.filter(frame=>frame.owner==='2'&&frame.op==='finish').length,0);
  assert.equal(f.calls.filter(frame=>frame.owner==='1'&&frame.op==='finish').length,1);
 }finally{gateA.resolve();gateB.resolve();await Promise.allSettled([a,b]);await f.cleanup();}
});

test('SI-J06 shared-core sync admission Busy waits for release without an S fact',async()=>{
 const f=await fixture(true),gate=deferred();let started=false;
 const a=work(f.a.db,async()=>{started=true;await gate.promise;return j.null();});let sync;
 try{
  await until(()=>started);f.b.db.transport=async()=>'[]';sync=f.b.db.sync();
  await turns();assert.deepEqual(f.b.reports,[]);
  const attempts=f.calls.filter(frame=>frame.owner==='2'&&frame.op==='prepare_push').length;
  assert.equal(attempts,1,'B reached the conservative core and received admission Busy');
  await turns();assert.equal(f.calls.filter(frame=>frame.owner==='2'&&frame.op==='prepare_push').length,attempts,'no retry polling while admission is held');
  gate.resolve();await a;await sync;
  await until(()=>f.calls.filter(frame=>frame.owner==='2'&&frame.op==='prepare_push').length>attempts);
  await turns();assert.deepEqual(f.b.reports,[]);assert.equal(f.listeners.size,0);
 }finally{gate.resolve();await Promise.allSettled([a,sync]);await f.cleanup();}
});
