// Phase 6 host guarantees ported to ABI1/binary keys. Legacy fixtures remain unchanged.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime,j} from '../src/index.js';
import {schemas} from './schemas.mjs';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
const deferred=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};};
const tick=()=>new Promise(resolve=>setImmediate(resolve));
function fixture({holdClose=false,failOpen=false,failClose=false}={}){
 const entered=deferred(),opened=deferred(),closed=deferred(),frames=[],keys=[],live=new Set();let count=0,closeCalls=0;
 const call=async(input,key)=>{
  const frame=JSON.parse(input);frames.push(frame);if(key)keys.push(new Uint8Array(key));
  if(frame.op==='open'){
   const owner=String(++count);live.add(owner);entered.resolve();if(count===1){await opened.promise;if(failOpen){live.delete(owner);return JSON.stringify({abi:'1',ok:false,error:{kind:'BackendFailure'}});}}
   return reply(owner);
  }
  if(frame.op==='close'){closeCalls++;if(failClose&&closeCalls===1)return JSON.stringify({abi:'1',ok:false,error:{kind:'BackendFailure'}});if(holdClose)await closed.promise;live.delete(frame.owner);}
  return reply(null);
 };
 let namespace=0;const runtime=createRuntime({open:call,call},{namespace:()=>new Uint8Array(16).fill(++namespace)}),client=new Database(runtime);
 return {client,frames,keys,live,entered,opened,closed,closeCalls:()=>closeCalls,release:()=>{opened.resolve();closed.resolve();}};
}
const open=(c,secure,name='lifecycle.sqlite',value=9)=>c.open({schema:schemas[0],migrations:[],engine:secure?{kind:'opfs',name}:{kind:'memory',store:null},...(secure?{encryptionKey:new Uint8Array(32).fill(value)}:{})});
test('S10 protected open failure makes one binary call with no plaintext fallback',async()=>{
 const frames=[];const c=new Database(createRuntime({open:async(input,key)=>{frames.push({input,key:new Uint8Array(key)});return JSON.stringify({abi:'1',ok:false,error:{kind:'BackendFailure'}});},call:()=>{throw new Error('unexpected call');}},{namespace:()=>new Uint8Array(16)}));
 await assert.rejects(open(c,true),{kind:'BackendFailure'});assert.equal(frames.length,1);assert.equal(JSON.parse(frames[0].input).args.engine.kind,'opfs');assert.equal(c.owner,null);
});
test('S10 duplicate protected or memory opens reject Busy before second channel call',async()=>{
 for(const secure of [true,false]){const f=fixture(),first=open(f.client,secure).catch(error=>error);await f.entered.promise;try{for(const duplicate of [true,false])await assert.rejects(open(f.client,duplicate),{kind:'Busy'});assert.equal(f.frames.filter(x=>x.op==='open').length,1);}finally{f.release();await first;await f.client.close();}}
});
test('S10 pending open cancellation never publishes and close waits for its exact disposal',async()=>{
 for(const secure of [true,false]){const f=fixture({holdClose:true}),opening=open(f.client,secure).then(value=>({value}),error=>({error}));await f.entered.promise;let done=false;const closing=f.client.close().then(()=>{done=true;});try{
  await tick();assert.equal(done,false);f.opened.resolve();assert.equal((await opening).error?.kind,'Closed');await tick();assert.equal(f.client.owner,null);assert.equal(f.closeCalls(),1);assert.equal(done,false);assert.equal(f.live.size,1);await assert.rejects(open(f.client,secure),{kind:'Busy'});f.closed.resolve();await closing;assert.equal(f.live.size,0);
 }finally{f.release();await opening;await closing;await f.client.close();}}
});
test('S10 close awaits failed opening without fabricating owner disposal',async()=>{
 const f=fixture({failOpen:true}),opening=open(f.client,true).catch(error=>error);await f.entered.promise;let done=false;const closing=f.client.close().then(()=>{done=true;});try{await tick();assert.equal(done,false);f.opened.resolve();assert.equal((await opening).kind,'Closed');await closing;assert.equal(f.closeCalls(),0);assert.equal(f.live.size,0);assert.equal(f.client.owner,null);}finally{f.release();await opening;await closing;await f.client.close();}
});
test('S10 failed disposal retains its handle and coalesces concurrent retries',async()=>{
 const f=fixture({failClose:true,holdClose:true}),opening=open(f.client,true).catch(error=>error);await f.entered.promise;const closing=f.client.close().catch(error=>error);try{f.opened.resolve();assert.equal((await opening).kind,'Closed');assert.equal((await closing).kind,'BackendFailure');assert.equal(f.live.size,1);assert.equal(f.client.owner,null);await assert.rejects(open(f.client,false),{kind:'Busy'});const retry=f.client.close(),shared=f.client.close();await tick();assert.equal(f.closeCalls(),2);f.closed.resolve();await Promise.all([retry,shared]);assert.equal(f.live.size,0);}finally{f.release();await opening;await closing;await f.client.close().catch(()=>{});}
});
test('S10 replacement open uses fresh caller configuration and never replays old key',async()=>{
 const f=fixture(),opening=open(f.client,true,'old.sqlite',7).catch(error=>error);await f.entered.promise;const closing=f.client.close();try{f.opened.resolve();assert.equal((await opening).kind,'Closed');await closing;await open(f.client,true,'new.sqlite',19);const opens=f.frames.filter(x=>x.op==='open');assert.deepEqual(opens.map(x=>x.args.engine.name),['old.sqlite','new.sqlite']);assert.notEqual(opens[0].args.namespace,opens[1].args.namespace);assert.deepEqual(f.keys,[new Uint8Array(32).fill(7),new Uint8Array(32).fill(19)]);assert.equal(JSON.stringify(f.client).includes('07'.repeat(32)),false);}finally{f.release();await opening;await closing;await f.client.close();}
});
test('G09 closed clients reject stateful operations without sending a retired owner frame',async()=>{
 const frames=[],runtime=createRuntime({open:()=>JSON.stringify({abi:'1',ok:true,value:'1'}),call:input=>{frames.push(JSON.parse(input));return JSON.stringify({abi:'1',ok:true,value:null});}},{namespace:()=>new Uint8Array(16)});const db=new Database(runtime,{namespace:'00'.repeat(16)});await db.open({schema:schemas[0],engine:{kind:'memory',store:null}});await db.close();const count=frames.length;await assert.rejects(db.schema(),{kind:'Closed'});await assert.rejects(db.insert('authors',{name:j.string('late')}),{kind:'Closed'});assert.equal(frames.length,count);
});
test('DEF-0046 invalid sync options reject before opening and leave the client reusable',async()=>{
 const f=fixture();f.release();try{
  await assert.rejects(f.client.open({schema:schemas[0],engine:{kind:'memory',store:null},transport:{}}),{kind:'InvalidRequest'});
  assert.equal(f.frames.some(frame=>frame.op==='open'),false);assert.equal(f.client.owner,null);
  await open(f.client,false);assert.equal(f.live.size,1);
 }finally{await f.client.close();}
});
test('DEF-0046 failed post-acquisition startup disposes the owner before rejecting',async()=>{
 const f=fixture();f.release();const start=f.client.startSync;f.client.startSync=()=>{throw Object.assign(Error('startup failed'),{kind:'InvalidRequest'});};
 try{
  await assert.rejects(f.client.open({schema:schemas[0],engine:{kind:'memory',store:null},transport:()=> '[]'}),{kind:'InvalidRequest'});
  assert.equal(f.closeCalls(),1);assert.equal(f.live.size,0);assert.equal(f.client.owner,null);assert.equal(f.client.transport,null);assert.equal(f.client.timer,null);
  f.client.startSync=start;await open(f.client,false);assert.equal(f.live.size,1);
 }finally{f.client.startSync=start;await f.client.close().catch(()=>{});}
});
test('DEF-0046 failed startup disposal retains the owner for an explicit close retry',async()=>{
 const f=fixture({failClose:true});f.release();const start=f.client.startSync;f.client.startSync=()=>{throw Object.assign(Error('startup failed'),{kind:'InvalidRequest'});};
 try{
  await assert.rejects(f.client.open({schema:schemas[0],engine:{kind:'memory',store:null},transport:()=> '[]'}),{kind:'BackendFailure'});
  assert.equal(f.closeCalls(),1);assert.equal(f.live.size,1);assert.ok(f.client.owner);await assert.rejects(open(f.client,false),{kind:'Busy'});
  await f.client.close();assert.equal(f.closeCalls(),2);assert.equal(f.live.size,0);assert.equal(f.client.owner,null);
  f.client.startSync=start;await open(f.client,false);
 }finally{f.client.startSync=start;await f.client.close().catch(()=>{});}
});
