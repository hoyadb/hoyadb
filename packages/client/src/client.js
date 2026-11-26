// API-0011: host scheduling only; all database semantics remain in Rust.
import {check,failure,publicFailure} from './errors.js';
import {copyOwned as copy,immutableOwned as immutableContext,integer,unsigned,handle,string,j,unboxWire} from './values.js';
import {Query,queryWire} from './query.js';
import {timerDelay} from './time.js';
const outer=value=>{copy(value);};
const immutableContextError=()=>{throw new TypeError('Trusted context is read-only');};
const syncTransport=transport=>{const push=typeof transport==='function'?transport:transport?.push;check(typeof push==='function');return typeof transport==='function'?push:push.bind(transport);};
const clients = new Set();
let releaseEpoch = 0;
const releaseWaiters = new Set();
function released() { releaseEpoch++; for (const wake of releaseWaiters) wake(); releaseWaiters.clear(); }
function waitRelease(epoch, cancellation) {
  if (epoch !== releaseEpoch) return Promise.resolve();
  let wake;
  const pending = new Promise(resolve => { wake = resolve; releaseWaiters.add(wake); });
  return Promise.race([pending,cancellation]).finally(() => releaseWaiters.delete(wake));
}

export class Database {
  #jobs = [];
  #pumping = false;
  #opening = null;
  #closing = null;
  #invalidated = null;
  #invalidate = null;
  #autoClock=null;
  #hostTimers=null;
  #autoTimer=null;
  #countTimer=null;
  #drainAgain=false;
  #countTokens=new Set();
  #releaseSubscriptions=new Set();
  constructor(runtime, options = {}) {
    this.runtime = runtime; this.options = Object.fromEntries(['context','clock','idGenerator','jitter','namespace','monotonic','timers','onBackgroundError','conflict','onResolution'].filter(key=>options[key]!==undefined).map(key=>[key,options[key]])); this.owner = null; this.generation = 0;
    this.mergeHooks=new Map();if(options.mergeHooks)this.setMergeHooks(options.mergeHooks);
    this.callbacks = new Map(); this.callbackErrors = []; this.draining = false;
    this.transport = null; this.timer = null; this.syncing = false; this.syncRun = null; this.syncWakePending = false; this.syncErrors = [];
  }
  async call(op,args={},owner=this.owner) {
    if(op!=='close')check(owner&&owner===this.owner&&!this.#closing,'Closed');
    try { return await this.runtime.call(op,args,owner); }
    finally { if(['finish','abort','cancel_request','close'].includes(op))released(); }
  }
  async open(options={}) {
    const memory=options.engine?.kind==='memory';
    if(memory)check(options.encryptionKey===undefined);
    else check(options.encryptionKey instanceof Uint8Array&&options.encryptionKey.byteLength===32);
    return this.#open(options,memory?new Uint8Array(0):new Uint8Array(options.encryptionKey));
  }
  async #open(options,key) {
    if(this.owner||this.#opening||this.#closing){key.fill(0);throw failure('Busy');}
    let resolve,reject;
    const received=new Promise((yes,no)=>{resolve=yes;reject=no;});
    const attempt={cancelled:false,received};this.#opening=attempt;
    (async()=>{
      try {
        // Capture and validate host options before acquiring a native owner.
        attempt.schema=copy(options.schema);
        attempt.conflict=this.options.conflict===undefined?undefined:conflictPolicy(this.options.conflict);
        check(this.options.onResolution===undefined||typeof this.options.onResolution==='function');
        const transport=options.transport;attempt.transport=transport?syncTransport(transport):null;attempt.reconcile=options.reconcile;
        const namespace=this.options.namespace??await this.runtime.namespace();
        check(typeof namespace==='string'&&/^[0-9a-f]{32}$/.test(namespace));
        check(!attempt.cancelled,'Closed');
        return await this.runtime.open({engine:copy(options.engine),schema:attempt.schema,migrations:copy(options.migrations||[]),namespace,options:{capacity:'1000',history:'100',initial_delay_ms:'1000',multiplier:'2',max_delay_ms:'60000',...options.options}},key);
      } finally {if(key.byteLength)key.fill(0);}
    })().then(resolve,reject);
    // Configuration owns the unpublished handle. Close waits for this operation
    // before disposal, and a failed disposal keeps the same handle for retry.
    attempt.ready=received.then(async owner=>{
      check(!attempt.cancelled,'Closed');
      if(attempt.conflict!==undefined)await this.runtime.call('set_conflict_policy',{policy:attempt.conflict},owner);
      check(!attempt.cancelled,'Closed');return owner;
    });
    try {
      const owner=await attempt.ready;check(!attempt.cancelled,'Closed');
      if(this.#opening===attempt)this.#opening=null;
      this.schemaValue=attempt.schema;this.owner=owner;this.generation++;this.#invalidated=new Promise(resolve=>{this.#invalidate=resolve;});clients.add(this);
      if(attempt.transport)this.startSync(attempt.transport,attempt.reconcile);
      return this;
    } catch(error){if(attempt.cancelled)throw failure('Closed');await this.close();throw error;}
    finally {if(!attempt.cancelled&&this.#opening===attempt)this.#opening=null;}
  }
  execute(definition, args, handler, options = {}) {
    try { outer(definition); outer(args); check(this.owner, 'Closed'); check(typeof handler === 'function'); }
    catch (error) { return Promise.reject(error); }
    const generation = this.generation, owner = this.owner;
    let identity, now;
    try { check(typeof this.options.context==='function','InvalidCommand');identity=copy(this.options.context()); outer(identity); now = integer((this.options.clock || Date.now)()); }
    catch (error) { return Promise.reject(error); }
    let resolve, reject, cancelled, completed;
    const result = new Promise((yes,no) => { resolve = yes; reject = no; });
    const cancellation = new Promise(yes => { cancelled = yes; });
    const completion = new Promise(yes => { completed = yes; });
    const job = {client:this,definition:copy(definition),args:copy(args),handler,identity,now,generation,owner,resolve,reject,cancellation,cancelled,completion,completed,dead:false,finishing:false};
    // The host enqueues synchronously, before allocating the core ticket.
    this.#jobs.push(job);
    job.cancel = () => { if (!job.finishing) { job.dead = true; cancelled(); reject(failure('Cancelled')); } };
    if (options.signal) {
      job.signal = options.signal;
      job.signal.addEventListener('abort',job.cancel,{once:true});
      if (job.signal.aborted) job.cancel();
    }
    void this.#pump(); return result;
  }
  async #pump() {
    if (this.#pumping) return;
    this.#pumping = true;
    try {
      while (this.#jobs.length) {
        await this.run(this.#jobs[0]);
        this.#jobs.shift();
      }
    } finally {
      this.#pumping = false;
      released();
      for (const client of clients) client.wakeSync();
    }
  }
  #listenRelease(owner,generation) {
    let active=true,unsubscribe;
    try {
      const subscribe=this.runtime.channel.subscribeRelease;
      if(subscribe===undefined)return ()=>{};
      check(typeof subscribe==='function');
      unsubscribe=subscribe.call(this.runtime.channel,()=>{
        if(active&&owner===this.owner&&generation===this.generation)released();
      });
      check(typeof unsubscribe==='function');
    } catch(error) {active=false;throw publicFailure(error);}
    const stop=()=>{
      if(!active)return;active=false;this.#releaseSubscriptions.delete(stop);
      try{unsubscribe();}catch(error){this.#remember(this.callbackErrors,publicFailure(error));}
    };
    this.#releaseSubscriptions.add(stop);return stop;
  }
  #retireReleaseSubscriptions(){for(const stop of this.#releaseSubscriptions)stop();}
  async run(job) {
    let request, admitted = false,stopRelease=()=>{};
    const live = () => !job.dead && job.generation === this.generation && job.owner === this.owner;
    const scoped = async (op,args = {}) => { check(live() && admitted && !job.finishing,'InvalidHandle'); return this.call(op,{request,...args},job.owner); };
    try {
      check(live(),'Cancelled');
      stopRelease=this.#listenRelease(job.owner,job.generation);
      void job.cancellation.then(()=>stopRelease());
      request = await this.call('request',{},job.owner);
      check(live(),'Cancelled');
      while (!admitted) {
        const epoch = releaseEpoch;
        admitted = await this.call('begin',{request,definition:job.definition,value:job.args,context:{identity:job.identity,now_ms:job.now}},job.owner);
        check(live(),'Cancelled');
        if (!admitted) await waitRelease(epoch,job.cancellation);
        check(live(),'Cancelled');
      }
      stopRelease();
      const scope = {
        args:await scoped('scope_args'),
        fetch:query => scoped('scope_fetch',{query:queryWire(query)}),
        insert:async(table,fields) => scoped('scope_insert',{table,fields,proposed_id:this.proposedId()}),
        replace:(table,id,fields) => scoped('scope_replace',{table,id,fields}),
        remove:(table,id) => scoped('scope_remove',{table,id}),
        batch:async writes => scoped('scope_batch',{writes:this.prepareWrites(writes)}),
        link:async(table,id,relation,target,fields={})=>scoped('scope_link',{table,id,relation,target,fields,proposed_id:this.proposedId()}),
        unlink:(table,id,relation,target)=>scoped('scope_unlink',{table,id,relation,target}),
      };
      const context = immutableContext(await scoped('scope_context'));
      Object.defineProperty(scope,'context',{enumerable:true,configurable:false,
        get:()=>context,set:immutableContextError});
      const work = Promise.resolve().then(() => { check(live(),'Cancelled'); return job.handler(scope); });
      const winner = await Promise.race([work.then(value => ({value})),job.cancellation.then(() => ({cancelled:true}))]);
      check(!winner.cancelled && live(),'Cancelled');
      outer(winner.value); job.finishing = true;
      const committed = await this.call('finish',{request,result:winner.value},job.owner);
      admitted = false; request = null; job.finishing = false;
      job.resolve(committed);
      // A callback failure cannot change the durable handler result.
      if (job.generation === this.generation && !this.#closing){await this.#backgroundDrain();this.scheduleCount();}
    } catch (error) { job.reject(error); }
    finally {
      stopRelease();
      if (request && job.owner === this.owner) {
        try { await this.call(admitted ? 'abort' : 'cancel_request',{request},job.owner); } catch (_) { /* close already disposed the ticket */ }
      }
      if (job.signal) job.signal.removeEventListener('abort',job.cancel);
      job.completed();
    }
  }
  query(table,source=null){return new Query(table,source,null,this);}
  related(table,id,relation){const target=this.schemaValue?.tables?.[table]?.relations?.[relation]?.table;return this.query(target??'',{table,id,relation});}
  fetch(query) { return this.call('fetch',{query:queryWire(query)}); }
  schema(){return this.call('schema');}
  protection(){return this.call('protection');}
  diagnostics(){return this.call('diagnostics');}
  proposedId(){if(this.options.idGenerator===undefined)return null;check(typeof this.options.idGenerator==='function');const id=string(this.options.idGenerator());check(id.length>0);return id;}
  prepareWrites(writes){return normalizeWrites(writes).map(write=>write.op==='insert'&&write.proposed_id===null?{...write,proposed_id:this.proposedId()}:write);}
  async mutation(op,args){const generation=this.generation;const result=await this.call(op,args);if(generation===this.generation){this.wakeSync();await this.#backgroundDrain();this.scheduleCount();}return result;}
  async insert(table,fields){return this.mutation('insert',{table,fields,proposed_id:this.proposedId()});}
  replace(table,id,fields){return this.mutation('replace',{table,id,fields});}
  remove(table,id){return this.mutation('remove',{table,id});}
  async batch(writes){return this.mutation('batch',{writes:this.prepareWrites(writes)});}
  async link(table,id,relation,target,fields={}){return this.mutation('link',{table,id,relation,target,fields,proposed_id:this.proposedId()});}
  unlink(table,id,relation,target){return this.mutation('unlink',{table,id,relation,target});}
  entityLink(table,id,relation,target,fields={}){return this.entityMutation('entity_link',{table,id,relation,target,fields});}
  entityUnlink(table,id,relation,target){return this.entityMutation('entity_unlink',{table,id,relation,target});}
  reconfigureEntities(scope,tables){return this.mutation('entity_reconfigure',{config:{scope,tables}});}
  receive(key,outcome){return this.mutation('receive',{key,outcome});}
  receiveEntity(table,record_id,write_id,outcome){return this.mutation('receive_entity',{table,record_id,write_id,outcome});}
  configureEntities(scope,tables) { return this.call('entity_configure',{config:{scope,tables}}); }
  configureEntitiesDurable(scope,tables) { return this.call('entity_configure_durable',{config:{scope,tables}}); }
  resolveEntityId(table,id) { return this.call('resolve_entity_id',{table,id}); }
  capturedEntity() {
    check(typeof this.options.context === 'function','InvalidCommand');
    const identity=copy(this.options.context());outer(identity);
    return {identity,now_ms:integer((this.options.clock || Date.now)())};
  }
  async entityMutation(op,args) {
    const generation=this.generation;
    const result=await this.call(op,{...args,context:this.capturedEntity()});
    if(generation===this.generation) {
      this.wakeSync();
      await this.#backgroundDrain();this.scheduleCount();
    }
    return result;
  }
  entityBatch(writes) { return this.entityMutation('entity_batch',{writes:normalizeWrites(writes)}); }
  entityBatchAliased(writes) { return this.entityMutation('entity_batch_aliased',{writes:normalizeWrites(writes).map(write=>write.op==='insert'?{...write,alias:write.alias??null,references:write.references??{}}:['replace','patch'].includes(write.op)?{...write,references:write.references??{}}:write)}); }
  patch(table,id,fields) { return this.entityMutation('patch',{table,id,fields}); }
  syncProgress() { return this.call('sync_progress'); }
  observeProgress(callback) { return this.subscribe('observe_progress',{},callback); }
  setMergeHooks(hooks){check(hooks&&typeof hooks==='object'&&!Array.isArray(hooks));const next=new Map();for(const [table,hook]of Object.entries(hooks)){string(table);check(typeof hook==='function','InvalidCommand');next.set(table,hook);}this.mergeHooks=next;}
  async pullOnce(transport) {
    check(typeof transport==='function');
    const owner=this.owner,generation=this.generation,hooks=new Map(this.mergeHooks);
    const live=()=>owner===this.owner && generation===this.generation;
    const schema=hooks.size?immutableContext(await this.schema()):null;
    while(live()){
      const batch=await this.call('begin_pull',{},owner);let consumed=false;
      try {
        const body=string(await transport(batch.since,batch.scope));check(live(),'Closed');
        const page=unboxWire(await this.runtime.decodeJson(body));check(live(),'Closed');
        if(hooks.size){
          let step=await this.resumeOwned(()=>this.call('merge_prepare',{request:batch.request,page,tables:[...hooks.keys()]},owner),owner,generation);check(live(),'Closed');
          while(step.kind==='offer'){
            const parts={schema};
            for(const part of ['current','metadata','incoming','default']){parts[part]=await this.call('merge_input',{session:step.session,step:step.step,part},owner);check(live(),'Closed');}
            const hook=hooks.get(step.table);check(typeof hook==='function','InvalidCommand');
            const proposal=copy(await Promise.race([Promise.resolve().then(()=>hook(immutableContext(parts))),this.#invalidated.then(()=>{throw failure('Closed');})]));check(live(),'Closed');
            const previous=step;step=await this.resumeOwned(()=>this.call('merge_reply',{session:previous.session,step:previous.step,row:proposal},owner),owner,generation);check(live(),'Closed');
          }
          check(step.kind==='ready','InvalidPayload');
          await this.resumeOwned(()=>this.call('merge_commit',{session:step.session},owner),owner,generation);
        }else await this.resumeOwned(()=>this.call('apply_pull',{request:batch.request,page},owner),owner,generation);
        check(live(),'Closed');consumed=true;
        await this.#backgroundDrain();return this.syncProgress();
      } catch(error){if(error.kind!=='StalePull'||!live())throw error;}
      finally {if(!consumed && live())try{await this.call('abort_pull',{request:batch.request},owner);}catch(_){/* stale core session already retired its ticket */}}
      // A stale page is discarded. The next loop fetches from a new core ticket
      // and its unchanged checkpoint, never replaying old bytes at a revision.
    }
    throw failure('Closed');
  }
  async reset(mode = 'preserve_recovery') {
    check(this.#jobs.length===0,'Busy');check(['preserve_recovery','discard_recovery'].includes(mode));
    await this.call('reset',{mode});
    this.generation++;this.#invalidate?.();this.#retireReleaseSubscriptions();this.#invalidated=new Promise(resolve=>{this.#invalidate=resolve;});this.cancelObservationTimers();this.callbacks.clear();this.syncSubscriptions?.clear();this.stopSync();
    this.syncRun=null;this.syncing=false;released();
  }
  queueStatus() { return this.#paged('queue_status','commands'); }
  // API-0011 §4 paged reads: the unpaged call stays the fast path. Only when its complete reply
  // exceeds one frame are the core-owned prefixes followed by key cursor; core orders and bounds them.
  async #paged(op,list) {
    let whole;
    try { return await this.call(op); } catch (error) { if (error?.kind !== 'LimitExceeded') throw error; whole = error; }
    for (let attempt = 0; ; attempt++) {
      const items = [];let after = null, first = null, page;
      try {
        do {
          page = await this.call(op+'_page',{after});
          check(page && Array.isArray(page[list]) && typeof page.has_more === 'boolean' && (page[list].length > 0 || !page.has_more),'InvalidPayload');
          first ??= page;for (const item of page[list]) items.push(item);
          if (page[list].length) after = string(page[list][page[list].length-1].key);
        } while (page.has_more);
      } catch (error) {
        // An older runtime has no paged operation: keep the original failure.
        if (error?.kind === 'InvalidRequest' && first === null) throw whole;
        // The cursor's entry settled between pages: restart from the first prefix.
        if (error?.kind !== 'Protocol' || after === null || attempt >= 2) throw error;
        continue;
      }
      if (list === 'entries') return items;
      const {has_more:_,...status} = first;return {...status,[list]:items};
    }
  }
  async preparePush(now = (this.options.clock || Date.now)(), jitter) { if(jitter===undefined)jitter=this.options.jitter?this.options.jitter():await this.runtime.jitter();const bounded=unsigned(jitter,16);check(Number(bounded)<=2500);return this.#deliverSyncMutation('prepare_push',{now_ms:integer(now),jitter_basis_points:bounded}); }
  async #deliverSyncMutation(op,args) {
    const owner=this.owner,generation=this.generation;const result=await this.call(op,args);
    // A due-queue check may commit observable state without waking its own pump.
    if(owner===this.owner&&generation===this.generation){
      if(op==='receive_wire'||op==='request_redelivery')this.wakeSync();
      if(this.callbacks.size)await this.#backgroundDrain();
    }
    return result;
  }
  receiveWire(body) { return this.#deliverSyncMutation('receive_wire',{body}); }
  requestRedelivery(key,now) { return this.#deliverSyncMutation('request_redelivery',{key,now_ms:integer(now)}); }
  pendingReconciliations() { return this.#paged('pending_reconciliations','entries'); }
  async setConflictPolicy(policy) { return this.call('set_conflict_policy',{policy:conflictPolicy(policy)}); }
  async reconcile(key,plan) { await this.reconcileResolving(key,plan);return null; }
  async reconcileGuarded(key,plan,guards) { await this.reconcileResolving(key,plan,guards);return null; }
  async rollback(key) { await this.rollbackResolving(key);return null; }
  reconcileResolving(key,plan,guards=[]) { return this.#resolve('reconcile_resolving',{key,plan,guards}); }
  rollbackResolving(key) { return this.#resolve('rollback_resolving',{key}); }
  discard(key) { return this.#resolve('discard',{key}); }
  async #resolve(op,args) {
    const owner=this.owner,generation=this.generation;
    const result=resolution(await this.call(op,args,owner));
    const live=()=>owner===this.owner&&generation===this.generation&&!this.#closing;
    if(live()&&result.kind==='kept_current'&&typeof this.options.onResolution==='function'){
      // Promise assimilation contains hostile thenables and reports at most once
      // per invocation. This callback stays installed after any failure.
      const failed=()=>{if(live())this.#report('callback','CallbackFailure',null,generation);};
      try{Promise.resolve(this.options.onResolution(args.key,result)).catch(failed);}catch(_){failed();}
    }
    if(live()){this.wakeSync();await this.#backgroundDrain();this.scheduleCount();}
    return result;
  }
  cancel(key) { return this.mutation('cancel',{key}); }
  forget(key) { return this.mutation('forget',{key}); }
  observationClock(){
    if(!this.#autoClock){
      let clock=this.options.monotonic;
      if(clock===undefined){const performance=globalThis.performance;check(performance&&typeof performance.now==='function');clock=()=>Math.floor(performance.now());}
      check(typeof clock==='function');const timers=this.options.timers??{set:(work,delay)=>globalThis.setTimeout(work,delay),clear:id=>globalThis.clearTimeout(id)};check(typeof timers.set==='function'&&typeof timers.clear==='function');
      unsigned(clock(),64);this.#autoClock=clock;this.#hostTimers=timers;
    }
    return unsigned(this.#autoClock(),64);
  }
  scheduleDrain(){
    if(!this.owner||this.callbacks.size===0)return;
    if(this.draining){this.#drainAgain=true;return;}
    if(this.#autoTimer!==null)return;
    const generation=this.generation;this.#autoTimer=this.#timerSet(()=>{this.#autoTimer=null;if(generation===this.generation&&this.owner)void this.#backgroundDrain();},0);
  }
  scheduleCount(){
    if(!this.owner||this.#countTokens.size===0||this.#countTimer!==null)return;
    const generation=this.generation;this.#countTimer=this.#timerSet(()=>{this.#countTimer=null;if(generation===this.generation&&this.owner)void this.#backgroundDrain();},16);
  }
  cancelObservationTimers(){
    if(this.#autoTimer!==null)this.#timerClear(this.#autoTimer);
    if(this.#countTimer!==null)this.#timerClear(this.#countTimer);
    this.#autoTimer=null;this.#countTimer=null;this.#drainAgain=false;this.#countTokens.clear();
  }
  async subscribe(op,args,callback) {
    check(typeof callback==='function');this.observationClock();
    const generation = this.generation, owner = this.owner;const token = await this.call(op,args);
    if(generation !== this.generation){
      // Registered after a reset of this same owner: retire the core subscription instead of leaking it.
      if(owner === this.owner)await this.call(op==='observe'?'unsubscribe':'unsubscribe_sync',{subscription:token}).catch(()=>{});
      throw failure('InvalidHandle');
    }
    this.callbacks.set(token,callback);
    if(op!=='observe'){this.syncSubscriptions??=new Set();this.syncSubscriptions.add(token);}
    else if(args.mode==='count')this.#countTokens.add(token);
    this.scheduleDrain();return token;
  }
  observeData(query, callback, mode = 'membership', columns = []) { return this.subscribe('observe',{query:queryWire(query),mode,columns},callback); }
  observeStatus(callback) { return this.subscribe('observe_status',{},callback); }
  observeOutcome(key,callback) { return this.subscribe('observe_outcome',{key},callback); }
  async unsubscribe(subscription) {
    this.callbacks.delete(subscription);this.#countTokens.delete(subscription);
    if(this.#countTokens.size===0&&this.#countTimer!==null){const timer=this.#countTimer;this.#countTimer=null;this.#timerClear(timer);}
    const op=this.syncSubscriptions?.delete(subscription)?'unsubscribe_sync':'unsubscribe';
    return this.call(op,{subscription});
  }
  async managedDrain(){if(this.draining){this.#drainAgain=true;return;}return this.drain();}
  // SPEC-0036: undeclared legacy diagnostics stay bounded; typed facts go to the sink.
  #remember(list,error){list.push(error);if(list.length>64)list.splice(0,list.length-64);}
  #report(source,error,token,generation){
    if(generation!==this.generation||!this.owner)return;
    const sink=this.options.onBackgroundError;if(typeof sink!=='function')return;
    // Only fixed public kinds leave the SDK; unknown kinds, including malformed wire kinds, become BackendFailure.
    const kind=error==='CallbackFailure'?error:typeof error==='string'?failure(error).kind:publicFailure(error).kind;
    try{const result=sink(Object.freeze({source,kind,token}));if(result&&typeof result.then==='function')result.then(undefined,()=>{});}
    catch(_){/* a sink failure, including a rejected returned thenable, is contained and never reported */}
  }
  // A host timer set/clear failure is one redacted timer fact; it never throws into delivery or a committed result.
  #timerSet(work,delay){const generation=this.generation;try{return this.#hostTimers.set(work,delay);}catch(error){this.#report('timer',error,null,generation);return null;}}
  #timerClear(handle){const generation=this.generation;try{this.#hostTimers.clear(handle);}catch(error){this.#report('timer',error,null,generation);}}
  #backgroundDrain(){
    const generation=this.generation;
    return this.managedDrain().catch(error=>{this.#remember(this.callbackErrors,error);this.#report('observation',error,null,generation);});
  }
  #retire(token){
    this.callbacks.delete(token);this.#countTokens.delete(token);this.syncSubscriptions?.delete(token);
    if(this.#countTokens.size===0&&this.#countTimer!==null){const timer=this.#countTimer;this.#countTimer=null;this.#timerClear(timer);}
  }
  // Callbacks run synchronously in event order and are never awaited. A throw or a later
  // rejection retires that listener once (C); its core unsubscribe failure is reported once (U).
  #invoke(token,callback,event,generation,terminated){
    const sync=this.syncSubscriptions?.has(token)===true;let failed=false;
    const fail=error=>{
      if(failed||generation!==this.generation||!this.owner)return;failed=true;
      if(!terminated){if(this.callbacks.get(token)!==callback)return;this.#retire(token);}
      this.#remember(this.callbackErrors,error);this.#report('callback','CallbackFailure',token,generation);
      if(!terminated)this.call(sync?'unsubscribe_sync':'unsubscribe',{subscription:token}).catch(failed=>this.#report('observation',failed,token,generation));
    };
    let result;
    try{result=callback(event);}catch(error){fail(error);return;}
    try{if(result&&typeof result.then==='function')result.then(undefined,fail);}catch(error){fail(error);}
  }
  async drain(now) {
    if(now===undefined)now=this.#autoClock?this.observationClock():'0';
    check(!this.draining,'Busy'); this.draining = true;
    const generation = this.generation; let delivered = 0;
    try {
      let more;
      do {
        const page = await this.call('drain',{now_ms:unsigned(now,64)});
        more = page.has_more;
        for (const event of page.events) {
          if (generation !== this.generation || !this.owner) break;
          const token = event.subscription, callback = this.callbacks.get(token);
          if (!callback) continue;
          // SPEC-0036: the generic error event is terminal; data error events stay recoverable.
          const terminated = event.kind === 'error';
          if (terminated) { this.#report('observation', event.value?.kind, token, generation); this.#retire(token); }
          this.#invoke(token, callback, copy(event), generation, terminated);
          delivered++;
        }
      } while (more && generation === this.generation && this.owner);
      return delivered;
    } finally { this.draining = false;if(this.#drainAgain){this.#drainAgain=false;this.scheduleDrain();} }
  }
  observe(query,callback,mode='membership',columns=[]){return this.observeData(query,callback,mode,columns);}
  refresh(){return this.mutation('refresh',{});}
  observationStats(){return this.call('observation_stats');}
  startSync(transport, reconcile = null) {this.transport = syncTransport(transport); this.plan = reconcile; this.wakeSync(); }
  stopSync() { this.transport = null; this.syncWakePending = false; clearTimeout(this.timer); this.timer = null; }
  wakeSync(delay = 0) {
    if (!this.transport || !this.owner) return;
    if (this.syncing || this.#jobs.length) { this.syncWakePending = true; return; }
    this.syncWakePending = false;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => { void this.sync(); },delay);
  }
  async resumeOwned(operation, owner, generation, cancellation) {
    if(owner!==this.owner||generation!==this.generation)return null;
    const invalidated=this.#invalidated??new Promise(()=>{});
    const stopRelease=this.#listenRelease(owner,generation);
    if(cancellation)void cancellation.then(stopRelease,stopRelease);
    try {
      while (owner === this.owner && generation === this.generation) {
        const epoch = releaseEpoch;
        try {
          const value=await operation();
          return owner===this.owner&&generation===this.generation?value:null;
        }
        catch (error) {
          if (error.kind !== 'Busy') throw error;
          // The same response/proposal is retained; no transport or mapper reruns.
          await waitRelease(epoch,cancellation?Promise.race([invalidated,cancellation]):invalidated);
        }
      }
      return null;
    } finally {stopRelease();}
  }
  async sync() {
    if (!this.transport || !this.owner) return;
    if (this.syncing || this.#jobs.length) { this.syncWakePending = true; return; }
    const owner = this.owner, generation = this.generation;
    const live = () => owner === this.owner && generation === this.generation;
    const run=Symbol('sync');this.syncRun=run;
    this.syncing = true; this.syncWakePending = false;
    let due = null;
    // SPEC-0039: keys whose core reconcile conflicted are not retried in this pass.
    const conflicted = new Set();
    try {
      let more;
      do {
        while (this.#jobs.length && live()) {
          const epoch = releaseEpoch;
          await waitRelease(epoch,new Promise(() => {}));
        }
        if (!live() || !this.transport) return;
        const batch = await this.resumeOwned(() => this.preparePush(),owner,generation);
        if (batch === null) return;
        due = batch.next_due_ms; more = batch.has_more;
        if (batch.envelopes.length) {
          const body = '[' + batch.envelopes.join(',') + ']';
          const response = await this.transport(body);
          if (!live()) return;
          const responseBody = string(response);
          const failures = new Map();
          const receipts = await this.resumeOwned(async () => {
            const entries = await this.receiveWire(responseBody);
            for (const entry of entries) if (!entry.ok && entry.error.kind !== 'Busy') failures.set(entry.key,entry.error.kind);
            if (entries.some(entry => !entry.ok && entry.error.kind === 'Busy')) throw failure('Busy');
            return entries;
          },owner,generation);
          if (receipts === null) return;
          for (const kind of failures.values()) { this.#remember(this.syncErrors,failure(kind)); this.#report('sync',kind,null,generation); }
          this.syncWakePending = true;
        }
        if (this.plan) {
          const pendingList=await this.pendingReconciliations();
          if (!live()) return;
          for (const pending of pendingList) {
            if (!live()) return;
            if (conflicted.has(pending.key)) continue;
            const plan = await this.plan(copy(pending));
            if (!live()) return;
            try { await this.resumeOwned(() => this.reconcile(pending.key,plan),owner,generation); }
            catch (error) {
              // A core RollbackConflict leaves the entry pending; later entries still reconcile.
              if (error?.kind !== 'RollbackConflict') throw error;
              conflicted.add(pending.key);this.#remember(this.syncErrors,error);this.#report('sync',error,null,generation);
            }
            if (!live()) return;
          }
        }
        await this.managedDrain();
      } while (more && live() && this.transport);
    } catch (error) { if(live()){this.#remember(this.syncErrors,error);this.#report('sync',error,null,generation);} }
    finally {
      if(this.syncRun===run) {
        this.syncRun=null;this.syncing = false;
        const pending = this.syncWakePending; this.syncWakePending = false;
        if (live() && this.transport) {
          if (pending) this.wakeSync();
          else if (due !== null) {
            // SPEC-0036: a failing clock sample is one sync fact; that timed wake is skipped.
            let delay = null;
            try { delay = timerDelay(due,(this.options.clock || Date.now)()); }
            catch (error) { this.#remember(this.syncErrors,error); this.#report('sync',error,null,generation); }
            if (delay !== null) this.wakeSync(Math.min(delay,2147483647));
          }
        }
      }
    }
  }
  async close() {
    if (this.#closing) return this.#closing;
    const attempt = this.#opening;
    if (!this.owner && !attempt) return;
    if (attempt) attempt.cancelled = true;
    let resolve, reject;
    const closing = new Promise((yes,no) => { resolve = yes; reject = no; });
    this.#closing = closing;
    // Start disposal synchronously after reserving it: active-owner generation
    // invalidation must still suppress the next callback in an extracted batch.
    this.#dispose(attempt).then(resolve,reject);
    try { await closing; }
    finally { if (this.#closing === closing) this.#closing = null; }
  }
  async #dispose(attempt) {
    this.stopSync();
    if (attempt) {
      try { await attempt.ready; } catch { /* failed configuration still owns its acquired handle */ }
      let owner;
      try { owner = await attempt.received; }
      catch {
        if (this.#opening === attempt) this.#opening = null;
        return;
      }
      // A failed close deliberately retains the private attempt and handle for
      // a later close retry. The key-bearing open RPC is never submitted again.
      await this.call('close',{},owner);
      if (this.#opening === attempt) this.#opening = null;
      return;
    }
    // Let an already-submitted finish report its actual committed result.
    const finishing = this.#jobs.find(job => job.finishing);
    if (finishing) await finishing.completion;
    const owner = this.owner;
    this.generation++; this.#invalidate?.();this.#retireReleaseSubscriptions();this.cancelObservationTimers(); this.callbacks.clear();this.syncSubscriptions?.clear();
    for (const job of this.#jobs) job.cancel();
    await this.call('close',{},owner); this.owner = null; clients.delete(this);
    this.syncRun=null;this.syncing=false;this.syncWakePending=false;released();
  }
}

function normalizeWrites(writes){check(Array.isArray(writes));return writes.map(write=>write.op==='insert'?{...copy(write),proposed_id:write.proposed_id??null}:copy(write));}
function conflictPolicy(policy){
  const value=copy(policy);check(value&&typeof value==='object'&&!Array.isArray(value));
  check(Object.keys(value).every(key=>key==='default'||key==='commands'));
  check(Object.hasOwn(value,'default')&&['stop','discard'].includes(value.default));
  const commands=Object.hasOwn(value,'commands')?value.commands:{};
  check(commands&&typeof commands==='object'&&!Array.isArray(commands));
  check(Object.values(commands).every(reaction=>['stop','discard'].includes(reaction)));
  return {default:value.default,commands};
}
function resolution(value){
  try{
    const report=copy(value);check(report&&typeof report==='object'&&!Array.isArray(report));
    const keys=Object.keys(report).sort();
    if(report.kind==='completed')check(keys.join(',')==='kind');
    else{
      check(report.kind==='kept_current'&&keys.join(',')==='discarded_keys,kind,skipped_writes,unmapped');
      for(const values of [report.discarded_keys,report.unmapped]){check(Array.isArray(values));for(const value of values)string(value);}
      check(typeof report.skipped_writes==='string');unsigned(report.skipped_writes,64);
    }
    return immutableContext(report);
  }catch(_){throw failure('InvalidPayload');}
}
export async function openDatabase(options){const db=new Database(options.runtime,options);await db.open(options);return db;}
