// SPEC-0039 / TC-0043 GW15 and the GW16 managed-pass rules, scripted channel only: SDK
// translation and driver behavior, not core guard semantics.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime,j} from '../src/index.js';
import {schemas} from './schemas.mjs';
const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const bad=kind=>JSON.stringify({abi:'1',ok:false,error:{kind}});
const tick=()=>new Promise(resolve=>setImmediate(resolve));
const plan=()=>({writes:[{op:'put',table:'sensors',id:'s',fields:{label:j.string('server')}}],mappings:{}});
const guards=()=>[
 {table:'sensors',id:'s',expected:{label:j.string('seen'),note:j.null()}},
 {table:'sensors',id:'u',expected:{label:j.string('omitted note')}},
 {table:'sensors',id:'t',expected:null}];
const pendingEntry=key=>({key,outcome:{status:'applied',result:{kind:'null'}}});

async function fixture({guardedError=null,reconcileErrors={},pending=[],pages=[{envelopes:[],has_more:false}]}={}) {
 const frames=[],reports=[];let page=0;const open=new Set(pending);
 const runtime=createRuntime({open:()=>ok('1'),call:input=>{
  const f=JSON.parse(input);frames.push(f);
  if(f.op==='reconcile_resolving'&&f.args.guards.length)return guardedError?bad(guardedError):ok({kind:'completed'});
  if(f.op==='reconcile_resolving'){const error=reconcileErrors[f.args.key];if(error)return bad(error);open.delete(f.args.key);return ok({kind:'completed'});}
  if(f.op==='pending_reconciliations')return ok([...open].map(pendingEntry));
  if(f.op==='prepare_push'){const value=pages[Math.min(page,pages.length-1)];page++;return ok({next_due_ms:null,...value});}
  if(f.op==='receive_wire')return ok([]);
  if(f.op==='observe')return ok('11');
  if(f.op==='drain')return ok({events:[],has_more:false});
  return ok(null);
 }});
 const db=new Database(runtime,{namespace:'00'.repeat(16),clock:()=>10,jitter:()=>0,monotonic:()=>0,timers:{set(){return 1;},clear(){}},onBackgroundError:e=>reports.push(e)});
 await db.open({engine:{kind:'memory',store:null},schema:schemas[1]});
 const ops=name=>frames.filter(f=>f.op===name);
 return {db,frames,reports,ops,resetPages:()=>{page=0;}};
}
/** Starts the managed driver and settles its initial pass so later passes are explicit. */
async function parked(db) {
 db.startSync(async()=>'[]');clearTimeout(db.timer);db.timer=null;await db.sync();
 clearTimeout(db.timer);db.timer=null;db.syncWakePending=false;
}

test('GW15 one resolving call carries the reconcile plan encoding and lossless guards in order',async()=>{
 const {db,ops}=await fixture();
 try{
  assert.equal(await db.reconcile('A',plan()),null);
  assert.equal(await db.reconcileGuarded('A',plan(),guards()),null);
  const sent=ops('reconcile_resolving').filter(f=>f.args.guards.length);
  assert.deepEqual(sent,[{abi:'1',op:'reconcile_resolving',owner:'1',args:{key:'A',plan:plan(),guards:guards()}}]);
  assert.deepEqual(sent[0].args.plan,ops('reconcile_resolving')[0].args.plan,'plan encoded as reconcile encodes it');
  assert.equal(Object.hasOwn(sent[0].args.guards[1].expected,'note'),false,'omitted field stays omitted');
  assert.deepEqual(sent[0].args.guards[0].expected.note,{kind:'null'},'explicit Null cell stays present');
  assert.equal(sent[0].args.guards[2].expected,null,'absence is null');
 }finally{await db.close();}
});

test('GW15 arguments are captured when the call starts',async()=>{
 const {db,ops}=await fixture();
 try{
  const givenPlan=plan(),givenGuards=guards();
  const pending=db.reconcileGuarded('A',givenPlan,givenGuards);
  givenGuards[0].expected.label=j.string('mutated');givenGuards.push({table:'sensors',id:'x',expected:null});givenPlan.writes.length=0;
  await pending;
  assert.deepEqual(ops('reconcile_resolving').map(f=>f.args),[{key:'A',plan:plan(),guards:guards()}]);
 }finally{await db.close();}
});

