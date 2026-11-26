// TC-0048 QUALITY10, regression for DEF-0043.
import test from 'node:test';
import assert from 'node:assert/strict';

class WorkerStub {
 constructor(){this.handlers=new Map();}
 addEventListener(name,fn){const set=this.handlers.get(name)||new Set();set.add(fn);this.handlers.set(name,set);}
 removeEventListener(name,fn){this.handlers.get(name)?.delete(fn);}
 postMessage(){}
 emit(name,data){for(const fn of this.handlers.get(name)||[])fn({data,preventDefault(){}});}
 terminate(){this.terminated=true;}
}


test('QUALITY10 new opens recover after worker crash while existing handles stay closed',async()=>{
 const old=Object.getOwnPropertyDescriptor(globalThis,'Worker'),workers=[];let ownedRuntime;
 Object.defineProperty(globalThis,'Worker',{configurable:true,value:class extends WorkerStub{constructor(){super();workers.push(this);}}});
 try{
  const {createWebRuntime}=await import('../src/web.js');
  const pending=createWebRuntime();workers[0].emit('message',{kind:'ready'});
  const stale=await pending,active=stale.call('owned-call');
  await new Promise(resolve=>setImmediate(resolve));
  workers[0].emit('error');await assert.rejects(active,{kind:'BackendFailure'});
  await assert.rejects(stale.call('later'),{kind:'Closed'});
  const fresh=createWebRuntime();assert.notEqual(fresh,pending);
  assert.equal(createWebRuntime(),fresh);assert.equal(workers.length,2);
  workers[0].emit('message',{kind:'reply',value:'late'});workers[0].emit('error');
  assert.equal(createWebRuntime(),fresh);
  const explicit=new WorkerStub(),failed=createWebRuntime({worker:explicit});
  explicit.emit('error');await assert.rejects(failed,{kind:'BackendFailure'});
  assert.equal(createWebRuntime(),fresh);
  workers[1].emit('message',{kind:'ready'});const replacement=ownedRuntime=await fresh;
  assert.notEqual(replacement,stale);assert.equal(await createWebRuntime(),replacement);
  await assert.rejects(stale.call('still closed'),{kind:'Closed'});
 }finally{ownedRuntime?.channel.close();if(old)Object.defineProperty(globalThis,'Worker',old);else delete globalThis.Worker;}
});
