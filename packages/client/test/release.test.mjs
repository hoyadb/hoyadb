// API-0011 mixed-wrapper release tests. Native platform emission is a separate gate.
import test from'node:test';import assert from'node:assert/strict';
import {Database,createRuntime,j}from'../src/index.js';import {schemas}from'./schemas.mjs';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
const definition={name:'release-v1',arguments:{},reference_paths:[]};
const deferred=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};};
const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function until(check){for(let n=0;n<50;n++){if(check())return;await tick();}assert.fail('expected retry attempt did not start');}
async function result(promise){let timer;try{return await Promise.race([promise.then(()=> 'completed',error=>error.kind),new Promise(resolve=>{timer=setTimeout(()=>resolve('timeout'),100);})]);}finally{clearTimeout(timer);}}
function execute(f,options={},handler=async()=>j.null()){const pending=f.db.execute(definition,j.object({}),handler,options);void pending.catch(()=>{});return pending;}
async function fixture({registration,holdBegin=false}={}){
 const f={held:true,calls:[],listeners:new Set(),begins:0,stops:0,last:null,replyGate:deferred()};
 const channel={subscribeRelease(listener){f.calls.push('subscribe');f.last=listener;if(registration)return registration(listener);f.listeners.add(listener);let live=true;return()=>{if(live){live=false;f.stops++;f.listeners.delete(listener);}};},open:()=>reply('1'),async call(input){const frame=JSON.parse(input);f.calls.push(frame.op);let value=null;
  if(frame.op==='request')value='50';
  if(frame.op==='begin'){const accepted=!f.held;if(++f.begins===1&&holdBegin)await f.replyGate.promise;value=accepted;}
  if(frame.op==='scope_args')value=j.object({});
  if(frame.op==='scope_context')value={identity:j.null(),now_ms:'10'};
  if(frame.op==='finish')value={key:'k',result:frame.args.result};
  if(frame.op==='drain')value={events:[],has_more:false};
  return reply(value);
 }};
 f.channel=channel;f.db=new Database(createRuntime(channel),{namespace:'00'.repeat(16),context:()=>j.null(),clock:()=>10});
 f.open=()=>f.db.open({schema:schemas[1],engine:{kind:'memory',store:null}});await f.open();f.signal=()=>{f.held=false;for(const listener of [...f.listeners])listener();};
 f.cleanup=async()=>{f.replyGate.resolve();await f.db.close();await tick();};return f;
}
test('G09 foreign release wakes pending begin and unsubscribe precedes handler',async()=>{
 const f=await fixture();let handlers=0;const pending=execute(f,{},async()=>{handlers++;assert.equal(f.listeners.size,0);return j.null();});
 try{await until(()=>f.begins===1);assert.equal(f.listeners.size,1);assert.ok(f.calls.indexOf('subscribe')<f.calls.indexOf('request'));f.signal();assert.equal(await result(pending),'completed');assert.equal(handlers,1);assert.equal(f.begins,2);assert.equal(f.stops,1);}finally{await f.cleanup();await pending.catch(()=>{});}
});
test('G09 release during suspended false begin response cannot be lost',async()=>{
 const f=await fixture({holdBegin:true}),pending=execute(f);
 try{await until(()=>f.begins===1);f.signal();f.replyGate.resolve();assert.equal(await result(pending),'completed');assert.equal(f.begins,2);assert.equal(f.listeners.size,0);}finally{await f.cleanup();await pending.catch(()=>{});}
});
test('G09 native release resumes exact Busy-owned operation without remapping',async()=>{
 const f=await fixture();let calls=0;const opaque={body:'exact retained response'};const pending=f.db.resumeOwned(async()=>{calls++;if(f.held)throw {kind:'Busy'};return opaque;},f.db.owner,f.db.generation);
 try{await until(()=>calls===1);assert.equal(f.listeners.size,1);f.signal();assert.equal(await result(pending),'completed');assert.equal(await pending,opaque);assert.equal(calls,2);assert.equal(f.listeners.size,0);}finally{await f.cleanup();await pending.catch(()=>{});}
});
test('G09 cancellation removes pending native subscription and original ticket',async()=>{
 const f=await fixture(),cancel=new AbortController(),pending=execute(f,{signal:cancel.signal},async()=>assert.fail('cancelled handler'));
 try{await until(()=>f.begins===1);assert.equal(f.listeners.size,1);cancel.abort();assert.equal(await result(pending),'Cancelled');await until(()=>f.calls.includes('cancel_request'));assert.equal(f.listeners.size,0);assert.equal(f.stops,1);}finally{cancel.abort();await f.cleanup();await pending.catch(()=>{});}
});
test('G09 owner invalidation disposes Busy listener and stale notification cannot wake replacement',async()=>{
 const f=await fixture();let first=0;const old=f.db.resumeOwned(async()=>{first++;throw {kind:'Busy'};},f.db.owner,f.db.generation);
 try{await until(()=>first===1);assert.equal(f.listeners.size,1);const stale=f.last;await f.db.close();assert.equal(await result(old),'completed');assert.equal(await old,null);assert.equal(f.listeners.size,0);await f.open();let next=0;const pending=f.db.resumeOwned(async()=>{next++;if(f.held)throw {kind:'Busy'};return 'new';},f.db.owner,f.db.generation);try{await until(()=>next===1);stale();await tick();assert.equal(next,1);f.signal();assert.equal(await result(pending),'completed');assert.equal(await pending,'new');assert.equal(f.listeners.size,0);}finally{await f.cleanup();await pending.catch(()=>{});}}
 finally{await f.cleanup();await old.catch(()=>{});}
});
test('G09 invalid release registration fails statically before core request allocation',async()=>{
 for(const [registration,kind]of[[()=>{throw Error('private native cause');},'BackendFailure'],[()=>null,'InvalidRequest'],[()=>42,'InvalidRequest']]){
  const f=await fixture({registration}),pending=execute(f);
  try{assert.equal(await result(pending),kind);assert.equal(f.calls.includes('request'),false);assert.equal(f.calls.includes('begin'),false);await assert.rejects(pending,error=>error.kind===kind&&!String(error.message).includes('private'));}finally{await f.cleanup();await pending.catch(()=>{});}
 }
});
test('G09 reset invalidation retires foreign Busy wait without an external release',async()=>{
 const f=await fixture();let attempts=0;const pending=f.db.resumeOwned(async()=>{attempts++;throw {kind:'Busy'};},f.db.owner,f.db.generation);
 try{await until(()=>attempts===1);assert.equal(f.listeners.size,1);await f.db.reset();assert.equal(await result(pending),'completed');assert.equal(await pending,null);assert.equal(f.listeners.size,0);assert.equal(f.stops,1);}finally{await f.cleanup();await pending.catch(()=>{});}
});
test('G09 cancelled suspended begin removes subscription before native reply arrives',async()=>{
 const f=await fixture({holdBegin:true}),cancel=new AbortController(),pending=execute(f,{signal:cancel.signal});
 try{await until(()=>f.begins===1);assert.equal(f.listeners.size,1);cancel.abort();assert.equal(await result(pending),'Cancelled');await tick();assert.equal(f.listeners.size,0);f.replyGate.resolve();await tick();assert.equal(f.stops,1);}finally{f.replyGate.resolve();await f.cleanup();await pending.catch(()=>{});}
});
test('G09 close removes subscription while an owned native response is still suspended',async()=>{
 const f=await fixture(),replyGate=deferred();const pending=f.db.resumeOwned(()=>replyGate.promise,f.db.owner,f.db.generation);
 try{await until(()=>f.listeners.size===1);await f.db.close();assert.equal(f.listeners.size,0);assert.equal(f.stops,1);replyGate.resolve('late');assert.equal(await result(pending),'completed');assert.equal(f.listeners.size,0);}finally{replyGate.resolve('late');await f.cleanup();await pending.catch(()=>{});}
});