test('GW15 core errors are returned unchanged without retry, fallback or scheduling',async()=>{
 for(const kind of ['RollbackConflict','InvalidCommand','Protocol','Busy']){
  const {db,ops,reports}=await fixture({guardedError:kind});
  try{
   await db.query('sensors').observe(()=>{});await parked(db);
   const prepares=ops('prepare_push').length,drains=ops('drain').length;
   await assert.rejects(db.reconcileGuarded('A',plan(),guards()),{kind});
   await tick();await tick();
   assert.equal(ops('reconcile_resolving').length,1,kind+': one call');
   assert.equal(ops('reconcile_resolving').filter(f=>f.args.guards.length===0).length,0,kind+': no unguarded fallback');
   assert.equal(db.syncWakePending,false,kind+': no driver wake');
   assert.equal(ops('prepare_push').length,prepares,kind+': no pass');
   assert.equal(ops('drain').length,drains,kind+': no observation drain');
   assert.deepEqual(reports,[],kind+': caller error is not a background fact');
  }finally{await db.close();}
 }
});

test('GW15 success has the same observation and driver wake effects as reconcile',async()=>{
 const effects=async call=>{
  const {db,ops}=await fixture();
  try{
   await db.query('sensors').observe(()=>{});await parked(db);
   const drains=ops('drain').length;
   assert.equal(await call(db),null);
   return {wake:db.syncWakePending||db.timer!==null,drains:ops('drain').length-drains};
  }finally{await db.close();}
 };
 const plain=await effects(db=>db.reconcile('A',plan()));
 const guarded=await effects(db=>db.reconcileGuarded('A',plan(),guards()));
 assert.equal(plain.wake,true);assert.ok(plain.drains>0);
 assert.deepEqual(guarded,plain);
});

test('GW16 managed pass reports a core RollbackConflict once and continues to later entries',async()=>{
 const {db,ops,reports}=await fixture({pending:['P1','P2'],reconcileErrors:{P1:'RollbackConflict'}});
 try{
  db.startSync(async()=>'[]',()=>plan());clearTimeout(db.timer);db.timer=null;
  await db.sync();db.stopSync();
  assert.deepEqual(ops('reconcile_resolving').map(f=>f.args.key),['P1','P2']);
  assert.deepEqual(reports,[{source:'sync',kind:'RollbackConflict',token:null}]);
  assert.deepEqual((await db.pendingReconciliations()).map(p=>p.key),['P1'],'conflicted outcome stays pending');
 }finally{await db.close();}
});

test('GW16 the conflicted key is not retried on later pages of the same pass',async()=>{
 const {db,ops,reports}=await fixture({pending:['P1','P2'],reconcileErrors:{P1:'RollbackConflict'},pages:[{envelopes:[],has_more:true},{envelopes:[],has_more:false}]});
 try{
  db.startSync(async()=>'[]',()=>plan());clearTimeout(db.timer);db.timer=null;
  await db.sync();db.stopSync();
  assert.equal(ops('prepare_push').length,2,'two pages in one pass');
  assert.deepEqual(ops('reconcile_resolving').map(f=>f.args.key),['P1','P2']);
  assert.deepEqual(reports,[{source:'sync',kind:'RollbackConflict',token:null}]);
 }finally{await db.close();}
});

test('GW16 skipping a conflict alone schedules no wake or timer',async()=>{
 const {db,ops,reports}=await fixture({pending:['P1'],reconcileErrors:{P1:'RollbackConflict'}});
 try{
  db.startSync(async()=>'[]',()=>plan());clearTimeout(db.timer);db.timer=null;
  await db.sync();
  assert.equal(db.syncWakePending,false);assert.equal(db.timer,null);
  assert.equal(ops('reconcile_resolving').length,1);assert.equal(reports.length,1);
 }finally{db.stopSync();await db.close();}
});

test('GW16 callback failures and other core errors keep the existing pass failure',async()=>{
 const cases=[
  {name:'callback throws',options:{pending:['P1','P2']},callback:()=>{throw new Error('application mapper');},reconciled:[],kind:'BackendFailure'},
  {name:'core Protocol',options:{pending:['P1','P2'],reconcileErrors:{P1:'Protocol'}},callback:()=>plan(),reconciled:['P1'],kind:'Protocol'}];
 for(const entry of cases){
  const {db,ops,reports}=await fixture(entry.options);
  try{
   db.startSync(async()=>'[]',entry.callback);clearTimeout(db.timer);db.timer=null;
   await db.sync();db.stopSync();
   assert.deepEqual(ops('reconcile_resolving').map(f=>f.args.key),entry.reconciled,entry.name+': pass stops');
   assert.deepEqual(reports,[{source:'sync',kind:entry.kind,token:null}],entry.name);
  }finally{await db.close();}
 }
});
