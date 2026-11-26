import {CommandClient,j,query} from './command-client.js';
const assert=(ok,message)=>{if(!ok)throw new Error(message);};
const ordered=value=>Array.isArray(value)?value.map(ordered):value&&typeof value==='object'?Object.fromEntries(Object.entries(value).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([key,item])=>[key,ordered(item)])):value;
const equal=(a,b)=>assert(JSON.stringify(ordered(a))===JSON.stringify(ordered(b)),`expected ${JSON.stringify(b)}, received ${JSON.stringify(a)}`);
const definition={name:'order-v1',arguments:{label:{kind:'String',optional:false}},reference_paths:[]};
const args=label=>j.object({label:j.string(label)});
const fields=label=>({label:j.string(label),total:j.integer('9223372036854775807'),ratio:j.float(Number.MAX_VALUE),active:j.boolean(false),placed_at:j.timestamp('-9223372036854775808'),notes:j.string('東京\0😀')});
async function rejected(promise,kind){try{await promise;}catch(error){if(kind)equal(error.kind,kind);return error;}throw new Error('expected rejection');}
async function until(check,timeout=10000){const end=performance.now()+timeout;while(!await check()){assert(performance.now()<end,'condition timed out');await new Promise(resolve=>setTimeout(resolve,5));}}
export async function runCommandChecks({request,report}) {
 let held=null;
 const send=async message=>{const response=await request(message);if(held&&JSON.parse(message.input).op===held.op){const gate=held;held=null;gate.entered();await new Promise(resolve=>{gate.release=resolve;});}return response;};
 const identity=j.object({subject:j.string('synthetic-user'),nested:j.array([j.object({integer:j.integer('9223372036854775807'),float:j.float(1.25),absent:j.null()})])});let samples=0;
 const owned=[];
 const create=(...args)=>{const client=new CommandClient(...args);owned.push(client);return client;};
 try {
 const a=create(send,{context:()=>identity,clock:()=>{samples++;return 1000;}});
 const b=create(send,{context:()=>j.null(),clock:()=>1000});
 await a.openUnprotected('phase-four-a.sqlite');await b.openUnprotected('phase-four-b.sqlite');
 let release,entered,scopeA;const enteredPromise=new Promise(resolve=>{entered=resolve;});const order=[];
 const first=a.execute(definition,args('first'),async scope=>{
  order.push('a');scopeA=scope;
  equal(scope.context.identity,identity);equal(scope.context.now_ms,'1000');
  const row=await scope.insert('orders',fields('first'));equal(row.fields.total,j.integer('9223372036854775807'));equal(row.fields.placed_at,j.timestamp('-9223372036854775808'));equal(row.fields.notes,j.string('東京\0😀'));
  entered();await new Promise(resolve=>{release=resolve;});
  for(const label of ['one','two'])await scope.insert('items',{label:j.string(label),quantity:j.integer('1'),order_id:j.reference(row.id)});
  equal((await scope.fetch({...query('items'),source:{table:'orders',id:row.id,relation:'items'}})).length,2);
  return j.object({id:j.string(row.id),integer:j.integer('-9223372036854775808')});
 });
 await Promise.race([enteredPromise,first.then(()=>{throw new Error('handler completed before its suspension gate');})]);identity.value.subject=j.string('mutated-after-capture');
 equal(scopeA.context.identity.value.subject,j.string('synthetic-user'));equal(await a.fetch(query('orders')),[]);equal(await b.fetch(query('orders')),[]);
 await rejected(a.call('direct_batch',{writes:[]}), 'Busy');
 const second=a.execute(definition,args('second'),async scope=>{order.push('a2');equal((await scope.fetch(query('orders'))).length,1);equal((await scope.fetch(query('items'))).length,2);const row=await scope.insert('orders',{...fields('second'),ratio:j.float(42),notes:j.null()});equal(row.fields.ratio,j.float(42));equal(row.fields.notes,j.null());return j.null();});
 const third=b.execute(definition,args('third'),async()=>{order.push('b');return j.null();});
 await Promise.resolve();equal(order,['a']);release();const committed=await first;await second;await third;equal(order,['a','a2','b']);equal(samples,2);equal((await a.fetch(query('orders'))).length,2);equal((await a.fetch(query('items'))).length,2);
 await rejected(scopeA.fetch(query('orders')),'InvalidHandle');
 const frozenContext=await a.preparePush(1000);assert(frozenContext.envelopes[0].includes('synthetic-user')&&!frozenContext.envelopes[0].includes('mutated-after-capture'),'captured identity must remain frozen');
 report('H01 actual awaited scopes, committed visibility, context copy and app-wide two-owner FIFO',{order,clockSamples:samples,committedInteger:committed.result.value.integer.value,atomicRelatedItems:2});
 // Abort staged writes on thrown/cancelled handlers. Explicit cancellation releases a never-settling host promise.
 await rejected(a.execute(definition,args('throw'),async scope=>{await scope.insert('orders',fields('aborted'));throw new Error('synthetic handler failure');}));
 equal((await a.fetch(query('orders'))).length,2);
 const cancel=new AbortController();let active;const started=new Promise(resolve=>{active=resolve;});
 const never=a.execute(definition,args('never'),async scope=>{await scope.insert('orders',fields('never'));active(scope);await new Promise(()=>{});return j.null();},{signal:cancel.signal});
 const neverFailed=rejected(never,'Cancelled');const stale=await started;
 const queuedCancel=new AbortController();let queuedCalled=false;
 const skipped=a.execute(definition,args('skip'),async()=>{queuedCalled=true;return j.null();},{signal:queuedCancel.signal});const skipFailed=rejected(skipped,'Cancelled');queuedCancel.abort();cancel.abort();await neverFailed;await skipFailed;equal(queuedCalled,false);equal((await a.fetch(query('orders'))).length,2);await rejected(stale.insert('orders',fields('late')),'InvalidHandle');
 // Hold an owned finish reply after the real core transaction committed.
 let finishSeen;const finishReached=new Promise(resolve=>{finishSeen=resolve;});const finishGate={op:'finish',entered:finishSeen};held=finishGate;
 const delayedCancel=new AbortController();const delayed=a.execute(definition,args('committed'),async scope=>{const row=await scope.insert('orders',{...fields('committed'),ratio:j.float(1.25)});equal(row.fields.ratio,j.float(1.25));return j.null();},{signal:delayedCancel.signal});
 await finishReached;equal((await a.fetch(query('orders'))).length,3);delayedCancel.abort();finishGate.release();assert((await delayed).key,'committed finish returns success');
 await until(()=>!a.draining);
 report('H02 thrown/queued/active cancellation, stale scope and delayed committed finish',{committedRows:3,queuedHandlerCalls:0,lateCompletionRejected:true});
 // Owned events invoke foreign callbacks after worker return, with reentrant reads and cancellation.
 let removed=0,survivor=0,recursive=false;let token2;
 const token1=await a.observeData(query('orders'),async event=>{
  assert(typeof window==='object','main-thread callback required');equal((await a.fetch(query('orders'))).length,3);await a.unsubscribe(token2);await rejected(a.drain(),'Busy');recursive=true;event.value.value.length=0;
 });
 token2=await a.observeData(query('orders'),()=>{removed++;});
 const throwing=await a.observeData(query('orders'),()=>{throw new Error('synthetic callback failure');});
 const live=await a.observeData(query('orders'),()=>{survivor++;});
 await a.drain();equal(removed,0);equal(survivor,1);equal(recursive,true);equal(a.callbackErrors.length,1);equal((await a.fetch(query('orders'))).length,3);
 for(const token of [token1,throwing,live])await a.unsubscribe(token);
 let cancelledDelivery=0;await a.observeData(query('orders'),()=>{cancelledDelivery++;});let extracted;const extraction=new Promise(resolve=>{extracted=resolve;});const drainGate={op:'drain',entered:extracted};held=drainGate;const drain=a.drain();await extraction;await a.close();drainGate.release();equal(await drain,0);equal(cancelledDelivery,0);
 const disposed=create(send,{context:()=>j.null(),clock:()=>1000});await disposed.openUnprotected('phase-four-disposed.sqlite');
 let disposedEntered;const disposedStarted=new Promise(resolve=>{disposedEntered=resolve;});const disposedRun=disposed.execute(definition,args('close-active'),async scope=>{await scope.insert('orders',fields('discarded'));disposedEntered();await new Promise(()=>{});return j.null();});const disposedFailure=rejected(disposedRun,'Cancelled');await disposedStarted;await disposed.close();await disposedFailure;await disposed.openUnprotected('phase-four-disposed.sqlite');equal(await disposed.fetch(query('orders')),[]);
 let closedFromCallback=false;await disposed.observeData(query('orders'),async()=>{closedFromCallback=true;await disposed.close();});await disposed.drain();equal(closedFromCallback,true);
 await b.close();
 report('H04 main-thread callbacks, recursive-drain rejection, exception isolation and close after extraction',{survivingCallbacks:survivor,callbackErrors:a.callbackErrors.length,cancelledDelivery,closedActiveStagingLost:true,closedFromCallback});
 // Capacity checked before foreign handler admission, independent of the shared 500 boundary.
 const cap=create(send,{context:()=>j.null(),clock:()=>1000});await cap.openUnprotected('phase-four-cap.sqlite',{options:{capacity:'1'}});
 let invalidInvoked=0;await rejected(cap.execute(definition,args('x'.repeat(1048576)),async()=>{invalidInvoked++;return j.null();}),'InvalidPayload');
 await rejected(cap.execute(definition,args('\ud800'),async()=>{invalidInvoked++;return j.null();}),'InvalidPayload');equal(invalidInvoked,0);
 const duplicate=await send({op:'command_rpc',input:`{"op":"fetch","op":"fetch","owner":"${cap.owner}","args":{"query":{"table":"orders","predicates":[],"order":null,"limit":null}}}`});equal(JSON.parse(duplicate.value).error.kind,'InvalidPayload');
 await cap.execute(definition,args('one'),async()=>j.null());let invoked=0;await rejected(cap.execute(definition,args('two'),async()=>{invoked++;return j.null();}),'Capacity');equal(invoked,0);await cap.close();
 report('H06 configured capacity rejects before handler',{capacity:1,rejectedHandlerCalls:invoked});
 // Fake failure then actual loopback HTTP. Canonical domain bytes remain raw strings.
 const wireDef={name:'wire-v1',arguments:{amount:{kind:'Integer',optional:false}},reference_paths:[]};
 const http=create(send,{context:()=>j.null()});await http.openUnprotected('phase-four-http.sqlite',{options:{initial_delay_ms:'20',multiplier:'2',max_delay_ms:'20'}});
 const keys=[];for(let i=0;i<2;i++)keys.push((await http.execute(wireDef,j.object({amount:j.integer('9223372036854775807')}),async()=>j.null())).key);
 let statuses=0,outcomes=0;await http.observeStatus(()=>{statuses++;});for(const key of keys)await http.observeOutcome(key,()=>{outcomes++;});
 const calls=[],received=[];
 http.startSync(async body=>{
  calls.push({body,at:performance.now()});
  if(calls.length===1)throw new Error('synthetic offline');
  const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json'},body,signal:AbortSignal.timeout(5000)});assert(response.ok,'loopback HTTP status');return await response.text();
 },async pending=>{
  received.push(pending);equal(pending.outcome.result.value.echo.value.amount,j.integer('9223372036854775807'));
  return {writes:[],mappings:{}};
 });
 await until(async()=>Number((await http.queueStatus()).live)===0,15000);http.stopSync();await until(()=>!http.syncing);await http.drain();
 assert(calls.length>=2,'automatic retry required');equal(calls[0].body,calls[1].body);assert(calls[1].at-calls[0].at>=15,'persisted backoff must defer retry');equal(received.length,2);assert(statuses>0&&outcomes>=2,'status/outcome callbacks required');assert(!calls[0].body.includes('"9223372036854775807"'),'envelope integer remains numeric JSON token');
 await rejected(Promise.resolve().then(()=>j.integer(9007199254740992)),'InvalidRequest');await rejected(Promise.resolve().then(()=>j.float(Infinity)),'InvalidRequest');
 report('H03/H06 exact i64, raw canonical arrays, actual fetch, reversed-key outcomes and timer retry',{calls:calls.length,retryDelay_ms:Math.round(calls[1].at-calls[0].at),keys:keys.length,received:received.length,statusCallbacks:statuses,outcomeCallbacks:outcomes,precisionSafe:true});
 // The same real HTTP seam also exercises single, omitted, repeated and
 // contradictory members. Rust parses and correlates every raw response.
 const post=async(body,mode)=>{const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json','X-Hoya-Fixture':mode},body,signal:AbortSignal.timeout(5000)});assert(response.ok,'fixture HTTP status');return http.receiveWire(await response.text());};
 const enqueue=async count=>{for(let i=0;i<count;i++)await http.execute(wireDef,j.object({amount:j.integer('9223372036854775807')}),async()=>j.null());const batch=await http.preparePush();equal(batch.envelopes.length,count);return '['+batch.envelopes.join(',')+']';};
 const settle=async()=>{for(const pending of await http.pendingReconciliations()){await http.reconcile(pending.key,{writes:[],mappings:{}});if(pending.outcome.state==='rejected')await http.rollback(pending.key);}equal((await http.queueStatus()).live,'0');};
 const one=await post(await enqueue(1),'applied');equal(one.length,1);assert(one.every(member=>member.ok),'single receipt');await settle();
 const multi=await enqueue(3);const partial=await post(multi,'partial');equal(partial.length,1);equal((await http.pendingReconciliations()).length,1);const repeated=await post(multi,'duplicate');equal(repeated.length,6);assert(repeated.every(member=>member.ok),'equal duplicate receipt idempotency');await settle();
 const conflict=await post(await enqueue(3),'conflict');equal(conflict.filter(member=>member.ok).length,3);equal(conflict.filter(member=>!member.ok).map(member=>member.error.kind),['Protocol']);equal((await http.pendingReconciliations()).length,3);await settle();
 const mixed=await post(await enqueue(3),'mixed');assert(mixed.every(member=>member.ok),'mixed receipts');equal((await http.pendingReconciliations()).map(member=>member.outcome.state).sort(),['applied','corrected','rejected']);await settle();equal((await http.preparePush()).envelopes,[]);
 report('H06 actual HTTP single/partial/equal-duplicate/conflicting/mixed keyed outcomes',{single:one.length,partial:partial.length,duplicate:repeated.length,conflictAccepted:3,conflictRejected:1,mixed:3,acknowledgedResends:0});
 // A successful real HTTP reply and later application plan must remain owned
 // while a newly admitted foreign handler temporarily makes core return Busy.
 const overlap=create(send,{context:()=>j.null()});await overlap.openUnprotected('phase-four-overlap.sqlite');
 await overlap.execute(wireDef,j.object({amount:j.integer('9223372036854775807')}),async()=>j.null());
 let responseEntered,responseRelease,planEntered,planRelease,networkCount=0,mapperCount=0;
 const responseStarted=new Promise(resolve=>{responseEntered=resolve;});const responseHold=new Promise(resolve=>{responseRelease=resolve;});
 const planStarted=new Promise(resolve=>{planEntered=resolve;});const planHold=new Promise(resolve=>{planRelease=resolve;});
 overlap.startSync(async body=>{networkCount++;const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json'},body,signal:AbortSignal.timeout(5000)});assert(response.ok,'overlap HTTP status');const text=await response.text();responseEntered();await responseHold;return text;},async()=>{mapperCount++;planEntered();await planHold;return {writes:[],mappings:{}};});
 const pause=()=>{let entered,release;const started=new Promise(resolve=>{entered=resolve;});const gate=new Promise(resolve=>{release=resolve;});const result=overlap.execute(definition,args('overlap'),async()=>{entered();await gate;throw new Error('abort synthetic overlapping handler');});return {started,release,result:rejected(result)};};
 await responseStarted;const responseScope=pause();await responseScope.started;responseRelease();await new Promise(resolve=>setTimeout(resolve,10));equal((await overlap.pendingReconciliations()).length,0);responseScope.release();await responseScope.result;
 await planStarted;const planScope=pause();await planScope.started;planRelease();await new Promise(resolve=>setTimeout(resolve,10));equal((await overlap.pendingReconciliations()).length,1);planScope.release();await planScope.result;
 await until(async()=>Number((await overlap.queueStatus()).live)===0);overlap.stopSync();await until(()=>!overlap.syncing);equal(networkCount,1);equal(mapperCount,1);equal(overlap.syncErrors.length,0);await overlap.close();
 report('H06 real HTTP response and async mapper survive overlapping scopes',{networkRequests:networkCount,mapperCalls:mapperCount,receiptAndPlanDeferred:true});
 const missing=create(send,{clock:()=>1000});await missing.openUnprotected('phase-four-missing-context.sqlite');let missingInvoked=0;await rejected(missing.execute(definition,args('missing'),async()=>{missingInvoked++;return j.null();}),'InvalidCommand');equal(missingInvoked,0);await missing.close();
 await http.close();
 } finally { for(const client of owned) await client.close().catch(()=>{}); }
}
export async function restartChecks({request,report,state}) {
 const send=async message=>request(message);
 const client=new CommandClient(send,{context:()=>j.null(),clock:()=>1000});
 let leaveOpen=false;
 try {
 const open=version=>client.openUnprotected('phase-four-restart.sqlite',{version,authored:version==='2',options:{initial_delay_ms:'100',multiplier:'2',max_delay_ms:'1000'}});
 await open('1');
 if(!state.restart){
  let row;
  const result=await client.execute(definition,args('restart'),async scope=>{row=await scope.insert('orders',fields('restart'));return j.integer('9223372036854775807');});
  const batch=await client.preparePush(1000);equal(batch.envelopes.length,1);
  const stored={key:result.key,row,batch};
  let entered;const active=new Promise(resolve=>{entered=resolve;});
  // Deliberately leave this host await suspended across a real page reload.
  void client.execute(definition,args('lost-stage'),async scope=>{await scope.insert('orders',fields('lost-stage'));entered();await new Promise(()=>{});return j.null();});
  await active;state.restart={step:1,...stored};
  leaveOpen=true;report('H05 writer committed+frozen, second handler staged before actual reload',{committed:1,stagedUncommitted:1});return true;
 }
 const saved=state.restart;
 equal(await client.fetch(query('orders')),[saved.row]);
 if(saved.step===1){
  const early=await client.preparePush(1000);equal(early.envelopes,[]);equal(early.next_due_ms,saved.batch.next_due_ms);
  const retry=await client.preparePush(1100);equal(retry.envelopes,saved.batch.envelopes);
  await client.close();await rejected(open('2'),'SchemaConflict');await open('1');equal(await client.fetch(query('orders')),[saved.row]);
  await client.call('receive',{key:saved.key,outcome:null});equal((await client.queueStatus()).awaiting_outcome,'1');
  state.restart.step=2;await client.close();report('H05 first reload restores exact frozen bytes, due time, rows and migration guard',{frozenEqual:true,stagedLost:true,ackOnly:true});return true;
 }
 if(saved.step===2){
  equal((await client.preparePush(9999)).envelopes,[]);equal((await client.queueStatus()).awaiting_outcome,'1');
  const body=`[{"idempotencyKey":"${saved.key}","status":"applied","result":{"serverId":"server-restart","n":9223372036854775807}}]`;
  await client.receiveWire(body);equal((await client.pendingReconciliations()).length,1);state.restart.step=3;await client.close();report('H05 second reload preserves receipt-only state and stores authoritative outcome',{resends:0,pendingReconciliation:1});return true;
 }
 const pending=await client.pendingReconciliations();equal(pending.length,1);equal(pending[0].key,saved.key);equal(pending[0].outcome.result.value.n,j.integer('9223372036854775807'));
 await client.reconcile(saved.key,{writes:[{op:'put',table:'orders',id:'server-restart',fields:saved.row.fields}],mappings:{[saved.row.id]:'server-restart'}});
 equal((await client.fetch(query('orders'))).map(row=>row.id),['server-restart']);equal((await client.queueStatus()).live,'0');await client.close();await open('2');equal((await client.fetch(query('orders')))[0].id,'server-restart');await client.call('direct_batch',{writes:[{op:'replace',table:'orders',id:'server-restart',fields:{...saved.row.fields,extra:j.string('authored-v2')}}]});equal((await client.fetch(query('orders')))[0].fields.extra,j.string('authored-v2'));await client.close();
 state.restart.step=4;report('H05 third reload recovers exact outcome, reconciles ID and permits authored v2 upgrade',{reloads:3,outcomeExact:true,serverId:'server-restart',schemaVersion:2});return false;
 } finally {if(!leaveOpen) await client.close().catch(()=>{});}
}
export async function runCommandPagingChecks({request,report}) {
 const client=new CommandClient(request,{context:()=>j.null()});
 await client.openUnprotected('phase-four-pages.sqlite');
 try {
  let initial;
  const rowCommand=await client.execute(definition,args('large row'),async scope=>{initial=await scope.insert('orders',fields('x'.repeat(300000)));return j.null();});
  let changed=false;const delivered=[];const tokens=[];
  for(let index=0;index<4;index++)tokens.push(await client.observeData(query('orders'),async event=>{
   delivered.push(event.value.value.length);
   if(!changed){changed=true;await client.unsubscribe(tokens[3]);await client.call('direct_batch',{writes:[{op:'insert',table:'orders',fields:fields('callback write')} ]});}
  }));
  equal(await client.drain(),3);equal(delivered,[1,1,1]);
  equal(await client.drain(),3);equal(delivered,[1,1,1,2,2,2]);
  for(const token of tokens.slice(0,3))await client.unsubscribe(token);
  const big={name:'page-v1',arguments:{payload:{kind:'String',optional:false}},reference_paths:[]};
  const payload=j.object({payload:j.string('x'.repeat(280000))});
  for(let index=0;index<4;index++)await client.execute(big,payload,async()=>j.null());
  const posts=[];let mapped=0;
  client.startSync(async body=>{posts.push(new TextEncoder().encode(body).length);assert(posts.at(-1)<=1048576,'every HTTP prefix is bounded');const response=await fetch('/hoyadb/commands',{method:'POST',headers:{'Content-Type':'application/json'},body,signal:AbortSignal.timeout(5000)});assert(response.ok,'paged HTTP status');return response.text();},async pending=>{
   mapped++;
   if(pending.key!==rowCommand.key)return {writes:[],mappings:{}};
   const id=pending.outcome.result.value.id.value;
   return {writes:[{op:'put',table:'orders',id,fields:initial.fields}],mappings:{[initial.id]:id}};
  });
  await until(async()=>Number((await client.queueStatus()).live)===0,60000);client.stopSync();await until(()=>!client.syncing);equal(mapped,5);assert(posts.length>1,'large prepared snapshot must use multiple HTTP arrays');equal(client.syncErrors.length,0);equal((await client.fetch({...query('orders'),limit:'1'})).length,1);
  report('H04/H06 bounded event prefixes, callback-write snapshot boundary and multiple actual HTTP arrays',{firstDrain:3,nextDrain:3,cancelledAfterExtraction:1,postBytes:posts,mappedCommands:mapped});
 } finally {client.stopSync();await client.close();}
}
