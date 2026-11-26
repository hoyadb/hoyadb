// TC-0043 GW16b (a379e8b) deterministic scripted scheduling controls: a skipped managed
// conflict does not self-loop. Added after the SDK implementation; scripted channel only.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime,j} from '../src/index.js';
import {schemas} from './schemas.mjs';
const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const bad=kind=>JSON.stringify({abi:'1',ok:false,error:{kind}});
const idle=()=>new Promise(resolve=>setTimeout(resolve,40));
const plan={writes:[{op:'put',table:'sensors',id:'s',fields:{label:j.string('server')}}],mappings:{}};

async function run(pending,conflicts){
 const frames=[],reports=[],callbacks=[];const open=new Set(pending);
 const runtime=createRuntime({open:()=>ok('1'),call:input=>{
  const f=JSON.parse(input);frames.push(f);
  if(f.op==='reconcile_resolving'){if(conflicts.has(f.args.key))return bad('RollbackConflict');open.delete(f.args.key);return ok({kind:'completed'});}
  if(f.op==='pending_reconciliations')return ok([...open].map(key=>({key,outcome:{status:'applied',result:{kind:'null'}}})));
  if(f.op==='prepare_push')return ok({envelopes:[],next_due_ms:null,has_more:false});
  if(f.op==='drain')return ok({events:[],has_more:false});
  return ok(null);
 }});
 const db=new Database(runtime,{namespace:'00'.repeat(16),clock:()=>10,jitter:()=>0,monotonic:()=>0,timers:{set(){return 1;},clear(){}},onBackgroundError:e=>reports.push(e)});
 await db.open({engine:{kind:'memory',store:null},schema:schemas[1]});
 db.startSync(async()=>'[]',pendingEntry=>{callbacks.push(pendingEntry.key);return plan;});
 for(let i=0;i<10;i++)await idle();
 const snapshot={
  prepares:frames.filter(f=>f.op==='prepare_push').length,
  reconciles:key=>frames.filter(f=>f.op==='reconcile_resolving'&&f.args.key===key).length,
  callbacks:key=>callbacks.filter(k=>k===key).length,reports:reports.length,
  // A fired Node Timeout stays referenced but destroyed; only a live handle is a pending timer.
  idle:!db.syncing&&!db.syncWakePending&&(db.timer===null||db.timer._destroyed===true)};
 for(let i=0;i<5;i++)await idle();
 const settled=frames.filter(f=>f.op==='prepare_push').length===snapshot.prepares;
 db.stopSync();await db.close();
 return {...snapshot,settled,reportKinds:reports.map(r=>r.kind)};
}

test('GW16b only a conflicting X: one callback, reconcile and report, then idle',async()=>{
 const result=await run(['X'],new Set(['X']));
 assert.equal(result.callbacks('X'),1);assert.equal(result.reconciles('X'),1);assert.equal(result.reports,1);assert.deepEqual(result.reportKinds,['RollbackConflict']);
 assert.equal(result.prepares,1,'no additional pass');assert.equal(result.idle,true,'no wake or timer');assert.equal(result.settled,true);
});

test('GW16b conflicting X plus successful Y: the success wake causes exactly one further pass',async()=>{
 const result=await run(['X','Y'],new Set(['X']));
 assert.equal(result.callbacks('Y'),1);assert.equal(result.reconciles('Y'),1);
 assert.equal(result.callbacks('X'),2);assert.equal(result.reconciles('X'),2);assert.equal(result.reports,2);
 assert.deepEqual(result.reportKinds,['RollbackConflict','RollbackConflict']);
 assert.equal(result.prepares,2,'exactly one further pass');assert.equal(result.idle,true);assert.equal(result.settled,true);
});
