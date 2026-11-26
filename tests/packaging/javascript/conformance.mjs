// TC-0036: real SDK/core assertions shared by Node-memory and browser consumers.
import {j} from '../../../packages/client/src/index.js';
import {openMemoryDatabase,createFakeTransport} from '../../../packages/testing/src/index.js';
import {schemas} from '../../../packages/client/test/schemas.mjs';
const sorted=value=>Array.isArray(value)?value.map(sorted):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(key=>[key,sorted(value[key])])):value;
const equal=(actual,expected,label)=>{if(JSON.stringify(sorted(actual))!==JSON.stringify(sorted(expected)))throw Error(label);};
const truth=(value,label)=>{if(!value)throw Error(label);};
const waitFor=async(predicate,label)=>{const end=Date.now()+15000;while(!predicate()){if(Date.now()>=end)throw Error(label);await new Promise(resolve=>setTimeout(resolve,1));}};
async function rejects(work,kind){try{await work();}catch(error){equal(error.kind,kind,'static rejection expected '+kind+' received '+error.kind);return;}throw Error('missing rejection '+kind);}
const col=(kind,optional=false)=>({kind,optional});
const namespace='7130'.repeat(8);
export async function runConformance(runtime,open=options=>openMemoryDatabase(options)){
 const reports=[];
 async function run(name,work){try{await work();reports.push({name,ok:true});}catch(error){reports.push({name,ok:false,error:error.kind??error.message});}}
 const options=(schema,extra={})=>({runtime,schema,namespace,clock:()=>1000,jitter:()=>0,context:()=>j.object({tenant:j.string('consumer')}),...extra});
 await run('G01/G03/G04 unrelated A schema all typed cells and three relation kinds',async()=>{
  const db=await open(options(schemas[0],{idGenerator:(()=>{let n=0;return()=>`fixture-${++n}`;})()}));
  try{
   const author=await db.insert('authors',{name:j.string('東京\0🌿')});const tag=await db.insert('tags',{label:j.string('tag')});
   const first=await db.insert('articles',{title:j.string('one'),score:j.integer('9223372036854775807'),ratio:j.float('42.0'),active:j.boolean(true),at:j.timestamp('-9223372036854775808'),note:j.null(),author_id:j.reference(author.id)});
   const second=await db.insert('articles',{title:j.string('two'),score:j.integer('-9223372036854775808'),ratio:j.float('1.25'),active:j.boolean(false),at:j.timestamp('9223372036854775807'),author_id:j.reference(author.id)});
   equal(first.id,'fixture-3','deterministic ordinary ID');equal(first.fields.ratio.kind,'float','integral float kind');equal(first.fields.score,j.integer('9223372036854775807'),'i64max');equal(first.fields.at,j.timestamp('-9223372036854775808'),'timestampmin');for(const value of ['-0.0','1.7976931348623157e308','2.2250738585072014e-308','5e-324']){const updated=await db.replace('articles',second.id,{...second.fields,ratio:j.float(value)});equal(updated.fields.ratio.kind,'float','finite float kind');equal(Number(updated.fields.ratio.value),Number(value),'finite float extrema');}
   equal((await db.related('articles',first.id,'author').fetch()).map(x=>x.id),[author.id],'ToOne');
   await db.link('articles',first.id,'tags',tag.id);equal((await db.related('articles',first.id,'tags').fetch()).map(x=>x.id),[tag.id],'ManyToMany');await db.unlink('articles',first.id,'tags',tag.id);equal(await db.related('articles',first.id,'tags').fetch(),[],'unlink');
   equal((await db.query('articles').where('score','Ge',j.integer('0')).fetch()).map(x=>x.id),[first.id],'Ge');
   equal((await db.query('articles').where('score','Lt',j.integer('0')).fetch()).map(x=>x.id),[second.id],'Lt');
   equal((await db.query('articles').where('title','Ne',j.string('one')).fetch()).map(x=>x.id),[second.id],'Ne');
   equal((await db.query('articles').whereIn('id',[]).fetch()),[],'emptyIn');
   equal((await db.query('articles').whereIn('id',[j.string(second.id),j.string(first.id)]).orderBy('score','Desc').limit(1).fetch()).map(x=>x.id),[first.id],'In/order/limit');
   await rejects(()=>db.query('tags',{table:'articles',id:first.id,relation:'author'}).fetch(),'InvalidQuery');
   await rejects(()=>db.batch([{op:'insert',table:'authors',fields:{name:j.string('not committed')}},{op:'insert',table:'authors',fields:{name:j.integer('1')}}]),'InvalidRow');equal((await db.query('authors').fetch()).length,1,'atomic bad batch');
  }finally{await db.close();}
 });
 await run('G01 unrelated B HasMany uses the same caller-defined facade',async()=>{
  const db=await open(options(schemas[1]));try{const sensor=await db.insert('sensors',{label:j.string('meter')});const reading=await db.insert('readings',{value:j.float('1.25'),at:j.timestamp('1000'),sensor_id:j.reference(sensor.id)});equal(await db.related('sensors',sensor.id,'readings').fetch(),[reading],'HasMany');}finally{await db.close();}
 });
 await run('G04 admitted In operands cross4096 and32766 through the existing core planner',async()=>{
  const db=await open(options(schemas[1]));try{const row=await db.insert('sensors',{label:j.string('match')});for(const count of [4095,4096,33000]){const values=Array.from({length:count},()=>j.string(''));values.push(j.string('match'));equal((await db.query('sensors').whereIn('label',values).fetch()).map(value=>value.id),[row.id],'large admitted In '+count);}}finally{await db.close();}
 });
 await run('G11 keyed batch fitting full images and latest same-ID transitions stay exact',async()=>{
  const db=await open(options(schemas[1]));
  try{
   const before=Array.from({length:128},(_,i)=>({id:'budget:'+String(i).padStart(3,'0'),fields:{label:j.string('before-'+i+'-'+ 'x'.repeat(2000))}}));
   const inserted=await db.batch(before.map(row=>({op:'insert',table:'sensors',proposed_id:row.id,fields:row.fields})));equal(inserted.map(change=>change.after),before,'exact keyed seed');
   const after=before.map((row,i)=>({id:row.id,fields:{label:j.string('after-'+i+'-'+ 'y'.repeat(2000))}}));
   const changes=await db.batch(after.map(row=>({op:'replace',table:'sensors',id:row.id,fields:row.fields})));equal(changes,before.map((row,i)=>({table:'sensors',id:row.id,before:row,after:after[i]})),'128 fitting exact before-after images');
   equal(await db.query('sensors').orderBy('id','Asc').fetch(),after,'all128 committed exact fields');
   const first=after[0],second={id:first.id,fields:{label:j.string('staged one')}},third={id:first.id,fields:{label:j.string('staged two')}};
   const staged=await db.batch([{op:'replace',table:'sensors',id:first.id,fields:second.fields},{op:'replace',table:'sensors',id:first.id,fields:third.fields},{op:'remove',table:'sensors',id:first.id}]);
   equal(staged,[{table:'sensors',id:first.id,before:first,after:second},{table:'sensors',id:first.id,before:second,after:third},{table:'sensors',id:first.id,before:third,after:null}],'latest same-ID staged image through removal');
   equal(await db.query('sensors').where('id','Eq',j.string(first.id)).fetch(),[],'removed exact key');equal((await db.query('sensors').fetch()).length,127,'other127 records preserved');
  }finally{await db.close();}
 });
 await run('G11 byte-exact terminal prefix progresses once without losing either event',async()=>{
  const schema={version:'1',tables:{documents:{columns:{body:col('String')},relations:{}}}},db=await open(options(schema));
  try{
   // Raw public runtime operations deliberately keep managed SDK drains absent.
   const owner=db.owner,query={table:'documents',predicates:[],order:null,limit:null,source:null};
   const subscription=await runtime.call('observe',{query,mode:'membership',columns:[]},owner);
   const event=rows=>({kind:'data',subscription,value:{kind:'rows',rows}}),row=body=>({id:'s',fields:{body:j.string(body)}});
   const complete=(body,has_more)=>({abi:'1',ok:true,value:{events:[event([]),event([row(body)])],has_more}});
   const body='x'.repeat(1048576-new TextEncoder().encode(JSON.stringify(complete('',true))).length);
   equal(new TextEncoder().encode(JSON.stringify(complete(body,true))).length,1048576,'true prefix exact1MiB');equal(new TextEncoder().encode(JSON.stringify(complete(body,false))).length,1048577,'terminal false flag adds one byte');
   equal(await runtime.call('insert',{table:'documents',fields:row(body).fields,proposed_id:'s'},owner),row(body),'near-bound insert exact result');
   equal(await runtime.call('drain',{now_ms:'0'},owner),{events:[event([])],has_more:true},'first smaller owned prefix');
   equal(await runtime.call('drain',{now_ms:'0'},owner),{events:[event([row(body)])],has_more:false},'second terminal prefix retains exact body');
   equal(await runtime.call('drain',{now_ms:'0'},owner),{events:[],has_more:false},'both events delivered exactly once');truth(await runtime.call('unsubscribe',{subscription},owner),'raw observer cleanup');
  }finally{await db.close();}
 });
 await run('G03 core codecs preserve extrema Float Unicode and reject duplicates',async()=>{
  const value=await runtime.decodeJson('{"min":-9223372036854775808,"max":9223372036854775807,"float":42.0,"label":"東京\\u0000🌿"}');equal(value.value.min,j.integer('-9223372036854775808'),'businessi64');equal(value.value.float.kind,'float','businessFloat');equal(await runtime.decodeJson(await runtime.encodeJson(value)),value,'codec roundtrip');await rejects(()=>runtime.decodeJson('{"x":1,"x":2}'),'InvalidPayload');
 });
 await run('G05 membership columns count delegate shared observation groups',async()=>{
  const db=await open(options(schemas[1]));try{const events=[];const q=db.query('sensors');const a=await q.observe(event=>events.push(event),'membership');const b=await q.observe(event=>events.push(event),'columns',['label']);const c=await q.observe(event=>events.push(event),'count');await waitFor(()=>events.length>=3,'managed three initial events');equal(events.length,3,'three initial events');events.length=0;const row=await db.insert('sensors',{label:j.string('first')});await waitFor(()=>events.some(x=>x.subscription===a&&x.value.kind==='rows'&&x.value.rows[0].id===row.id),'membership insertion');events.length=0;await db.replace('sensors',row.id,{label:j.string('changed')});await waitFor(()=>events.some(x=>x.subscription===b&&x.value.rows[0]?.fields.label.value==='changed'),'column edit');truth(!events.some(x=>x.subscription===a),'membership stable');await db.unsubscribe(a);await db.unsubscribe(b);await db.unsubscribe(c);await waitFor(()=>!db.draining,'managed drain completes');await db.drain('18446744073709551615');}finally{await db.close();}
 });
 await run('G05 real managed count initial delivery and unpostponed deadline need no manual drain',async()=>{
  let now=0,next=0;const timers=new Map(),caps={set(work,delay){const id=++next;timers.set(id,{work,due:now+delay});return id;},clear(id){timers.delete(id);}};
  const advance=async value=>{now=value;for(const [id,timer]of [...timers])if(timer.due<=now){timers.delete(id);timer.work();}await new Promise(resolve=>setTimeout(resolve,0));};
  const db=await open(options(schemas[1],{monotonic:()=>now,timers:caps}));const counts=[];
  try{const subscription=await db.query('sensors').observe(event=>{if(event.value.kind==='count')counts.push(event.value.count);},'count');equal(counts,[],'registration schedules later turn');await advance(0);await waitFor(()=>counts.length>=1,'automatic initial count delivery');equal(counts,['0'],'automatic initial count');await advance(1);const row=await db.insert('sensors',{label:j.string('one')});equal([...timers.values()].filter(timer=>timer.due>now).map(timer=>timer.due),[17],'single deadline');await advance(6);await db.replace('sensors',row.id,{label:j.string('two')});equal([...timers.values()].filter(timer=>timer.due>now).map(timer=>timer.due),[17],'deadline not postponed');await advance(17);await waitFor(()=>counts.length>=2,'pending count delivery');equal(counts,['0','1'],'core pending count flushed');await db.unsubscribe(subscription);}finally{await db.close();}equal(timers.size,0,'close cancels managed timers');
 });
 await run('G06/G20/TC16 awaited handler immutable context optimistic rows and queue offline',async()=>{
  const db=await open(options(schemas[1],{idGenerator:()=> 'local:test-fixed-id-001'}));try{const result=await db.execute({name:'add-sensor-v1',arguments:{},reference_paths:[]},j.object({}),async scope=>{let caught=false;try{scope.context.identity.value.tenant=j.string('changed');}catch{caught=true;}truth(caught,'immutable context');await Promise.resolve();const row=await scope.insert('sensors',{label:j.string('offline')});equal(row.id,'local:test-fixed-id-001','command deterministic prefixed ID');return j.integer('9223372036854775807');});equal(result.result,j.integer('9223372036854775807'),'handler exact result');equal((await db.query('sensors').fetch()).length,1,'optimistic local row');const status=await db.queueStatus();equal(status.pending,'1','queued offline');equal(status.commands[0].attempts,'0','not automatically sent');if(!open.protected)equal(await db.protection(),{encrypted:false,cipher:'none',version:'not_applicable',backup:'not_applicable'},'memory protection');else{const protection=await db.protection();truth(protection.encrypted&&protection.cipher==='SQLCipher'&&protection.backup==='unsupported','actual protected capability');}}finally{await db.close();}
 });
 await run('G19/G21/TC16 real fake transport correlates applied rejected and dropped ACK retries',async()=>{
  const db=await open(options(schemas[1]));const definition={name:'fake-v1',arguments:{},reference_paths:[]};
  try{
   const first=await db.execute(definition,j.object({}),async()=>j.null());
   const fake=createFakeTransport({runtime,handlers:{'fake-v1':()=>({status:'applied',result:j.object({large:j.integer('9223372036854775807'),float:j.float('42.0')})})}});
   const batch=await db.preparePush(1000,0);equal(batch.envelopes.length,1,'prepared production envelope');
   await db.receiveWire(await fake.push('['+batch.envelopes.join(',')+']'));equal(fake.pushes[0][0].value.idempotencyKey,j.string(first.key),'automatic correlation');
   const pending=await db.pendingReconciliations();equal(pending[0].outcome.result.value.large,j.integer('9223372036854775807'),'fake result exact core i64');equal(pending[0].outcome.result.value.float.kind,'float','fake result integral float');await db.reconcile(first.key,{writes:[],mappings:{}});
   const second=await db.execute(definition,j.object({}),async()=>j.null());const rejecting=createFakeTransport({runtime,handlers:{'fake-v1':()=>({status:'rejected',reason:'business refusal'})}});const rejectedBatch=await db.preparePush(1000,0);await db.receiveWire(await rejecting.push('['+rejectedBatch.envelopes.join(',')+']'));equal((await db.queueStatus()).commands.find(entry=>entry.key===second.key).state,'rejected','business rejection');
   const third=await db.execute(definition,j.object({}),async()=>j.null());const sent=await db.preparePush(1000,0);let dropped=0;try{await(async()=>{dropped++;throw Error('synthetic offline');})('['+sent.envelopes.join(',')+']');}catch{}equal(dropped,1,'transport rejection');const status=(await db.queueStatus()).commands.find(entry=>entry.key===third.key);equal(status.state,'sent','dropped ACK leaves Sent');equal(status.attempts,'1','persisted attempt');equal(status.next_due_ms,'2000','core due time');equal((await db.preparePush(1999,0)).envelopes,[],'no early retry');equal((await db.preparePush(2000,0)).envelopes,sent.envelopes,'exact original frozen retry bytes');
  }finally{await db.close();}
 });
 await run('G12/G13/G14 passive async hooks preserve staged sequence authority and stale refetch',async()=>{
  const db=await open(options(schemas[1]));
  const change=(op,label,time)=>({table:'sensors',record_id:'server:merge',op,changes:{label:j.string(label)},removed_fields:[],timestamp_ms:String(time),field_timestamps:{label:String(time)}});
  const page={checkpoint:'merged',complete:true,changes:[change('create','first',1),change('update','second',2)]};
  try{
   await db.configureEntities('merge-scope',['sensors']);const observed=[];const subscription=await db.query('sensors').observe(event=>observed.push(event),'columns',['label']);await waitFor(()=>observed.length>=1,'managed pre-merge initial event');observed.length=0;let hooks=0;
   db.setMergeHooks({sensors:async input=>{hooks++;equal(input.schema.tables.readings.columns.sensor_id.kind,'Reference','normalized hook schema');equal(await db.query('sensors').fetch(),[],'no staged row publishes under hook');equal(observed,[],'no precommit event');if(hooks===1)equal(input.current,null,'first current null');else equal(input.current.fields.label,j.string('custom-first'),'second current uses first staged proposal');let immutable=false;try{input.default.metadata.fields.label.timestamp_ms='999';}catch{immutable=true;}truth(immutable,'immutable default metadata');await Promise.resolve();return {id:input.default.row.id,fields:{label:j.string('custom-'+input.incoming.changes.label.value)}};}});
   await db.pullOnce(async since=>{equal(since,null,'bootstrap checkpoint');return JSON.stringify(page);});equal(hooks,2,'two sequential offers');equal((await db.query('sensors').fetch())[0].fields.label,j.string('custom-second'),'validated staged custom final');equal((await db.syncProgress()).pages,'1','one atomic page');
   db.setMergeHooks({sensors:input=>({...input.default.row,id:'forged'})});await rejects(()=>db.pullOnce(async()=>JSON.stringify({checkpoint:'bad',complete:true,changes:[change('update','bad',3)]})),'InvalidCommand');equal((await db.query('sensors').fetch())[0].fields.label,j.string('custom-second'),'invalid proposal preserved row');equal((await db.syncProgress()).pages,'1','invalid proposal preserved checkpoint');
   let offers=0,network=0;db.setMergeHooks({sensors:async input=>{if(++offers===1)await db.insert('readings',{value:j.float('1.25'),at:j.timestamp('3'),sensor_id:j.reference('server:merge')});return input.default.row;}});
   await db.pullOnce(async since=>{equal(since,'merged','stale unchanged checkpoint refetch');network++;return JSON.stringify({checkpoint:'after-stale',complete:true,changes:[change('update','fresh',3)]});});equal(network,2,'stale page fetched anew');equal(offers,2,'hook uses a fresh session');equal((await db.query('sensors').fetch())[0].fields.label,j.string('fresh'),'fresh commit');await db.unsubscribe(subscription);
  }finally{await db.close();}
 });
 await run('G17/G18 mixed command entity correlation and checkpoint progress use shared core',async()=>{
  let clock=100;const db=await open(options(schemas[1],{clock:()=>clock}));
  try{
   await db.configureEntities('mixed-scope',['sensors']);await db.execute({name:'ping-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.null());
   const created=(await db.entityBatch([{op:'insert',table:'sensors',fields:{label:j.string('created')}}]))[0].after;truth(created.id.startsWith('entity:'),'strict entity allocator');clock=101;await db.patch('sensors',created.id,{label:j.string('patched')});
   const fake=createFakeTransport({runtime,handlers:{'ping-v1':()=>({status:'applied',result:j.null()})},entityHandlers:{sensors:envelope=>({status:'applied',result:j.object({id:envelope.value.recordId,created:j.boolean(envelope.value.op.value==='create')})})}});
   const first=await db.preparePush(1000,0);equal(first.envelopes.length,2,'command and producer sent but dependent waits');let response=await runtime.decodeJson(await fake.push('['+first.envelopes.join(',')+']'));response.value.reverse();await db.receiveWire(await runtime.encodeJson(response));
   for(const pending of await db.pendingReconciliations()){const result=pending.outcome.result;await db.reconcile(pending.key,{writes:[],mappings:result?.kind==='object'&&result.value.created?.value?{[result.value.id.value]:result.value.id.value}:{}});}
   const second=await db.preparePush(1000,0);equal(second.envelopes.length,1,'dependent update released');await db.receiveWire(await fake.push('['+second.envelopes.join(',')+']'));for(const pending of await db.pendingReconciliations())await db.reconcile(pending.key,{writes:[],mappings:{}});
   const status=await db.queueStatus();equal(status.live,'0','all reconciled');equal(status.commands.length,3,'mixed history');truth(status.commands.every(entry=>entry.attempts==='1'&&entry.next_due_ms===null),'persisted payload-free status');
   await db.pullOnce(async()=>JSON.stringify({checkpoint:'mixed-ready',complete:true,changes:[{table:'sensors',record_id:created.id,op:'update',changes:{label:j.string('backend')},removed_fields:[],timestamp_ms:'200',field_timestamps:{label:'200'}}]}));equal((await db.query('sensors').fetch())[0].fields.label,j.string('backend'),'authoritative pull');equal(await db.syncProgress(),{phase:'ready',ready:true,pages:'1',changes:'1',tombstones:'0'},'progress exact');equal(Object.keys(await db.diagnostics()).sort(),['schema_version','pending','sending','awaiting_outcome','live','blocked','ready','pages','changes','tombstones'].sort(),'diagnostics ten fields');
  }finally{await db.close();}
 });
 await run('A01/A07/D06 atomic aliased writes and durable resolution cross the packaged facade',async()=>{
  const schema={...schemas[1],tables:{...schemas[1].tables,markers:{columns:{label:col('String')},relations:{}}}};
  const db=await open(options(schema));try{
   await db.configureEntitiesDurable('alias-consumer',['sensors','readings']);
   const created=await db.entityBatchAliased([{op:'insert',table:'sensors',alias:'parent',fields:{label:j.string('parent')}},{op:'insert',table:'readings',alias:'child',fields:{value:j.float('1.25'),at:j.timestamp('1000')},references:{sensor_id:'parent'}}]);
   equal(created.changes.length,2,'one parent child batch');truth(created.aliases.parent.startsWith('entity:'),'real parent identity');
   equal((await db.query('readings').fetch())[0].fields.sensor_id,j.reference(created.aliases.parent),'only real reference stored');
   await rejects(()=>db.entityBatchAliased([{op:'insert',table:'markers',proposed_id:'marker',fields:{label:j.string('done')}},{op:'patch',table:'readings',id:'missing',fields:{value:j.float('9.0')}}]),'RowNotFound');
   equal(await db.query('markers').fetch(),[],'failed partial import leaves no marker');
   await db.entityBatchAliased([{op:'patch',table:'readings',id:created.aliases.child,fields:{value:j.float('2.0')}},{op:'insert',table:'markers',proposed_id:'marker',fields:{label:j.string('done')}}]);
   const child=(await db.query('readings').fetch())[0];equal(child.fields.at,j.timestamp('1000'),'patch preserves omitted field');equal(child.fields.value.kind,'float','patch retains supplied float kind');equal(Number(child.fields.value.value),2,'patch commits supplied field');
   const pushed=await db.preparePush(1000,0);equal(pushed.envelopes.length,1,'only parent ready');
   const parent=(await db.queueStatus()).commands.find(entry=>entry.state==='sent');truth(parent,'parent sending');
   await db.receive(parent.key,{status:'applied',result:j.null()});
   await db.reconcile(parent.key,{writes:[],mappings:{[created.aliases.parent]:'canonical-parent'}});
   await db.forget(parent.key);
   equal(await db.resolveEntityId('sensors',created.aliases.parent),'canonical-parent','resolution survives explicit forget');
   equal((await db.query('readings').fetch())[0].fields.sensor_id,j.reference('canonical-parent'),'child reference remapped');
  }finally{await db.close();}
 });
 if(!open.retainedUnsupported)await run('G02/G22 retained backend authored seven-step migration and failure preservation',async()=>{
  const store=await runtime.createMemoryStore();const v1={version:'1',tables:{sensors:{columns:{label:col('String'),old:col('String')},relations:{}},obsolete:{columns:{label:col('String')},relations:{}}}};
  const v2={version:'2',tables:{sensors:{columns:{caption:col('String'),optional:col('String',true),added:col('String')},relations:{}},auxiliary:{columns:{label:col('String')},relations:{}}}};
  const migration={from_version:'1',to:v2,steps:[{op:'AddTable',table:'auxiliary',definition:v2.tables.auxiliary},{op:'DropTable',table:'obsolete'},{op:'AddColumn',table:'sensors',name:'optional',column:col('String',true),default:{kind:'some',value:j.null()}},{op:'AddColumn',table:'sensors',name:'added',column:col('String'),default:{kind:'some',value:j.string('initial')}},{op:'DropColumn',table:'sensors',name:'old'},{op:'RenameColumn',table:'sensors',from:'label',to:'caption'},{op:'Backfill',table:'sensors',column:'added',value:j.string('filled')},{op:'SetRelations',table:'sensors',relations:{}}]};
  let db;try{db=await openMemoryDatabase(options(v1,{store,idGenerator:()=> 'retained'}));await db.insert('sensors',{label:j.string('before'),old:j.string('drop')});await rejects(()=>runtime.dropMemoryStore(store),'Busy');await db.close();db=null;
   await rejects(()=>openMemoryDatabase(options(v2,{store,migrations:[{...migration,steps:[]}]})),'MigrationFailure');
   db=await openMemoryDatabase(options(v1,{store}));equal((await db.query('sensors').fetch())[0].fields.label,j.string('before'),'failed migration preserves original');await db.close();db=null;
   db=await openMemoryDatabase(options(v2,{store,migrations:[migration]}));equal(await db.query('sensors').fetch(),[{id:'retained',fields:{caption:j.string('before'),optional:j.null(),added:j.string('filled')}}],'all seven migrated endpoint');await db.close();db=null;
  }finally{await db?.close();await runtime.dropMemoryStore(store);}
 });
 reports.push(...await runManagedSyncObservations(runtime,open));
 reports.push(...await runExternalReceiptWake(runtime,open));
 return reports;
}

export async function runActualHttp(runtime,open){
 const reports=[];const db=await open({runtime,schema:schemas[1],namespace:'1718'.repeat(8),context:()=>j.null(),clock:()=>1000,jitter:()=>0});
 try{
  await db.configureEntities('http-scope',['sensors']);const args=j.object({min:j.integer('-9223372036854775808'),max:j.integer('9223372036854775807'),float:j.float('42.0')});
  const command=await db.execute({name:'http-v1',arguments:{min:{kind:'Integer',optional:false},max:{kind:'Integer',optional:false},float:{kind:'Float',optional:false}},reference_paths:[]},args,async()=>j.null());const row=(await db.entityBatch([{op:'insert',table:'sensors',fields:{label:j.string('HTTP 東京\0🌿')}}]))[0].after;
  const batch=await db.preparePush(1000,0);equal(batch.envelopes.length,2,'actual mixed HTTP batch');const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json'},body:'['+batch.envelopes.join(',')+']'});truth(response.ok,'actual HTTP200');const receipts=await db.receiveWire(await response.text());truth(receipts.every(receipt=>receipt.ok),'actual mixed correlations');
  for(const pending of await db.pendingReconciliations()){if(pending.key===command.key){equal(pending.outcome.result.value.min,j.integer('-9223372036854775808'),'HTTP exact min');equal(pending.outcome.result.value.max,j.integer('9223372036854775807'),'HTTP exact max');equal(pending.outcome.result.value.float.kind,'float','HTTP integral Float');await db.reconcile(pending.key,{writes:[],mappings:{}});}else await db.reconcile(pending.key,{writes:[],mappings:{[row.id]:row.id}});}
  equal((await db.queueStatus()).live,'0','actual HTTP reconciled');reports.push({name:'G17/G21 actual HTTP mixed reversed outcomes preserve i64 Float and tuples',ok:true});
 }catch(error){reports.push({name:'G17/G21 actual HTTP mixed reversed outcomes preserve i64 Float and tuples',ok:false,error:error.kind??error.message});}finally{await db.close();}return reports;
}


// DEF-0012: await real callbacks, never advance delivery with an application drain.
export async function runManagedSyncObservations(runtime,open=options=>openMemoryDatabase(options)){
 const reports=[];const options={runtime,schema:schemas[1],namespace,clock:()=>1000,jitter:()=>0,context:()=>j.null()};
 for(const stage of ['prepare','receive']){
  let db,token;
  try{
   db=await open(options);const command=await db.execute({name:'managed-sync-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.null());
   if(stage==='receive')await db.preparePush(1000,0);
   const events=[];let waiting;
   const awaited=state=>{if(events.some(value=>value.state===state))return Promise.resolve();return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{waiting=null;reject(Error('managed '+state+' callback missing without another operation'));},3000);waiting={state,resolve:()=>{clearTimeout(timer);waiting=null;resolve();}};});};
   token=await db.observeOutcome(command.key,event=>{events.push(event.value);if(event.value.state===waiting?.state)waiting.resolve();});
   await awaited(stage==='prepare'?'queued':'sent');
   if(stage==='prepare'){await db.preparePush(1000,0);await awaited('sent');equal(events.map(value=>value.state),['queued','sent'],'prepare transition delivered exactly once');}
   else{const body=await runtime.encodeJson(j.array([j.object({idempotencyKey:j.string(command.key),status:j.string('applied'),result:j.null()})]));await db.receiveWire(body);await awaited('acknowledged');equal(events.map(value=>value.state),['sent','acknowledged'],'receipt transition delivered exactly once');}
   reports.push({name:'G05 managed '+stage+' transition delivered without another operation',ok:true});
  }catch(error){reports.push({name:'G05 managed '+stage+' transition delivered without another operation',ok:false,error:error.kind??error.message});}
  finally{if(token)await db.unsubscribe(token);await db?.close();}
 }
 return reports;
}


// DEF-0015 / G19: a public external receipt must wake the settled SDK driver.
export async function runExternalReceiptWake(runtime,open=options=>openMemoryDatabase(options)){
 let db,token,watchdog;const reports=[];const name='G19 external wire receipt wakes sleeping local reconciliation before retry deadline';
 try{
  db=await open({runtime,schema:schemas[1],namespace,clock:()=>1000,jitter:()=>0,context:()=>j.null(),options:{initial_delay_ms:'60000',max_delay_ms:'60000'}});
  const command=await db.execute({name:'external-receipt-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.null());
  let applied;const completed=new Promise(resolve=>{applied=resolve;});let mapped=0,pushes=0,prepares=0;
  token=await db.observeOutcome(command.key,event=>{if(event.value.state==='applied')applied();});
  const prepare=db.preparePush.bind(db);db.preparePush=(...args)=>{prepares++;return prepare(...args);};
  const body=await runtime.encodeJson(j.array([j.object({idempotencyKey:j.string(command.key),status:j.string('applied'),result:j.null()})]));
  db.startSync(async()=>{pushes++;return'[]';},async pending=>{equal(pending.key,command.key,'own pending receipt');mapped++;return{writes:[],mappings:{}};});
  await waitFor(()=>prepares===2&&!db.syncing,'driver settled before external receipt');
  const before=await db.queueStatus();equal(before.commands[0].attempts,'1','one original send');equal(before.commands[0].next_due_ms,'61000','future retry deadline');equal(mapped,0,'no outcome to map before receipt');
  await db.receiveWire(body);
  await Promise.race([completed,new Promise((_,reject)=>{watchdog=setTimeout(()=>reject(Error('external receipt did not wake local reconcile before future retry')),3000);})]);clearTimeout(watchdog);
  equal(pushes,1,'external outcome never resends transport');equal(mapped,1,'one mapper invocation');const after=await db.queueStatus();equal(after.live,'0','local reconciliation committed');equal(after.commands[0].state,'applied','exact final outcome');equal(after.commands[0].attempts,'1','no early retry');
  await new Promise(resolve=>setTimeout(resolve,30));truth(!db.syncing&&!db.syncWakePending,'receipt driver settles without self-wake loop');truth(prepares>=3&&prepares<=4,'bounded original and receipt due checks');reports.push({name,ok:true});
 }catch(error){reports.push({name,ok:false,error:error.kind??error.message});}
 finally{clearTimeout(watchdog);if(token)await db.unsubscribe(token);await db?.close();}
 return reports;
}
