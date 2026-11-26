// API-0011 / API-0012 review regressions: entity links, terminal and recoverable
// observation errors, disposal after close and owner-epoch races.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import init,{Facade} from '../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {Database,createRuntime,j} from '../src/index.js';
import {ApplicationDatabase,openApplicationDatabase} from '../src/application.js';
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});

const col=(kind,optional=false)=>({kind,optional});
const one=(table,foreign_key)=>({kind:'ToOne',table,foreign_key,optional:false});
const schema={version:'1',tables:{
 articles:{columns:{title:col('String')},relations:{tags:{kind:'ManyToMany',table:'tags',through:'article_tags',source_relation:'article',target_relation:'tag'}}},
 tags:{columns:{label:col('String')},relations:{}},
 article_tags:{columns:{},relations:{article:one('articles','article_id'),tag:one('tags','tag_id')}},
 notes:{columns:{body:col('String')},relations:{}}
}};
async function application(extra={}){
 const facade=new Facade();const runtime=createRuntime({open:(s,k)=>facade.open(s,k),call:s=>facade.call(s)},{namespace:()=>new Uint8Array(16).fill(7)});
 let next=0;const db=await openApplicationDatabase({runtime,schema,engine:'memory',clock:()=>new Date(1000),jitter:()=>0,idGenerator:()=>`local:app-${++next}`,...extra});
 return {db,async close(){await db.close();facade.free();}};
}
const deferred=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};};
const tick=()=>new Promise(resolve=>setImmediate(resolve));
const reply=value=>JSON.stringify({abi:'1',ok:true,value});

test('R2 application link and unlink use the entity path for a configured through table',async()=>{
 const f=await application({entities:{scope:'app',tables:['articles','tags','article_tags']}});try{
  const article=await f.db.table('articles').insert({title:'A'}),tag=await f.db.table('tags').insert({label:'T'});
  const join=await f.db.table('articles').link(article.id,'tags',tag.id);
  assert.equal(join.article_id,article.id);assert.equal(join.tag_id,tag.id);assert.match(join.id,/^entity:/);
  assert.deepEqual((await f.db.related('articles',article.id,'tags').fetch()).map(row=>row.id),[tag.id]);
  assert.ok((await f.db.queueStatus()).commands.some(entry=>entry.kind==='entity'&&entry.table==='article_tags'));
  assert.equal(await f.db.table('articles').unlink(article.id,'tags',tag.id),'1');
  assert.deepEqual(await f.db.related('articles',article.id,'tags').fetch(),[]);
 }finally{await f.close();}
});
test('R2 an unconfigured through table keeps ordinary links and the injected ID',async()=>{
 const f=await application({entities:{scope:'app',tables:['notes']}});try{
  const article=await f.db.table('articles').insert({title:'A'}),tag=await f.db.table('tags').insert({label:'T'});
  const join=await f.db.table('articles').link(article.id,'tags',tag.id);assert.equal(join.id,'local:app-3');
  assert.equal(await f.db.table('articles').unlink(article.id,'tags',tag.id),'1');
 }finally{await f.close();}
});

test('R3 a terminal error event is never delivered to a status, progress or outcome callback as data',async()=>{
 for(const [method,args,data] of [['observeStatus',[],{kind:'status',subscription:'7',value:{pending:'0'}}],['observeProgress',[],{kind:'progress',subscription:'7',value:{ready:true}}],['observeOutcome',['key'],{kind:'outcome',subscription:'7',value:{state:'sent'}}]]){
  let listener;const raw={[method]:async(...values)=>{listener=values.at(-1);return '7';},async unsubscribe(){return true;}};
  const seen=[],app=new ApplicationDatabase({},raw,{});const stop=app[method](...args,value=>{seen.push(value);});await stop.ready;
  listener(data);assert.deepEqual(seen,[data.value]);
  assert.doesNotThrow(()=>listener({kind:'error',subscription:'7',value:{kind:'LimitExceeded'}}));
  assert.deepEqual(seen,[data.value],method+' must not receive the terminal event as a value');
 }
});

test('R5 unsubscribe after close resolves and a failed registration has nothing to dispose',async()=>{
 const f=await application();let closed=false;try{
  const stop=f.db.query('notes').observe(()=>{});await stop.ready;
  await f.db.close();closed=true;
  assert.equal(await stop(),false);
  const late=f.db.query('notes').observe(()=>{});await assert.rejects(late.ready,{kind:'Closed'});
  assert.equal(await late(),undefined);
  const status=f.db.observeStatus(()=>{});await assert.rejects(status.ready,{kind:'Closed'});assert.equal(await status(),undefined);
 }finally{if(!closed)await f.db.close();}
});
test('R5 a live unsubscribe failure is still returned to the caller',async()=>{
 const raw={async observeStatus(){return '7';},async unsubscribe(){throw Object.assign(new Error('BackendFailure'),{kind:'BackendFailure'});}};
 const stop=new ApplicationDatabase({},raw,{}).observeStatus(()=>{});await stop.ready;
 await assert.rejects(stop(),{kind:'BackendFailure'});
});

