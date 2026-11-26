// SPEC-0044 / TC-0048 QUALITY05/06, regression for DEF-0041.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createRuntime} from '../src/runtime.js';
import {createWorkerChannel} from '../src/web.js';
import * as node from '../src/node-entry.js';

class WorkerStub {
 constructor(){this.handlers=new Map();}
 addEventListener(name,fn){const set=this.handlers.get(name)||new Set();set.add(fn);this.handlers.set(name,set);}
 removeEventListener(name,fn){this.handlers.get(name)?.delete(fn);}
 postMessage(){}
 emit(name,data){for(const fn of this.handlers.get(name)||[])fn({data,preventDefault(){}});}
 terminate(){this.terminated=true;}
}

test('QUALITY05 failed browser bootstrap is retryable without splitting concurrent callers',async()=>{
 const old=Object.getOwnPropertyDescriptor(globalThis,'Worker'),workers=[];let ownedRuntime;
 Object.defineProperty(globalThis,'Worker',{configurable:true,value:class extends WorkerStub{constructor(){super();workers.push(this);}}});
 try{
  // The pinned Node runner isolates files; finally also closes our cached realm.
  const {createWebRuntime}=await import('../src/web.js');
  const first=createWebRuntime(),concurrent=createWebRuntime();
  assert.equal(first,concurrent);assert.equal(workers.length,1);
  workers[0].emit('error');await assert.rejects(first,{kind:'BackendFailure'});
  assert.equal(workers[0].terminated,true);
  const replacement=createWebRuntime();assert.notEqual(replacement,first);
  assert.equal(workers.length,2);workers[1].emit('message',{kind:'ready'});
  const runtime=ownedRuntime=await replacement;assert.equal(await createWebRuntime(),runtime);
  const explicit=new WorkerStub(),failed=createWebRuntime({worker:explicit});
  explicit.emit('error');await assert.rejects(failed,{kind:'BackendFailure'});
  assert.equal(await createWebRuntime(),runtime);assert.equal(workers.length,2);
 }finally{ownedRuntime?.channel.close();if(old)Object.defineProperty(globalThis,'Worker',old);else delete globalThis.Worker;}
});

function runtimeWithEntropy(namespace){
 return createRuntime({open(){throw new Error('no database I/O');},call(){throw new Error('no database I/O');}},{namespace});
}
test('QUALITY06 jitter rejection boundary and exhaustion have exact bounded outcomes',async()=>{
 let calls=0;
 const exhausted=runtimeWithEntropy(()=>{calls++;return new Uint8Array(16).fill(255);});
 await assert.rejects(exhausted.jitter(),{kind:'BackendFailure'});assert.equal(calls,16);
 for(const [sample,expected] of [[0,0],[2500,2500],[2501,0],[62524,2500]]){
  const bytes=new Uint8Array(16).fill(255);bytes[0]=sample>>8;bytes[1]=sample&255;
  assert.equal(await runtimeWithEntropy(()=>bytes).jitter(),expected);
 }
 let attempts=0;
 const retry=runtimeWithEntropy(()=>{attempts++;const bytes=new Uint8Array(16).fill(255);bytes[0]=62525>>8;bytes[1]=62525&255;if(attempts===2){bytes[2]=0;bytes[3]=7;}return bytes;});
 assert.equal(await retry.jitter(),7);assert.equal(attempts,2);
});

test('QUALITY03 an unexpected worker reply retires pending and subsequent requests',async()=>{
 const worker=new WorkerStub(),channel=createWorkerChannel(worker);
 worker.emit('message',{kind:'ready'});const pending=channel.call('owned request');
 await new Promise(resolve=>setImmediate(resolve));
 worker.emit('message',{kind:'reply',value:'opaque',unexpected:true});
 await assert.rejects(pending,{kind:'InvalidPayload'});
 assert.equal(worker.terminated,true);
 await assert.rejects(channel.call('later'),{kind:'Closed'});
});

test('QUALITY03 Node protected opens reject before attempting memory-only WASM bootstrap',async()=>{
 const schema=node.defineSchema({version:1,tables:{items:node.table({columns:{title:node.column.string()}})}});
 for(const engine of [{filename:'quality.sqlite'},{engine:{kind:'opfs',name:'quality.sqlite'}}]){
  await assert.rejects(node.openDatabase({schema,...engine}),{kind:'InvalidRequest'});
  await assert.rejects(node.openDatabase({schema,...engine,encryptionKey:new Uint8Array(32)}),{kind:'UnsupportedOperation'});
 }
});
