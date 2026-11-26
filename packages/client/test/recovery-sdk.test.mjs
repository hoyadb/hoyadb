// SPEC-0038 / TC-0042 SDK1-2: channel translation and managed-driver wake only.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime} from '../src/index.js';
import {schemas} from './schemas.mjs';
const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const bad=kind=>JSON.stringify({abi:'1',ok:false,error:{kind}});
const tick=()=>new Promise(resolve=>setImmediate(resolve));
const entry=awaiting_key=>({kind:'command',table:null,record_id:null,key:'B',state:'queued',blocked_on:[],awaiting_local:false,awaiting_key,rolled_back:false,attempts:'0',next_due_ms:null});
const status=awaiting=>({pending:'1',sending:'0',awaiting_outcome:'0',live:'1',blocked:false,commands:[entry(awaiting)]});
async function fixture({error=null}={}) {
 const frames=[],reports=[],pending=[];let ready=false,prepared=0,delivered=false;
 const frozen='{"idempotencyKey":"A","args":{"value":42.0}}';
 const runtime=createRuntime({open:()=>ok('1'),call:input=>{
  const f=JSON.parse(input);frames.push(f);
  if(f.op==='queue_status')return ok(status('A'));
  if(f.op==='observe_status')return ok('11');
  if(f.op==='drain')return ok({events:delivered?[]:(delivered=true,[{kind:'status',subscription:'11',value:status('A')}]),has_more:false});
  if(f.op==='request_redelivery'){if(error)return bad(error);ready=true;return ok(null);}
  if(f.op==='prepare_push'){prepared++;const envelopes=ready?[frozen]:[];ready=false;return ok({envelopes,next_due_ms:null,has_more:false});}
  if(f.op==='receive_wire')return ok([]);
  return ok(null);
 }});
 const db=new Database(runtime,{namespace:'00'.repeat(16),clock:()=>10,jitter:()=>0,monotonic:()=>0,timers:{set(fn){pending.push(fn);return pending.length;},clear(){}},onBackgroundError:e=>reports.push(e)});
 await db.open({engine:{kind:'memory',store:null},schema:schemas[1]});
 return {db,frames,reports,pending,frozen,prepared:()=>prepared};
}
test('SDK1 status fetch and observation preserve awaiting_key (wire passthrough characterization)',async()=>{
 const {db}=await fixture();const seen=[];
 try{assert.equal((await db.queueStatus()).commands[0].awaiting_key,'A');await db.observeStatus(e=>seen.push(e.value));await db.drain();assert.deepEqual(seen,[status('A')]);}
 finally{await db.close();}
});
test('SDK2 explicit rearm carries lossless sample once with no driver or transport',async()=>{
 const {db,frames,reports}=await fixture();
 try{
  assert.equal(await db.requestRedelivery('A','253402300799999'),null);
  assert.deepEqual(frames.filter(f=>f.op==='request_redelivery'),[{abi:'1',op:'request_redelivery',owner:'1',args:{key:'A',now_ms:'253402300799999'}}]);
  assert.equal(frames.filter(f=>f.op==='prepare_push').length,0);assert.deepEqual(reports,[]);
 }finally{await db.close();}
});
test('SDK2 rearm Protocol and Busy are returned unchanged without retry',async()=>{
 for(const error of ['Protocol','Busy','InvalidCommand']){
  const {db,frames,reports}=await fixture({error});
  try{await assert.rejects(db.requestRedelivery('A',-1),{kind:error});assert.equal(frames.filter(f=>f.op==='request_redelivery').length,1);assert.deepEqual(reports,[]);}
  finally{await db.close();}
 }
});
test('SDK2 explicit rearm wakes a parked managed driver and sends frozen bytes once',async()=>{
 const {db,frames,frozen,prepared,reports}=await fixture();const sent=[];
 try{
  db.startSync(async body=>{sent.push(body);return '[]';});
  // Settle the driver's initial empty batch and remove any initial wake timer.
  clearTimeout(db.timer);db.timer=null;await db.sync();
  assert.ok(prepared()>0);assert.deepEqual(sent,[]);
  await db.requestRedelivery('A',10);
  for(let i=0;i<100&&sent.length===0;i++)await new Promise(resolve=>setTimeout(resolve,1));
  await tick();
  assert.deepEqual(sent,['['+frozen+']']);
  assert.equal(frames.filter(f=>f.op==='request_redelivery').length,1);assert.deepEqual(reports,[]);
 }finally{await db.close();}
});
