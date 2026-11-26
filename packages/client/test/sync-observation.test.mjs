// DEF-0012: scheduler control is a mock Channel assertion; actual events use consumer/conformance.mjs.
import test from'node:test';import assert from'node:assert/strict';import{Database,createRuntime}from'../src/index.js';import{schemas}from'./schemas.mjs';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
test('G05 preparing and receiving do not self-wake the managed sync driver',async()=>{
 let prepares=0,pushes=0;let second;const emptyCheck=new Promise(resolve=>{second=resolve;});
 const runtime=createRuntime({open:()=>reply('1'),call:input=>{const frame=JSON.parse(input);if(frame.op==='prepare_push'){prepares++;if(prepares===2)second();return reply({envelopes:prepares===1?['{}']:[],has_more:false,next_due_ms:null});}if(frame.op==='receive_wire')return reply([]);if(frame.op==='drain')return reply({events:[],has_more:false});return reply(null);}});
 const db=new Database(runtime,{namespace:'00'.repeat(16),clock:()=>1000,jitter:()=>0});await db.open({schema:schemas[1],engine:{kind:'memory',store:null}});
 let watchdog;
 try{db.startSync(async body=>{assert.equal(body,'[{}]');pushes++;return'[]';});await Promise.race([emptyCheck,new Promise((_,reject)=>{watchdog=setTimeout(()=>reject(Error('empty due check missing')),1000);})]);await new Promise(resolve=>setTimeout(resolve,30));assert.equal(pushes,1);assert.equal(prepares,2,'notification delivery must not wake its own empty prepare');assert.equal(db.syncing,false);assert.equal(db.syncWakePending,false);assert.deepEqual(db.syncErrors,[]);}finally{clearTimeout(watchdog);await db.close();}
});
