// API-0008: host policy unit tests; actual worker/OPFS tests are separate gates.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime,j} from '../src/index.js';
import {schemas} from './schemas.mjs';
// Test-only adapter keeps prior host assertions while each mock speaks ABI1.
class CommandClient extends Database {
 constructor(send,options={}) {
  const channel={open:async input=>convert(await send({input})),call:async input=>convert(await send({input}))};
  super(createRuntime(channel,{namespace:()=>new Uint8Array(16).fill(1)}),options);
 }
 openUnprotected(_path,options={}){return this.open({schema:schemas[0],engine:{kind:'memory',store:null},migrations:[],...options});}
}
function convert(response){if(response.error)return JSON.stringify({abi:'1',ok:false,error:response.error});return JSON.stringify({abi:'1',...JSON.parse(response.value)});}

test('S11 handler context rejects strict and non-strict nested mutation through awaits',async()=>{
 const original=j.object({nested:j.array([j.object({token:j.string('synthetic-private-context')})])});
 let finished=false;
 const client=new CommandClient(async({input})=>{
  const frame=JSON.parse(input);let value=null;
  if(frame.op==='request')value='600';if(frame.op==='begin')value=true;
  if(frame.op==='scope_args')value=j.object({});
  if(frame.op==='scope_context')value={identity:structuredClone(original),now_ms:'100'};
  if(frame.op==='finish'){finished=true;value={key:'test-only',result:frame.args.result};}
  if(frame.op==='drain')value={events:[],has_more:false};
  return {value:JSON.stringify({ok:true,value})};
 },{context:()=>original,clock:()=>100});
 // This test isolates immutable handler views from the separately tested open factories.
 client.owner='immutable-context-fixture';
 await client.execute({name:'readonly-v1',arguments:{},reference_paths:[]},j.object({}),async scope=>{
  const snapshot=structuredClone({identity:original,now_ms:'100'});
  const mutate=[
   ()=>{scope.context={};},
   ()=>{scope.context.identity=j.null();},
   ()=>{scope.context.now_ms='101';},
   ()=>{scope.context.identity.value.nested.value[0].value.token.value='changed';},
   ()=>scope.context.identity.value.nested.value.push(j.null()),
   ()=>Object.defineProperty(scope.context,'identity',{value:j.null()}),
   ()=>Object.setPrototypeOf(scope.context,{}),
   // Function constructors deliberately create non-strict caller code.
   Function('scope','scope.context = {};'),
   Function('scope','scope.context.identity = {};'),
   Function('scope','scope.context.identity.value.nested.value[0] = {};'),
   Function('scope','delete scope.context.identity;'),
  ];
  for(const attempt of mutate)assert.throws(()=>attempt(scope),TypeError);
  await Promise.resolve();
  assert.deepEqual(scope.context,snapshot);
  const descriptor=Object.getOwnPropertyDescriptor(scope.context,'identity');
  assert.throws(()=>{descriptor.value.value.nested.value[0]=j.null();},TypeError);
  assert.deepEqual(scope.context,snapshot);
  return j.null();
 });
 assert.equal(finished,true);await client.close();
});
const definition={name:'test-v1',arguments:{label:{kind:'String',optional:false}},reference_paths:[]};
test('E01 entity calls capture trusted context atomically with each mutation',async()=>{
 const frames=[];let sampled=0;
 const client=new CommandClient(async({input})=>{const frame=JSON.parse(input);frames.push(frame);return {value:JSON.stringify({ok:true,value:frame.op==='open'?'51':frame.op==='entity_batch'?[]:frame.op==='drain'?{events:[],has_more:false}:null})};},{context:()=>{sampled++;return j.object({min:j.integer('-9223372036854775808')});},clock:()=>1001});
 await client.openUnprotected('phase-four-entity-node.sqlite');
 await client.configureEntities('tenant:one',['customers']);
 await client.entityBatch([{op:'insert',table:'customers',fields:{country:j.string('東京\0🌿')}}]);
 await client.patch('customers','entity:sample',{country:j.string('next')});
 assert.equal(sampled,2);
 assert.deepEqual(frames.filter(x=>x.op==='entity_configure')[0].args,{config:{scope:'tenant:one',tables:['customers']}});
 for(const frame of frames.filter(x=>['entity_batch','patch'].includes(x.op))) {
  assert.equal(frame.args.context.now_ms,'1001');assert.deepEqual(frame.args.context.identity,j.object({min:j.integer('-9223372036854775808')}));
 }
 assert.equal(frames.filter(x=>x.op==='entity_batch').length,1);assert.equal(frames.filter(x=>x.op==='patch').length,1);
 await client.close();
});
test('E13 complete bounded UTF8 pull uses one core decode and one production apply',async()=>{
 const frames=[],large='🌿\n'.repeat(60000);
 const page={checkpoint:'opaque / 1',complete:false,changes:[{table:'articles',record_id:'a1',op:'update',changes:{title:j.string(large)},removed_fields:[],timestamp_ms:'1',field_timestamps:{title:'1'}}]};
 const body=JSON.stringify(page);let calls=0;
 const box=value=>value===null?j.null():typeof value==='string'?j.string(value):typeof value==='boolean'?j.boolean(value):Array.isArray(value)?j.array(value.map(box)):j.object(Object.fromEntries(Object.entries(value).map(([key,item])=>[key,box(item)])));
 const client=new CommandClient(async({input})=>{const f=JSON.parse(input);frames.push(f);let value=null;
  if(f.op==='open')value='52';
  if(f.op==='begin_pull')value={request:'17',since:'opaque / zero',scope:'tenant:one',generation:'1'};
  if(f.op==='json_decode'){assert.equal(f.args.body,body);value=box(page);}
  if(f.op==='apply_pull')assert.deepEqual(f.args.page,page);
  if(f.op==='drain')value={events:[],has_more:false};
  return {value:JSON.stringify({ok:true,value})};
 });
 await client.openUnprotected('phase-four-pull-node.sqlite');
 await client.pullOnce(async(since,scope)=>{calls++;assert.equal(since,'opaque / zero');assert.equal(scope,'tenant:one');return body;});
 assert.equal(calls,1);assert.equal(frames.filter(f=>f.op==='json_decode').length,1);assert.equal(frames.filter(f=>f.op==='apply_pull').length,1);await client.close();
});
test('E18 reset changes foreign generation and callbacks only after successful core reset',async()=>{
 let rejectReset=true;
 const client=new CommandClient(async({input})=>{const f=JSON.parse(input);return {value:JSON.stringify(f.op==='reset'&&rejectReset?{ok:false,error:{kind:'Busy'}}:{ok:true,value:f.op==='open'?'53':null})};});
 await client.openUnprotected('phase-four-reset-node.sqlite');client.callbacks.set('saved',()=>{});
 const previous=client.generation;await assert.rejects(client.reset(),{kind:'Busy'});
 assert.equal(client.generation,previous);assert.ok(client.callbacks.has('saved'));
 rejectReset=false;await client.reset('discard_recovery');assert.equal(client.generation,previous+1);assert.equal(client.callbacks.size,0);
 await client.close();
});
test('E18 late transport completion cannot clear a replacement generation sync pump',async()=>{
 let prepared=0,oldRelease,newRelease;
 const client=new CommandClient(async({input})=>{const f=JSON.parse(input);let value=null;
  if(f.op==='open')value='54';
  if(f.op==='prepare_push')value={envelopes:++prepared<=2?['{}']:[],next_due_ms:null,has_more:false};
  if(f.op==='receive_wire'||f.op==='pending_reconciliations')value=[];
  if(f.op==='drain')value={events:[],has_more:false};
  return {value:JSON.stringify({ok:true,value})};
 });
 const wait=async condition=>{const end=Date.now()+500;while(!condition()){assert.ok(Date.now()<end);await new Promise(resolve=>setTimeout(resolve,2));}};
 await client.openUnprotected('phase-four-generation-node.sqlite');
 try {
  client.startSync(()=>new Promise(resolve=>{oldRelease=resolve;}));await wait(()=>oldRelease);
  await client.reset('discard_recovery');
  client.startSync(()=>new Promise(resolve=>{newRelease=resolve;}));await wait(()=>newRelease);
  oldRelease('[]');await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(client.syncing,true,'old finally must not clear the new pump');
 } finally {client.stopSync();oldRelease?.('[]');newRelease?.('[]');await new Promise(resolve=>setTimeout(resolve,5));await client.close();}
});
test('E18 close and reopen can start a new pump while the old transport remains suspended',async()=>{
 let prepared=0,oldRelease,newRelease,owners=70;
 const client=new CommandClient(async({input})=>{const f=JSON.parse(input);let value=null;
  if(f.op==='open')value=String(++owners);
  if(f.op==='prepare_push')value={envelopes:++prepared<=2?['{}']:[],next_due_ms:null,has_more:false};
  if(f.op==='receive_wire'||f.op==='pending_reconciliations')value=[];
  if(f.op==='drain')value={events:[],has_more:false};
  return {value:JSON.stringify({ok:true,value})};
 });
 const wait=async condition=>{const end=Date.now()+500;while(!condition()){assert.ok(Date.now()<end,'old suspended pump blocked the reopened generation');await new Promise(resolve=>setTimeout(resolve,2));}};
 await client.openUnprotected('phase-four-old-generation.sqlite');
 try {
  client.startSync(()=>new Promise(resolve=>{oldRelease=resolve;}));await wait(()=>oldRelease);
  await client.close();await client.openUnprotected('phase-four-new-generation.sqlite');
  client.startSync(()=>new Promise(resolve=>{newRelease=resolve;}));await wait(()=>newRelease);
  oldRelease('[]');await new Promise(resolve=>setTimeout(resolve,10));assert.equal(client.syncing,true);
 } finally {client.stopSync();oldRelease?.('[]');newRelease?.('[]');await new Promise(resolve=>setTimeout(resolve,5));await client.close();}
});
function transport() {
 let request=1;
 return async ({input})=>{
  const frame=JSON.parse(input);let value=null;
  switch(frame.op){
   case 'open':value='1';break;
   case 'request':value=String(++request);break;
   case 'begin':value=true;break;
   case 'scope_args':value=j.object({label:j.string('value')});break;
   case 'scope_context':value={identity:j.null(),now_ms:'100'};break;
   case 'finish':value={key:`key-${frame.args.request}`,result:frame.args.result};break;
   case 'drain':value={events:[],has_more:false};break;
  }
  return {value:JSON.stringify({ok:true,value})};
 };
}
test('H01 host invokes its awaited handler and returns the owned result',async()=>{
 const client=new CommandClient(transport(),{context:()=>j.null(),clock:()=>100});
 await client.openUnprotected('phase-four-node.sqlite');let calls=0;
 const result=await client.execute(definition,j.object({label:j.string('value')}),async()=>{calls++;await Promise.resolve();return j.integer('9223372036854775807');});
 assert.equal(calls,1);assert.deepEqual(result.result,j.integer('9223372036854775807'));
 await client.close();
});
test('H03 unsafe integer Number coercion rejects before transport',()=>{
 assert.throws(()=>j.integer(9007199254740992));
 assert.deepEqual(j.integer(9223372036854775807n),{kind:'integer',value:'9223372036854775807'});
 assert.throws(()=>j.float(Infinity));
});
test('H01 two owners execute FIFO and active cancellation releases a never-settling handler',async()=>{
 const a=new CommandClient(transport(),{context:()=>j.null(),clock:()=>100});
 const b=new CommandClient(transport(),{context:()=>j.null(),clock:()=>100});
 await a.openUnprotected('phase-four-a.sqlite'); await b.openUnprotected('phase-four-b.sqlite');
 const controller=new AbortController();let entered, calls=[];
 const started=new Promise(resolve=>{entered=resolve;});
 const first=a.execute(definition,j.object({label:j.string('value')}),async scope=>{calls.push('a');entered(scope);await new Promise(()=>{});return j.null();},{signal:controller.signal});
 const failure=assert.rejects(first,{kind:'Cancelled'});
 const stale=await started;
 const second=b.execute(definition,j.object({label:j.string('value')}),async()=>{calls.push('b');return j.null();});
 await Promise.resolve();assert.deepEqual(calls,['a']); controller.abort();
 await failure;await second;assert.deepEqual(calls,['a','b']);
 await assert.rejects(stale.fetch({table:'orders',predicates:[],order:null,limit:null}),{kind:'InvalidHandle'});
 await a.close();await b.close();
});
test('H02 cancellation while finish reply is delayed preserves committed success',async()=>{
 const base=transport();let committed,release;
 const delayed=async message=>{const response=await base(message);if(JSON.parse(message.input).op==='finish'){committed();await new Promise(resolve=>{release=resolve;});}return response;};
 const client=new CommandClient(delayed,{context:()=>j.null(),clock:()=>100});await client.openUnprotected('phase-four-finish.sqlite');
 const controller=new AbortController();const finished=new Promise(resolve=>{committed=resolve;});
 const result=client.execute(definition,j.object({label:j.string('value')}),async()=>j.null(),{signal:controller.signal});
 await finished;controller.abort();release();assert.equal((await result).key.startsWith('key-'),true);await client.close();
});
test('H01 pending begin waits for a release signal without polling or calling handler',async()=>{
 let locked=true,begins=0,called=0;const base=transport();
 const send=async message=>{const op=JSON.parse(message.input).op;if(op==='begin'){begins++;return {value:JSON.stringify({ok:true,value:!locked})};}if(op==='cancel_request')locked=false;return base(message);};
 const client=new CommandClient(send,{context:()=>j.null(),clock:()=>100});await client.openUnprotected('phase-four-pending.sqlite');
 const promise=client.execute(definition,j.object({label:j.string('value')}),async()=>{called++;return j.null();});
 await new Promise(resolve=>setTimeout(resolve,10));assert.equal(called,0);assert.equal(begins,1);
 await client.call('cancel_request',{request:'999'});await promise;assert.equal(called,1);assert.equal(begins,2);await client.close();
});
test('H06 owned HTTP response and application plan survive active-handler Busy without repeating work',async()=>{
 const base=transport();let active=false,sent=false,received=false,reconciled=false,networkCalls=0,mapperCalls=0;
 let networkEntered,networkRelease,mapperEntered,mapperRelease;
 const networkStarted=new Promise(resolve=>{networkEntered=resolve;});
 const response=new Promise(resolve=>{networkRelease=resolve;});
 const mapperStarted=new Promise(resolve=>{mapperEntered=resolve;});
 const plan=new Promise(resolve=>{mapperRelease=resolve;});
 const wire=async message=>{
  const frame=JSON.parse(message.input);let value;
  switch(frame.op){
   case 'prepare_push':value={envelopes:sent?[]:['{"command":"synthetic"}'],next_due_ms:null};sent=true;break;
   case 'begin':active=true;return base(message);
   case 'finish':case 'abort':active=false;return base(message);
   case 'receive_wire':if(active){value=[{key:'key',ok:false,error:{kind:'Busy'}}];break;}received=true;value=[{key:'key',ok:true}];break;
   case 'pending_reconciliations':value=received&&!reconciled?[{key:'key',outcome:{state:'applied',result:j.null()}}]:[];break;
   case 'reconcile_resolving':if(active)return {value:JSON.stringify({ok:false,error:{kind:'Busy'}})};reconciled=true;value={kind:'completed'};break;
   default:return base(message);
  }
  return {value:JSON.stringify({ok:true,value})};
 };
 const client=new CommandClient(wire,{context:()=>j.null(),clock:()=>100});await client.openUnprotected('phase-four-overlap.sqlite');
 const wait=async condition=>{const end=Date.now()+500;while(!condition()){assert.ok(Date.now()<end,'owned response/plan was dropped after Busy');await new Promise(resolve=>setTimeout(resolve,2));}};
 const suspended=()=>{let started,release;const entered=new Promise(resolve=>{started=resolve;});const hold=new Promise(resolve=>{release=resolve;});const result=client.execute(definition,j.object({label:j.string('value')}),async()=>{started();await hold;return j.null();});return {entered,release,result};};
 try {
  client.startSync(async()=>{networkCalls++;networkEntered();return response;},async()=>{mapperCalls++;mapperEntered();return plan;});
  await networkStarted;const one=suspended();await one.entered;networkRelease('[]');await new Promise(resolve=>setTimeout(resolve,10));assert.equal(received,false);one.release();await one.result;
  await wait(()=>mapperCalls>0);await mapperStarted;const two=suspended();await two.entered;mapperRelease({writes:[],mappings:{}});await new Promise(resolve=>setTimeout(resolve,10));assert.equal(reconciled,false);two.release();await two.result;
  await wait(()=>reconciled);assert.equal(networkCalls,1);assert.equal(mapperCalls,1);assert.equal(client.syncErrors.length,0);
 }finally{client.stopSync();await client.close();}
});
test('H06 a finish wake during an awaited mapper is replayed when the old batch has no deadline',async()=>{
 const base=transport();let queued=false,resolved=false,networkCalls=0,mapperEntered,mapperRelease;
 const started=new Promise(resolve=>{mapperEntered=resolve;});const gate=new Promise(resolve=>{mapperRelease=resolve;});
 const wire=async message=>{const frame=JSON.parse(message.input);let value;
  switch(frame.op){
   case 'prepare_push':value={envelopes:queued?['{"command":"next"}']:[],next_due_ms:null};break;
   case 'finish':queued=true;return base(message);
   case 'receive_wire':queued=false;value=[];break;
   case 'pending_reconciliations':value=resolved?[]:[{key:'old',outcome:{state:'applied',result:j.null()}}];break;
   case 'reconcile_resolving':resolved=true;value={kind:'completed'};break;
   default:return base(message);
  }return {value:JSON.stringify({ok:true,value})};
 };
 const client=new CommandClient(wire,{context:()=>j.null(),clock:()=>100});await client.openUnprotected('phase-four-wake.sqlite');
 try{
  client.startSync(async()=>{networkCalls++;return '[]';},async()=>{mapperEntered();await gate;return {writes:[],mappings:{}};});await started;
  await client.execute(definition,j.object({label:j.string('value')}),async()=>j.null());await new Promise(resolve=>setTimeout(resolve,5));mapperRelease();
  const end=Date.now()+500;while(networkCalls===0&&Date.now()<end)await new Promise(resolve=>setTimeout(resolve,2));assert.equal(networkCalls,1,'a committed new command stalled after its wake was dropped');
  assert.equal(resolved,true,'the retained mapper proposal reached reconciliation');assert.equal(client.syncErrors.length,0);
 }finally{client.stopSync();await client.close();}
});
test('H04 bounded event pages retain one drain guard and callback liveness',async()=>{
 const base=transport();let page=0,delivered=0;
 const send=async message=>{const op=JSON.parse(message.input).op;if(op==='observe')return {value:JSON.stringify({ok:true,value:'700'})};if(op==='drain')return {value:JSON.stringify({ok:true,value:{events:[{subscription:'700',kind:'data',value:{kind:'count',count:String(++page)}}],has_more:page===1}})};return base(message);};
 const client=new CommandClient(send,{context:()=>j.null(),clock:()=>100});await client.openUnprotected('phase-four-pages.sqlite');
 await client.observeData({table:'orders',predicates:[],order:null,limit:null},async()=>{delivered++;await assert.rejects(client.drain(),{kind:'Busy'});});
 assert.equal(await client.drain(),2);assert.equal(delivered,2);assert.equal(page,2);await client.close();
});
test('H06 each bounded prepare prefix is a separate nonempty HTTP array',async()=>{
 const base=transport();let page=0;const bodies=[];
 const send=async message=>{const op=JSON.parse(message.input).op;let value;
  if(op==='prepare_push'){page++;value={envelopes:page<3?[`{"part":${page}}`]:[],next_due_ms:null,has_more:page===1};}
  else if(op==='receive_wire'||op==='pending_reconciliations')value=[];else return base(message);
  return {value:JSON.stringify({ok:true,value})};
 };
 const client=new CommandClient(send,{context:()=>j.null(),clock:()=>100});await client.openUnprotected('phase-four-push-pages.sqlite');
 try{client.startSync(async body=>{bodies.push(body);return '[]';});const end=Date.now()+500;while(bodies.length<2&&Date.now()<end)await new Promise(resolve=>setTimeout(resolve,2));assert.deepEqual(bodies,['[{"part":1}]','[{"part":2}]']);}
 finally{client.stopSync();await client.close();}
});
