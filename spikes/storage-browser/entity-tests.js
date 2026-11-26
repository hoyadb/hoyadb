import {CommandClient,j,query} from './command-client.js';
const assert=(ok,message)=>{if(!ok)throw new Error(message);};
const equal=(a,b)=>assert(JSON.stringify(a)===JSON.stringify(b),'synthetic values differ');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(check,timeout=15000){const end=performance.now()+timeout;while(!await check()){assert(performance.now()<end,'entity condition timed out');await delay(5);}}
async function rejected(work,kind){try{await work;}catch(error){if(kind)equal(error.kind,kind);return error;}throw new Error('expected entity rejection');}
const context=()=>j.object({identity:j.string('synthetic trusted identity'),max:j.integer('9223372036854775807')});
const definition={name:'entity-host-v1',arguments:{},reference_paths:[]};
const insert=country=>({op:'insert',table:'customers',fields:{country:j.string(country)}});
async function pullHttp(since,_scope,mode='normal') {
 const response=await fetch('/hoyadb/changes'+(since===null?'':'?since='+encodeURIComponent(since)),{headers:{'X-Hoya-Fixture':mode},signal:AbortSignal.timeout(5000)});
 assert(response.ok,'actual pull HTTP status');return response.text();
}
export async function runEntityChecks({request,report}) {
 const clients=[];const create=()=>{const c=new CommandClient(request,{context});clients.push(c);return c;};
 try {
  const a=create();await a.openUnprotected('phase-four-entity-host.sqlite',{options:{initial_delay_ms:'20',max_delay_ms:'20'}});
  await a.configureEntities('synthetic:phase-five',['customers']);
  const progress=[],data=[];await a.observeProgress(event=>progress.push(event.value));
  await a.observeData(query('customers'),event=>data.push(event.value),'columns',['country']);await a.drain();
  const changes=await a.entityBatch([insert('first 東京\0🌿')]);const id=changes[0].id;
  assert(id.startsWith('entity:'),'namespace-bearing stable entity ID');
  await a.patch('customers',id,{country:j.string('local patched')});
  await a.execute(definition,j.object({}),async()=>j.null());
  const bodies=[],mapper=[];
  a.startSync(async body=>{
   bodies.push(body);if(bodies.length===1)throw new Error('synthetic offline');
   const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json'},body,signal:AbortSignal.timeout(5000)});
   assert(response.ok,'actual mixed HTTP status');return response.text();
  },async pending=>{
   mapper.push(pending.key);const value=pending.outcome.result?.value;
   return {writes:[],mappings:value?.created?.value?{[value.id.value]:value.id.value}:{}};
  });
  await until(async()=>Number((await a.queueStatus()).live)===0);a.stopSync();await until(()=>!a.syncing);await a.drain();
  assert(bodies.length>=3,'mixed create/command retry followed by dependent patch');equal(bodies[0],bodies[1]);
  assert(bodies[0].includes('"writeId"')&&bodies[0].includes('"idempotencyKey"'),'one actual mixed array');
  assert(mapper.length===3,'three distinct application reconciliations');
  assert(data.length>=3,'committed create and column-update callbacks');
  report('E01–E06 actual entity mutations, mixed HTTP retries and mapping',{envelopes:3,httpArrays:bodies.length,exactRetry:true,stableId:true});
  await a.pullOnce(pullHttp);let state=await a.syncProgress();equal(state.phase,'bootstrapping');equal(state.ready,false);
  await a.pullOnce(pullHttp);state=await a.syncProgress();equal(state.phase,'ready');equal(state.pages,'2');
  const rows=await a.fetch(query('customers'));equal(rows.find(row=>row.id==='server:pull-customer').fields.country,j.string('second 東京\0🌿'));
  assert(progress.some(p=>p.phase==='bootstrapping')&&progress.some(p=>p.phase==='ready'),'partial and ready callback snapshots');
  report('E11–E13 actual paginated pull exposes partial data and sticky readiness',{pages:state.pages,rows:rows.length});
  let releaseResponse,transportEntered,handlerEntered;
  const transportStarted=new Promise(resolve=>{transportEntered=resolve;}),handlerStarted=new Promise(resolve=>{handlerEntered=resolve;});
  let busyNetworkCalls=0;
  const busyPull=a.pullOnce(async since=>{busyNetworkCalls++;const body=await pullHttp(since,null);transportEntered();await new Promise(resolve=>{releaseResponse=resolve;});return body;});
  await transportStarted;
  const controller=new AbortController();
  const suspended=a.execute(definition,j.object({}),async scope=>{await scope.insert('customers',{country:j.string('abandoned scope')});handlerEntered();await new Promise(()=>{});return j.null();},{signal:controller.signal});
  const abandoned=rejected(suspended,'Cancelled');await handlerStarted;
  releaseResponse();await delay(15);equal((await a.syncProgress()).pages,'2');
  controller.abort();await abandoned;await busyPull;equal(busyNetworkCalls,1);
  assert(!(await a.fetch(query('customers'))).some(row=>row.fields.country.value==='abandoned scope'),'aborted handler staging absent');
  report('E14 actual suspended handler retains a Busy page without repeating HTTP',{networkCalls:busyNetworkCalls,abortedStagingAbsent:true});
  let respond,requested;const requestedWork=new Promise(resolve=>{requested=resolve;});
  const pending=a.pullOnce(async(since)=>{requested();return new Promise(resolve=>{respond=()=>pullHttp(since,null).then(resolve);});});
  await requestedWork;
  await a.patch('customers',id,{country:j.string('intervening write')});
  const stale=rejected(pending,'StalePull');await respond();await stale;
  await a.pullOnce(pullHttp);
  report('E14 a suspended old revision is retired and a fresh HTTP request succeeds',{staleRejected:true,refetched:true});
  await rejected(a.reset());assert((await a.fetch(query('customers'))).length>0,'default reset preserved live patch');
  let lateReply,started;const startedWork=new Promise(resolve=>{started=resolve;});
  const late=a.pullOnce(async since=>{const body=await pullHttp(since,null);started();await new Promise(resolve=>{lateReply=resolve;});return body;});
  const disposed=rejected(late,'Closed');await startedWork;
  const before=progress.length;await a.reset('discard');lateReply();await disposed;
  equal(await a.fetch(query('customers')),[]);await a.drain();equal(progress.length,before);
  equal((await a.syncProgress()).phase,'not_started');
  report('E16–E18 reset preserves by default, discards explicitly and suppresses late callbacks',{empty:true,oldReplyInert:true});
  const correction=create();await correction.openUnprotected('phase-four-entity-correction.sqlite');await correction.configureEntities('synthetic:correction',['customers']);
  const created=(await correction.entityBatch([insert('optimistic high clock')]))[0].id;
  const entry=(await correction.queueStatus()).commands[0];await correction.preparePush();
  await correction.call('receive_entity',{table:'customers',record_id:created,write_id:entry.key,outcome:{state:'corrected',result:j.null()}});
  const selected=(value,time)=>({table:'customers',record_id:created,op:'update',changes:{country:j.string(value)},removed_fields:[],timestamp_ms:String(time),field_timestamps:{country:String(time)}});
  await correction.reconcile(entry.key,{writes:[{op:'entity',change:selected('authoritative25',25)}],mappings:{[created]:created}});
  await correction.pullOnce(async()=>JSON.stringify({checkpoint:'after-clock-correction',complete:true,changes:[selected('later30',30)]}));
  equal((await correction.fetch(query('customers')))[0].fields.country,j.string('later30'));
  report('E06 actual typed correction retains backend clocks below optimistic time',{selectedClock:'25',laterClock:'30'});
  const reused=create();await reused.openUnprotected('phase-four-entity-old-pump.sqlite');await reused.configureEntities('synthetic:old-pump',['customers']);
  await reused.entityBatch([insert('old generation')]);
  let oldRelease,newRelease,oldEntered,newEntered;
  const oldStarted=new Promise(resolve=>{oldEntered=resolve;}),newStarted=new Promise(resolve=>{newEntered=resolve;});
  const heldTransport=entered=>async body=>{const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json'},body,signal:AbortSignal.timeout(5000)});assert(response.ok,'held actual mixed HTTP');const raw=await response.text();await entered();return raw;};
  const identityPlan=async pending=>{const value=pending.outcome.result?.value;return {writes:[],mappings:value?.created?.value?{[value.id.value]:value.id.value}:{}};};
  reused.startSync(heldTransport(()=>{oldEntered();return new Promise(resolve=>{oldRelease=resolve;});}),identityPlan);await oldStarted;
  await reused.close();await reused.openUnprotected('phase-four-entity-new-pump.sqlite');await reused.configureEntities('synthetic:new-pump',['customers']);await reused.entityBatch([insert('new generation')]);
  reused.startSync(heldTransport(()=>{newEntered();return new Promise(resolve=>{newRelease=resolve;});}),identityPlan);await newStarted;
  oldRelease();await delay(15);assert(reused.syncing,'old completion cannot clear the replacement pump');newRelease();
  await until(async()=>Number((await reused.queueStatus()).live)===0);reused.stopSync();await until(()=>!reused.syncing);
  const original=create();await original.openUnprotected('phase-four-entity-old-pump.sqlite');await original.configureEntities('synthetic:old-pump',['customers']);
  equal((await original.queueStatus()).live,'1');equal((await original.fetch(query('customers')))[0].fields.country,j.string('old generation'));
  report('E18/E19 actual close/reopen isolates suspended HTTP and preserves the original file',{newPumpResolved:true,oldRecoveryUnchanged:true});
  const large=create();await large.openUnprotected('phase-four-entity-large.sqlite');await large.configureEntities('synthetic:large',['customers']);
  await large.pullOnce((since,scope)=>pullHttp(since,scope,'large'));
  const largeRows=await large.fetch(query('customers'));equal(largeRows[0].fields.country,j.string('🌿\n'.repeat(60000)));
  await large.reset();equal(await large.fetch(query('customers')),[]);
  report('E20 actual multi-chunk UTF-8 page and total reset',{valueBytes:new TextEncoder().encode('🌿\n'.repeat(60000)).length});
 } finally {for(const client of clients.reverse())await client.close();}
}

export async function entityRestart({request,report,state}) {
 const client=new CommandClient(request,{context});
 try {
  await client.openUnprotected('phase-four-entity-restart.sqlite');await client.configureEntities('synthetic:restart',['customers']);
  state.restart??={step:0};
  if(state.restart.step===0) {
   await client.pullOnce(pullHttp);const progress=await client.syncProgress();equal(progress.pages,'1');equal(progress.ready,false);
   state.restart.step=1;report('E13 first bootstrap page before actual reload',{checkpointCommitted:true,ready:false});return true;
  }
  if(state.restart.step===1) {
   let observed;await client.pullOnce(async(since,scope)=>{observed=since;return pullHttp(since,scope);});equal(observed,'fixture:page-one');
   const progress=await client.syncProgress();equal(progress.pages,'2');equal(progress.ready,true);
   const changes=await client.entityBatch([insert('queued before reload')]);state.restart.id=changes[0].id;
   state.restart.step=2;report('E13 reload resumes exact checkpoint and persists an offline diff',{pages:'2',ready:true});return true;
  }
  const rows=await client.fetch(query('customers'));assert(rows.some(row=>row.id===state.restart.id),'queued row survived reload');
  equal((await client.queueStatus()).pending,'1');await rejected(client.reset());await client.reset('discard');equal(await client.fetch(query('customers')),[]);
  state.restart.step=3;report('E04/E17 second reload retains entity recovery and explicit discard resets it',{reloads:2,recoverySurvived:true});return false;
 } finally {await client.close();}
}
