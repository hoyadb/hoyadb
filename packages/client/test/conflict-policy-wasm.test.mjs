// TC-0044 CP01/02/04/07/09/13: actual WASM core plus the application SDK.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import init,{Facade} from '../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {createRuntime,j} from '../src/index.js';
import {openApplicationDatabase,defineCommand} from '../src/application.js';
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});
const schema={version:'1',tables:{notes:{columns:{body:{kind:'String',optional:false}},relations:{}}}};
const plan={writes:[],mappings:{}};
async function fixture(options={}){
 const facade=new Facade(),runtime=createRuntime({open:(input,key)=>facade.open(input,key),call:input=>facade.call(input)});
 const store=await runtime.createMemoryStore(),reports=[];let ids=0;
 const config={runtime,schema,engine:{kind:'memory',store},namespace:'47'.repeat(16),context:()=>null,clock:()=>1000,jitter:()=>0,idGenerator:()=>++ids===1?'note':`outside-${ids}`,onResolution:(key,result)=>reports.push([key,result]),...options};
 const db=await openApplicationDatabase(config);
 const edit=db.command(defineCommand('edit-v1',{args:{},handler:db=>db.table('notes').findById('note').update({body:'optimistic'})}));
 await db.table('notes').insert({body:'initial'});
 return {db,runtime,store,config,reports,edit,async close(){await db.close();await runtime.dropMemoryStore(store);facade.free();}};
}
async function rejected(f){
 const {key}=await f.edit.executeWithReceipt({});await f.db.preparePush(1000,0);
 await f.db.receive([{idempotencyKey:key,status:'rejected',reason:'server'}]);
 await f.db.reconcile(key,plan);
 await f.db.table('notes').findById('note').update({body:'newer'});return key;
}

test('CP01/02/04 default Stop, exact override, Discarded recovery and reopen reset',async()=>{
 const f=await fixture();let reopened;
 try{const key=await rejected(f);await assert.rejects(f.db.rollbackResolving(key),{kind:'RollbackConflict'});
  await f.db.setConflictPolicy({default:'discard',commands:{'edit-v1':'stop'}});await assert.rejects(f.db.rollback(key),{kind:'RollbackConflict'});
  await f.db.setConflictPolicy({default:'stop',commands:{'edit-v1':'discard'}});
  const result=await f.db.rollbackResolving(key);assert.deepEqual(result,{kind:'kept_current',discarded_keys:[key],unmapped:[],skipped_writes:'0'});
  assert.deepEqual(f.reports,[[key,result]]);assert.equal((await f.db.table('notes').findById('note').fetch()).body,'newer');
  assert.equal((await f.db.queueStatus()).commands.find(x=>x.key===key).state,'discarded');
  assert.deepEqual(await f.db.pendingReconciliations(),[]);await f.db.close();
  reopened=await openApplicationDatabase(f.config);assert.equal((await reopened.queueStatus()).commands.find(x=>x.key===key).state,'discarded');
  const edit=reopened.command(defineCommand('edit-v1',{args:{},handler:db=>db.table('notes').findById('note').update({body:'again'})}));
  const next=(await edit.executeWithReceipt({})).key;await reopened.preparePush(1000,0);await reopened.receive([{idempotencyKey:next,status:'rejected',reason:'server'}]);
  await reopened.reconcile(next,plan);
  await reopened.table('notes').findById('note').update({body:'after reopen'});await assert.rejects(reopened.rollbackResolving(next),{kind:'RollbackConflict'});
 }finally{await reopened?.close();await f.close();}
});

test('CP07 correction Discard skips writes but preserves explicit guard failure and terminal state',async()=>{
 const f=await fixture({conflict:{default:'discard'}});
 try{const {key}=await f.edit.executeWithReceipt({});await f.db.preparePush(1000,0);await f.db.receive([{idempotencyKey:key,status:'corrected',result:null}]);
  await f.db.table('notes').findById('note').update({body:'newer'});
  const outside=await f.db.table('notes').insert({body:'outside'});
  const rawPlan={writes:[{op:'put',table:'notes',id:'note',fields:{body:j.string('server')}},{op:'put',table:'notes',id:outside.id,fields:{body:j.string('server outside')}}],mappings:{}};
  await assert.rejects(f.db.raw.reconcileResolving(key,rawPlan,[{table:'notes',id:outside.id,expected:{body:j.string('stale')}}]),{kind:'RollbackConflict'});
  assert.deepEqual(f.reports,[]);
  const result=await f.db.reconcileResolving(key,{writes:[{op:'put',table:'notes',id:'note',fields:{body:'server'}}],mappings:{}});
  assert.deepEqual(result,{kind:'kept_current',discarded_keys:[],unmapped:[],skipped_writes:'1'});assert.deepEqual(f.reports,[[key,result]]);
  assert.equal((await f.db.table('notes').findById('note').fetch()).body,'newer');assert.equal((await f.db.queueStatus()).commands.find(x=>x.key===key).state,'corrected');
  assert.equal((await f.db.table('notes').findById(outside.id).fetch()).body,'outside');
 }finally{await f.close();}
});

test('CP09 manual discard keeps optimistic rows and Completed does not notify',async()=>{
 const f=await fixture();
 try{const {key}=await f.edit.executeWithReceipt({});assert.deepEqual(await f.db.discard(key),{kind:'kept_current',discarded_keys:[key],unmapped:[],skipped_writes:'0'});
  assert.equal((await f.db.table('notes').findById('note').fetch()).body,'optimistic');assert.equal(f.reports.length,1);
  await f.db.forget(key);const next=(await f.edit.executeWithReceipt({})).key;await f.db.preparePush(1000,0);await f.db.receive([{idempotencyKey:next,status:'applied',result:null}]);
  assert.deepEqual(await f.db.reconcileResolving(next,plan),{kind:'completed'});assert.equal(f.reports.length,1);
 }finally{await f.close();}
});

test('CP13 admitted handler rejects policy update Busy and leaves constructor policy intact',async()=>{
 const f=await fixture({conflict:{default:'stop'}});let entered,release;
 const started=new Promise(resolve=>{entered=resolve;}),held=new Promise(resolve=>{release=resolve;});
 const active=f.db.command(defineCommand('held-v1',{args:{},async handler(){entered();await held;return null;}})).execute({});
 try{await started;await assert.rejects(f.db.setConflictPolicy({default:'discard'}),{kind:'Busy'});
  release();await active;const key=await rejected(f);await assert.rejects(f.db.rollbackResolving(key),{kind:'RollbackConflict'});
  assert.deepEqual(f.reports,[]);
 }finally{release();await active;await f.close();}
});