test('R6 a recoverable data error without an error callback is reported and keeps the listener',async()=>{
 let listener;const facts=[],seen=[];
 const raw={async observeData(query,callback){listener=callback;return '7';},async unsubscribe(){return true;}};
 const app=new ApplicationDatabase({},raw,{onBackgroundError:fact=>facts.push(fact)});
 const stop=app.observeQuery({wire:{},tableName:'notes'},count=>{seen.push(count);},undefined,true);await stop.ready;
 assert.doesNotThrow(()=>listener({kind:'data',subscription:'7',value:{kind:'error',error:{kind:'BackendFailure'}}}));
 assert.deepEqual(facts,[{source:'observation',kind:'BackendFailure',token:null}]);
 listener({kind:'data',subscription:'7',value:{kind:'count',count:'2'}});assert.deepEqual(seen,[2]);
});

function host({hold=[],options={}}={}){
 const frames=[],gates=new Map(hold.map(op=>[op,deferred()])),entered=new Map(hold.map(op=>[op,deferred()]));
 const values={request:'5',begin:true,scope_args:j.null(),scope_context:{identity:j.null(),now_ms:'1'},finish:{key:'k',result:j.null()},drain:{events:[],has_more:false},observe:'9',unsubscribe:true};
 const call=async input=>{const frame=JSON.parse(input);frames.push(frame);if(gates.has(frame.op)){entered.get(frame.op).resolve();await gates.get(frame.op).promise;}return reply(values[frame.op]??null);};
 const runtime=createRuntime({open:()=>reply('1'),call},{namespace:()=>new Uint8Array(16)});
 const reports=[],db=new Database(runtime,{namespace:'00'.repeat(16),context:()=>j.null(),clock:()=>1,onBackgroundError:report=>reports.push(report),...options});
 return {db,frames,reports,entered,gates,open:()=>db.open({schema:{version:'1',tables:{}},engine:{kind:'memory',store:null}})};
}
const definition={name:'touch-v1',arguments:{},reference_paths:[]};
const notes={table:'notes',predicates:[],order:null,limit:null,source:null};

test('R7 a subscription registered across a reset is retired in core, not leaked',async()=>{
 const f=host({hold:['observe']});await f.open();try{
  const pending=f.db.observe(notes,()=>{}).then(value=>({value}),error=>({error}));await f.entered.get('observe').promise;
  await f.db.reset();f.gates.get('observe').resolve();
  assert.equal((await pending).error?.kind,'InvalidHandle');
  assert.deepEqual(f.frames.filter(frame=>frame.op==='unsubscribe').map(frame=>frame.args),[{subscription:'9'}]);
 }finally{await f.db.close();}
});

test('R8 a close racing a submitted finish reports no background observation failure',async()=>{
 const f=host({hold:['finish']});await f.open();
 const result=f.db.execute(definition,j.null(),()=>j.null());await f.entered.get('finish').promise;
 const closing=f.db.close();await tick();f.gates.get('finish').resolve();
 assert.deepEqual(await result,{key:'k',result:j.null()});await closing;
 assert.deepEqual(f.reports,[]);assert.deepEqual(f.db.callbackErrors,[]);
 assert.equal(f.frames.filter(frame=>frame.op==='drain').length,0);
});

test('R10 execute on a closed database rejects Closed like every other operation',async()=>{
 const f=host();
 await assert.rejects(f.db.execute(definition,j.null(),()=>j.null()),{kind:'Closed'});
 await f.open();await f.db.close();
 await assert.rejects(f.db.execute(definition,j.null(),()=>j.null()),{kind:'Closed'});
 await assert.rejects(f.db.fetch(notes),{kind:'Closed'});
});

