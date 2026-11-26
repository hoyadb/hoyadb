// API-0012 / TC-0046. Real WASM integration, not a replacement database.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import init,{Facade} from '../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {createRuntime,j,openDatabase as openHost} from '../src/index.js';
import {openApplicationDatabase,defineCommand} from '../src/application.js';
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});
const col=(kind,optional=false)=>({kind,optional});
const schema={version:'1',tables:{
 projects:{columns:{name:col('String')},relations:{tasks:{kind:'HasMany',table:'tasks',foreign_key:'project_id'}}},
 tasks:{columns:{title:col('String'),done:col('Boolean'),priority:col('Integer'),at:col('Timestamp'),note:col('String',true)},relations:{project:{kind:'ToOne',table:'projects',foreign_key:'project_id',optional:false}}},
 notes:{columns:{body:col('String'),update:col('String',true)},relations:{}}
}};
const fields=(title,project)=>({title,done:false,priority:2,at:new Date(1000),project_id:project});
const until=async fn=>{for(let n=0;n<200;n++){if(await fn())return;await new Promise(r=>setTimeout(r,2));}assert.fail('host-turn condition timed out');};
async function fixture(extra={}){
 const facade=new Facade();const runtime=createRuntime({open:(s,k)=>facade.open(s,k),call:s=>facade.call(s)},{namespace:()=>new Uint8Array(16).fill(4)});
 let next=0;const options={runtime,schema,engine:'memory',clock:()=>new Date(1000),jitter:()=>0,idGenerator:()=>`local:app-${++next}`,...extra};
 const db=await openApplicationDatabase(options);assert.ok(db,'application open returns a database');
 return {db,runtime,options,async close(){await db.close();facade.free();}};
}
test('APP01/02 plain rows, symbolic queries, lazy relations and atomic partial updates',async()=>{
 const f=await fixture();try{
  const project=await f.db.table('projects').insert({name:'Launch'});
  const row=await f.db.table('tasks').insert(fields('Write',project.id));
  assert.equal(row.title,'Write');assert.ok(row.at instanceof Date);assert.equal(row.priority,2);
  assert.equal((await f.db.query('tasks').where('project.name','=','Launch').orderBy('priority','desc').limit(1).fetch())[0].id,row.id);
  assert.equal((await project.tasks.fetch())[0].title,'Write');assert.equal((await row.project.fetch())[0].name,'Launch');
  await f.db.table('tasks').findById(row.id).update({title:'Updated'});
  const updated=await f.db.table('tasks').findById(row.id).fetch();assert.equal(updated.title,'Updated');assert.equal(updated.done,false);
  await assert.rejects(f.db.table('tasks').insert({...fields('bad',project.id),priority:9007199254740992}),{kind:'InvalidRow'});
  assert.equal(await f.db.table('tasks').findById(row.id).remove(),true);
 }finally{await f.close();}
});
test('APP03 immediate unsubscribe, initial and field-update snapshots, callback retirement',async()=>{
 const facts=[];const f=await fixture({onBackgroundError:fact=>facts.push(fact)});try{
  const row=await f.db.table('notes').insert({body:'before'});const seen=[];
  const stop=f.db.query('notes').observe(rows=>{seen.push(rows.map(r=>r.body));});assert.equal(typeof stop,'function');await stop.ready;
  await until(()=>seen.length===1);await f.db.table('notes').findById(row.id).update({body:'after'});await until(()=>seen.at(-1)?.[0]==='after');
  await stop();await stop();await f.db.table('notes').findById(row.id).update({body:'last'});assert.equal(seen.length,2);
  let early=0;const retired=f.db.query('notes').observe(()=>early++);await retired();await retired.ready;await new Promise(r=>setTimeout(r,5));assert.equal(early,0);
  const broken=f.db.query('notes').observe(()=>{throw Error('private');});await broken.ready;await until(()=>facts.length>0);assert.equal(facts[0].source,'callback');assert.equal(facts[0].kind,'CallbackFailure');await broken();
 }finally{await f.close();}
});
test('APP04 scoped async handlers, captured identity, rollback and explicit database binding',async()=>{
 let ids=0;const f=await fixture({context:()=>({user:{id:'ada'}}),idGenerator:()=>++ids===1?'project-1':`local:handler-${ids}`}),g=await fixture();try{
  const project=await f.db.table('projects').insert({name:'P'});
  const definition=defineCommand('add-task-v1',{args:{title:'string',project:'reference'},async handler(db,args,ctx){
   assert.equal(ctx.identity.user.id,'ada');assert.throws(()=>{ctx.identity.user.id='evil';},TypeError);assert.equal(ctx.now().getTime(),1000);
   assert.equal(db.command,undefined);await assert.rejects(f.db.table('notes').insert({body:'outer'}),{kind:'Busy'});
   const task=await db.table('tasks').insert(fields(args.title,args.project));await Promise.resolve();
   assert.equal((await db.query('tasks').where('id','=',task.id).fetch()).length,1);return task;
  }});
  const add=f.db.command(definition);const receipt=await add.executeWithReceipt({title:'Ship',project:project.id});assert.equal(receipt.result.title,'Ship');assert.ok(receipt.result.at instanceof Date);
  assert.equal((await f.db.queueStatus()).commands.length,1);assert.equal((await g.db.query('tasks').fetch()).length,0);
  const fail=f.db.command(defineCommand('fail-v1',{args:{},async handler(db){await db.table('notes').insert({body:'never'});throw Error('abort');}}));
  await assert.rejects(fail.execute({}),/abort/);assert.equal((await f.db.query('notes').fetch()).length,0);assert.equal((await f.db.queueStatus()).commands.length,1);
 }finally{await f.close();await g.close();}
});
test('APP05/07 dependencies wait for all explicit mappings and survive retained-store reopen',async()=>{
 const f=await fixture();try{
  const store=await f.runtime.createMemoryStore();await f.db.close();
  const db=await openApplicationDatabase({...f.options,engine:{kind:'memory',store}});
  const make=db.command(defineCommand('make-v1',{args:{},handler:db=>db.table('notes').insert({body:'parent'})}));
  const first=await make.executeWithReceipt({});
  const child=await db.command(defineCommand('use-v1',{args:{parent:'reference',literal:'string'},handler:()=>null})).executeWithReceipt({parent:first.result.id,literal:'local:business-text'});
  const batch=await db.preparePush(1000,0);assert.equal(batch.envelopes.length,1);assert.equal(batch.envelopes[0].command,'make-v1');
  await db.receive([{idempotencyKey:first.key,status:'applied',result:{mappings:{[first.result.id]:'remote-parent'}}}]);await db.close();
  const reopened=await openApplicationDatabase({...f.options,engine:{kind:'memory',store},transport:{push:()=>[]},reconcile:p=>({writes:[],mappings:p.outcome.result.mappings??{}})});
  await reopened.sync();assert.equal((await reopened.query('notes').where('id','=','remote-parent').fetch()).length,1);
  const next=await reopened.preparePush(1000,0);assert.equal(next.envelopes[0].idempotencyKey,child.key);assert.equal(next.envelopes[0].args.parent,'remote-parent');assert.equal(next.envelopes[0].args.literal,'local:business-text');
  await reopened.close();await f.runtime.dropMemoryStore(store);
 }finally{await f.close();}
});
test('APP06 entities are opt-in, record names do not overwrite fields, patch stays atomic',async()=>{
 let ids=0;const f=await fixture({idGenerator:()=>++ids===1?'note-1':`local:entity-${ids}`});try{
  const row=await f.db.table('notes').insert({body:'one',update:'data'});assert.equal(row.update,'data');assert.equal((await f.db.queueStatus()).commands.length,0);
  await f.db.configureEntities('app',['notes']);await f.db.table('notes').findById(row.id).update({body:'two'});
  assert.equal((await f.db.queueStatus()).commands.at(-1).kind,'entity');assert.equal((await f.db.table('notes').findById(row.id).fetch()).update,'data');
  let release,entered;const waiting=new Promise(r=>entered=r);const finish=new Promise(r=>release=r);
  const run=f.db.command(defineCommand('patch-v1',{args:{},async handler(db){await db.table('notes').findById(row.id).update({body:'scoped'});entered();await finish;return null;}})).execute({});
  await waiting;await assert.rejects(f.db.table('notes').findById(row.id).update({update:'lost'}),{kind:'Busy'});release();await run;
  assert.equal((await f.db.table('notes').findById(row.id).fetch()).update,'data');
 }finally{await f.close();}
});
test('APP08 app pull schedules once, skips Busy, reports failures and stops pending responses',async()=>{
 const facts=[];const f=await fixture({onBackgroundError:x=>facts.push(x)});try{
  await f.db.configureEntities('app',['notes']);let calls=0;
  const transport={push:()=>[],pull:async()=>{calls++;return {checkpoint:'p',complete:true,changes:[]};}};
  f.db.startSync(transport,null,{pullIntervalMs:15});await until(()=>calls>=2);f.db.stopSync();const count=calls;await new Promise(r=>setTimeout(r,25));assert.equal(calls,count);
  let respond,started;const pending=new Promise(r=>started=r);f.db.startSync({push:()=>[],pull:()=>{started();return new Promise(r=>respond=r);}},null,{pullIntervalMs:10});
  await pending;f.db.stopSync();respond({checkpoint:'stale',complete:true,changes:[]});await new Promise(r=>setTimeout(r,10));assert.equal((await f.db.syncProgress()).pages,'2');
  f.db.startSync({push:()=>[],pull:()=>{throw Object.assign(Error(),{kind:'BackendFailure'});}},null,{pullIntervalMs:10});await until(()=>facts.some(x=>x.source==='sync'));f.db.stopSync();
 }finally{await f.close();}
});
test('APP02 tagged numeric schema caller keeps raw behavior and key failure precedes open',async()=>{
 const f=await fixture();try{
  let opens=0;const runtime={...f.runtime,open:()=>{opens++;throw Error('should not open');}};
  await assert.rejects(openApplicationDatabase({schema,runtime,filename:'protected'}),{kind:'InvalidRequest'});assert.equal(opens,0);
  await assert.rejects(openHost({runtime:f.runtime,schema:{...schema,version:1},engine:{kind:'memory',store:null},namespace:'05'.repeat(16)}),{kind:'InvalidRequest'});
  const db=await openHost({runtime:f.runtime,schema,engine:{kind:'memory',store:null},namespace:'05'.repeat(16)});
  const row=await db.insert('notes',{body:j.string('raw')});assert.equal(row.fields.body.kind,'string');await db.close();
 }finally{await f.close();}
});
test('APP04 nested typed records keep Dates, own data and relation fields',async()=>{
 let ids=0;const f=await fixture({idGenerator:()=>++ids===1?'project-nested':`local:nested-${ids}`});try{
  const p=await f.db.table('projects').insert({name:'P'});
  const result=await f.db.command(defineCommand('nested-v1',{args:{},async handler(db){const row=await db.table('tasks').insert(fields('nested',p.id));return {rows:[row],again:row};}})).execute({});
  assert.ok(result.rows[0].at instanceof Date);assert.equal(result.rows[0].at.getTime(),1000);assert.equal(result.again.project_id,p.id);assert.equal((await result.again.project.fetch())[0].name,'P');
 }finally{await f.close();}
});
test('APP08 awaited pull rejects while background pull skips Busy and reports once',async()=>{
 const facts=[];const f=await fixture({onBackgroundError:x=>facts.push(x),transport:{push:()=>[],pull:()=>{throw Object.assign(Error(),{kind:'BackendFailure'});}}});try{
  await f.db.configureEntities('app',['notes']);await assert.rejects(f.db.sync(),{kind:'BackendFailure'});assert.equal(facts.length,0);
  const original=f.db.raw.pullOnce.bind(f.db.raw);f.db.raw.pullOnce=()=>Promise.reject(Object.assign(Error(),{kind:'Busy'}));
  f.db.startSync(f.options.transport,null,{pullIntervalMs:10});await new Promise(r=>setTimeout(r,25));f.db.stopSync();assert.equal(facts.length,0);f.db.raw.pullOnce=original;
 }finally{await f.close();}
});
test('APP01 relation navigation rejects inherited names',async()=>{
 const f=await fixture();try{
  const p=await f.db.table('projects').insert({name:'P'});
  assert.throws(()=>f.db.related('projects',p.id,'constructor'),{kind:'InvalidQuery'});
  await assert.rejects(f.db.table('projects').link(p.id,'constructor','target'),{kind:'InvalidQuery'});
 }finally{await f.close();}
});
test('APP05 persisted creation facts and entity identity fill survive reopen',async()=>{
 const f=await fixture();try{
  const store=await f.runtime.createMemoryStore();await f.db.close();
  const options={...f.options,engine:{kind:'memory',store},entities:{scope:'app',tables:['notes']}};
  const db=await openApplicationDatabase(options);const row=await db.table('notes').insert({body:'created'});const batch=await db.preparePush(1000,0);
  await db.receive([{table:'notes',recordId:row.id,writeId:batch.envelopes[0].writeId,status:'applied',result:null}]);await db.close();
  const reopened=await openApplicationDatabase({...options,entities:undefined});
  const [pending]=await reopened.pendingReconciliations();assert.equal(pending.kind,'entity');assert.deepEqual(pending.created,{[row.id]:'notes'});
  const actual=reopened.raw.reconcile.bind(reopened.raw);let captured;reopened.raw.reconcile=async(key,plan)=>{captured=plan;};
  await reopened.reconcile(pending.key,{writes:[],mappings:{[row.id]:'server:canonical'}});assert.equal(captured.mappings[row.id],'server:canonical');
  await reopened.reconcile(pending.key,{writes:[],mappings:{extra:'preserved'}});assert.deepEqual(captured.mappings,{[row.id]:row.id,extra:'preserved'});
  reopened.raw.reconcile=actual;await reopened.reconcile(pending.key,{writes:[],mappings:{}});assert.equal((await reopened.pendingReconciliations()).length,0);
  assert.equal((await reopened.query('notes').fetch())[0].id,row.id);await reopened.close();await f.runtime.dropMemoryStore(store);
 }finally{await f.close();}
});
test('APP08 stop releases a held pull ticket and reset can proceed without a network response',async()=>{
 const f=await fixture();try{
  await f.db.configureEntities('app',['notes']);let started;const entered=new Promise(r=>started=r);
  f.db.startSync({push:()=>[],pull:()=>{started();return new Promise(()=>{});}},null,{pullIntervalMs:5});await entered;
  f.db.stopSync();let replacement=0;f.db.startSync({push:()=>[],pull:()=>{replacement++;return {checkpoint:'new',complete:true,changes:[]};}},null,{pullIntervalMs:5});
  await until(()=>replacement>0);f.db.stopSync();await f.db.reset();assert.equal((await f.db.syncProgress()).ready,false);
 }finally{await f.close();}
});
test('APP05 application reconcile delegates an idempotent replay to the core',async()=>{
 const f=await fixture();try{
  const receipt=await f.db.command(defineCommand('empty-v1',{args:{},handler:()=>null})).executeWithReceipt({});await f.db.preparePush(1000,0);await f.db.receive([{idempotencyKey:receipt.key,status:'applied',result:null}]);
  const plan={writes:[],mappings:{}};await f.db.reconcile(receipt.key,plan);await f.db.reconcile(receipt.key,plan);
 }finally{await f.close();}
});
test('APP04 returned handler data is captured before the durable finish await',async()=>{
 const f=await fixture();try{
  const value={nested:{text:'captured'}};const call=f.db.raw.call.bind(f.db.raw);
  f.db.raw.call=async(op,...args)=>{if(op==='finish')value.nested.text='changed outside';return call(op,...args);};
  const result=await f.db.command(defineCommand('capture-v1',{args:{},handler:()=>value})).execute({});assert.equal(result.nested.text,'captured');
 }finally{await f.close();}
});
test('APP08 stopped background push cannot apply a late server outcome',async()=>{
 const facts=[];const f=await fixture({onBackgroundError:x=>facts.push(x)});try{
  const receipt=await f.db.command(defineCommand('late-v1',{args:{},handler:()=>null})).executeWithReceipt({});let entered,respond;const waiting=new Promise(r=>entered=r);
  f.db.startSync({push:()=>{entered();return new Promise(r=>respond=r);}},()=>({writes:[],mappings:{}}));await waiting;f.db.stopSync();respond([{idempotencyKey:receipt.key,status:'applied',result:null}]);
  await new Promise(r=>setTimeout(r,10));assert.equal((await f.db.queueStatus()).commands[0].state,'sent');assert.equal(facts.length,0);
 }finally{await f.close();}
});
test('APP08 stopping a never-resolving push permits a fresh background sync',async()=>{
 let now=1000;const f=await fixture({clock:()=>new Date(now)});try{
  await f.db.command(defineCommand('restart-v1',{args:{},handler:()=>null})).execute({});let entered;const waiting=new Promise(r=>entered=r);
  f.db.startSync({push:()=>{entered();return new Promise(()=>{});}});await waiting;f.db.stopSync();let fresh=0;
  // Advance the captured test clock beyond the retained retry deadline; Sent redelivery itself is a no-op.
  now=100000;
  f.db.startSync({push:envelopes=>{fresh++;return envelopes.map(e=>({idempotencyKey:e.idempotencyKey,status:'applied',result:null}));}},()=>({writes:[],mappings:{}}));
  await until(()=>fresh===1);f.db.stopSync();
 }finally{await f.close();}
});
test('APP08 a background transport failure retains and retries the frozen request',async()=>{
 const facts=[];const f=await fixture({clock:Date.now,options:{initial_delay_ms:'1',max_delay_ms:'1',multiplier:'2'},onBackgroundError:x=>facts.push(x)});try{
  await f.db.command(defineCommand('retry-v1',{args:{},handler:()=>null})).execute({});let calls=0,first;
  f.db.startSync({push:envelopes=>{calls++;if(calls===1){first=envelopes;throw Object.assign(Error(),{kind:'BackendFailure'});}assert.deepEqual(envelopes,first);return envelopes.map(e=>({idempotencyKey:e.idempotencyKey,status:'applied',result:null}));}},()=>({writes:[],mappings:{}}));
  await until(()=>calls>=2);f.db.stopSync();assert.ok(facts.some(f=>f.source==='sync'&&f.kind==='BackendFailure'));
 }finally{await f.close();}
});
test('APP08 a conflicting background record does not starve later recovery or repeat its report',async()=>{
 let n=0;const facts=[];const f=await fixture({idGenerator:()=>`seed-${++n}`,onBackgroundError:x=>facts.push(x)});try{
  const a=await f.db.table('notes').insert({body:'a'}),b=await f.db.table('notes').insert({body:'b'});
  const edit=f.db.command(defineCommand('edit-v1',{args:{id:'string'},handler:(db,args)=>db.table('notes').findById(args.id).update({body:'optimistic'})}));
  const first=await edit.executeWithReceipt({id:a.id});const second=await edit.executeWithReceipt({id:b.id});await f.db.table('notes').findById(a.id).update({body:'later edit'});
  f.db.startSync({push:envelopes=>envelopes.map(e=>({idempotencyKey:e.idempotencyKey,status:e.idempotencyKey===first.key?'corrected':'applied',result:null}))},pending=>({mappings:{},writes:pending.key===first.key?[{op:'put',table:'notes',id:a.id,fields:{body:'server'}}]:[]}));
  await until(async()=>{const pending=await f.db.pendingReconciliations();return pending.length===1&&pending[0].key===first.key;});
  await f.db.table('notes').insert({body:'wake'});await new Promise(r=>setTimeout(r,15));
  assert.equal((await f.db.pendingReconciliations()).some(p=>p.key===second.key),false);assert.equal(facts.filter(x=>x.kind==='RollbackConflict').length,1);
 }finally{await f.close();}
});
test('APP08 close awaits the cancelled push before disposing its owner',async()=>{
 const f=await fixture();let release;try{
  let entered;const ready=new Promise(r=>entered=r),held=new Promise(r=>release=r);const prepare=f.db.raw.preparePush.bind(f.db.raw);
  f.db.raw.preparePush=async()=>{entered();await held;return prepare();};f.db.startSync({push:()=>[]});await ready;
  let closed=false;const closing=f.db.close().then(()=>{closed=true;});await new Promise(r=>setTimeout(r,5));assert.equal(closed,false);release();await closing;
 }finally{release?.();await f.close();}
});