const limit=()=>JSON.stringify({abi:'1',ok:false,error:{kind:'LimitExceeded'}});
function paged(replies){
 const frames=[];const runtime=createRuntime({open:()=>reply('1'),call:input=>{const frame=JSON.parse(input);frames.push(frame);return replies(frame,frames);}},{namespace:()=>new Uint8Array(16)});
 const db=new Database(runtime,{namespace:'00'.repeat(16)});
 return {db,frames,open:()=>db.open({schema:{version:'1',tables:{}},engine:{kind:'memory',store:null}})};
}
const entry=key=>({key,outcome:{status:'rejected',reason:'r'}});
test('R1b a pending list that exceeds one frame is read through every cursor page, in order',async()=>{
 const f=paged(frame=>frame.op==='pending_reconciliations'?limit():frame.op==='pending_reconciliations_page'?reply(frame.args.after===null?{entries:[entry('a'),entry('b')],has_more:true}:frame.args.after==='b'?{entries:[entry('c')],has_more:false}:null):reply(null));
 await f.open();try{
  assert.deepEqual(await f.db.pendingReconciliations(),[entry('a'),entry('b'),entry('c')]);
  assert.deepEqual(f.frames.map(frame=>[frame.op,frame.args]),[['pending_reconciliations',{}],['pending_reconciliations_page',{after:null}],['pending_reconciliations_page',{after:'b'}]]);
 }finally{await f.db.close();}
});
test('R1b a list that fits keeps the single unpaged call, and a retired cursor restarts the walk',async()=>{
 const fits=paged(frame=>reply(frame.op==='pending_reconciliations'?[entry('a')]:null));await fits.open();
 try{assert.deepEqual(await fits.db.pendingReconciliations(),[entry('a')]);assert.deepEqual(fits.frames.map(frame=>frame.op),['pending_reconciliations']);}finally{await fits.db.close();}
 let walks=0;const f=paged(frame=>{
  if(frame.op==='pending_reconciliations')return limit();
  if(frame.op!=='pending_reconciliations_page')return reply(null);
  if(frame.args.after===null){walks++;return reply(walks===1?{entries:[entry('a')],has_more:true}:{entries:[entry('b')],has_more:false});}
  return JSON.stringify({abi:'1',ok:false,error:{kind:'Protocol'}});
 });await f.open();
 try{assert.deepEqual(await f.db.pendingReconciliations(),[entry('b')]);assert.equal(walks,2);}finally{await f.db.close();}
});
test('R1b a runtime without the paged operation keeps the original LimitExceeded',async()=>{
 const f=paged(frame=>frame.op==='pending_reconciliations'?limit():frame.op==='pending_reconciliations_page'?JSON.stringify({abi:'1',ok:false,error:{kind:'InvalidRequest'}}):reply(null));await f.open();
 try{await assert.rejects(f.db.pendingReconciliations(),{kind:'LimitExceeded'});}finally{await f.db.close();}
});
test('R1b an oversized queue status is assembled from its cursor pages',async()=>{
 const counters={pending:'3',sending:'0',awaiting_outcome:'0',live:'3',blocked:false};
 const f=paged(frame=>frame.op==='queue_status'?limit():frame.op==='queue_status_page'?reply(frame.args.after===null?{...counters,commands:[{key:'a'},{key:'b'}],has_more:true}:{...counters,commands:[{key:'c'}],has_more:false}):reply(null));
 await f.open();try{
  assert.deepEqual(await f.db.queueStatus(),{...counters,commands:[{key:'a'},{key:'b'},{key:'c'}]});
  assert.deepEqual(f.frames.filter(frame=>frame.op==='queue_status_page').map(frame=>frame.args),[{after:null},{after:'b'}]);
 }finally{await f.db.close();}
});

test('R1b real core: five deliverable 300 kB outcomes exceed one frame and are all returned in order',async()=>{
 const facade=new Facade();const runtime=createRuntime({open:(s,k)=>facade.open(s,k),call:s=>facade.call(s)},{namespace:()=>new Uint8Array(16).fill(8)});
 const db=new Database(runtime,{namespace:'08'.repeat(16),context:()=>j.null(),clock:()=>1000,jitter:()=>0});
 await db.open({schema:{version:'1',tables:{notes:{columns:{body:col('String')},relations:{}}}},engine:{kind:'memory',store:null}});
 try{
  const keys=[];for(let n=0;n<5;n++)keys.push((await db.execute(definition,j.object({}),()=>j.null())).key);
  await db.preparePush(1000,0);
  for(const [n,key] of keys.entries())assert.deepEqual(await db.receiveWire(JSON.stringify([{idempotencyKey:key,status:'applied',result:String(n).repeat(300000)}])),[{key,ok:true}]);
  await assert.rejects(db.call('pending_reconciliations'),{kind:'LimitExceeded'});
  const pending=await db.pendingReconciliations();
  assert.deepEqual(pending.map(entry=>entry.key),keys);
  assert.equal(pending[4].outcome.result.value.length,300000);
  assert.deepEqual((await db.queueStatus()).commands.map(entry=>entry.key),keys);
 }finally{await db.close();facade.free();}
});
