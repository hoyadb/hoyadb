// API-0008 / TC-0032 C20: experimental host dispatcher, not a packaged SDK.
const failure = kind => Object.assign(new Error(kind), {kind});
const check = (ok, kind = 'InvalidRequest') => { if (!ok) throw failure(kind); };
const copy = value => structuredClone(value);
const immutableContextError = () => { throw new TypeError('Trusted context is read-only'); };
function immutableContext(value) {
  if (value === null || typeof value !== 'object') return value;
  // Wrap children before freezing so descriptors cannot expose mutable targets.
  const target = Array.isArray(value) ? value.map(immutableContext) :
    Object.fromEntries(Object.entries(value).map(([key,item]) => [key,immutableContext(item)]));
  Object.freeze(target);
  // Frozen objects alone silently ignore assignments from non-strict handlers.
  return new Proxy(target, {set:immutableContextError,deleteProperty:immutableContextError,
    defineProperty:immutableContextError,setPrototypeOf:immutableContextError});
}
function integer(value) {
  if (typeof value === 'number') check(Number.isSafeInteger(value));
  check(['string','number','bigint'].includes(typeof value));
  const text = String(value);
  check(/^-?(0|[1-9][0-9]*)$/.test(text));
  const n = BigInt(text);
  check(n >= -9223372036854775808n && n <= 9223372036854775807n);
  return text;
}
function float(value) {
  check(typeof value === 'number' || typeof value === 'string');
  const text = String(value);
  check(/^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$/.test(text) && Number.isFinite(Number(text)));
  return text;
}
function outer(value) {
  check(value === null || ['string','boolean','object'].includes(typeof value));
  if (value && typeof value === 'object') {
    check(Array.isArray(value) || Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
    for (const item of Object.values(value)) outer(item);
  }
}
const string = value => { check(typeof value === 'string'); return value; };
export const j = {
  null: () => ({kind:'null'}),
  boolean: value => { check(typeof value === 'boolean'); return {kind:'boolean',value}; },
  string: value => ({kind:'string',value:string(value)}),
  integer: value => ({kind:'integer',value:integer(value)}),
  float: value => ({kind:'float',value:float(value)}),
  array: value => { check(Array.isArray(value)); outer(value); return {kind:'array',value:copy(value)}; },
  object: value => { check(value && !Array.isArray(value) && typeof value === 'object'); outer(value); return {kind:'object',value:copy(value)}; },
  reference: value => ({kind:'reference',value:string(value)}),
  timestamp: value => ({kind:'timestamp',value:integer(value)}),
};
export const query = table => ({table,predicates:[],order:null,limit:null});
const jobs = [];
const clients = new Set();
let pumping = false;
let releaseEpoch = 0;
const releaseWaiters = new Set();
function released() { releaseEpoch++; for (const wake of releaseWaiters) wake(); releaseWaiters.clear(); }
function waitRelease(epoch, cancellation) {
  if (epoch !== releaseEpoch) return Promise.resolve();
  let wake;
  const pending = new Promise(resolve => { wake = resolve; releaseWaiters.add(wake); });
  return Promise.race([pending,cancellation]).finally(() => releaseWaiters.delete(wake));
}

async function pump() {
  if (pumping) return;
  pumping = true;
  try {
    while (jobs.length) {
      const job = jobs[0];
      await job.client.run(job);
      jobs.shift();
    }
  } finally {
    pumping = false;
    released();
    for (const client of clients) client.wakeSync();
  }
}
export class CommandClient {
  #opening = null;
  #closing = null;
  constructor(send, options = {}) {
    this.send = send; this.options = options; this.owner = null; this.generation = 0;
    this.callbacks = new Map(); this.callbackErrors = []; this.draining = false;
    this.transport = null; this.timer = null; this.syncing = false; this.syncRun = null; this.syncWakePending = false; this.syncErrors = [];
  }
  async call(op, args = {}, owner = this.owner) {
    const frame = {op,owner,args}; outer(frame);
    const response = await this.send({op:'command_rpc',input:JSON.stringify(frame)});
    if (response.error) throw failure(response.error.kind);
    const decoded = JSON.parse(response.value);
    if (['finish','abort','cancel_request','close'].includes(op)) released();
    if (!decoded.ok) throw failure(decoded.error.kind);
    return decoded.value;
  }
  async open(path, options = {}) {
    const key = options?.encryptionKey;
    check(key instanceof Uint8Array && key.byteLength === 32);
    const key_hex = Array.from(key, byte => byte.toString(16).padStart(2,'0')).join('');
    return this.#open(path, options, 'open_secure', {key_hex});
  }
  openUnprotected(path, options = {}) {
    return this.#open(path, options, 'open');
  }
  async #open(path, options, op, protectedArgs = {}) {
    check(!this.owner && !this.#opening && !this.#closing, 'Busy');
    let resolve, reject;
    const received = new Promise((yes,no) => { resolve = yes; reject = no; });
    const attempt = {cancelled:false,received};
    // Reserve before invoking the caller's transport; even synchronous reentry
    // must see the same pending attempt. Retain no key/configuration on it.
    this.#opening = attempt;
    try {
      const bytes = crypto.getRandomValues(new Uint8Array(16));
      const namespace = Array.from(bytes,b => b.toString(16).padStart(2,'0')).join('');
      this.call(op, {path,namespace,options:{capacity:'1000',history:'100',initial_delay_ms:'1000',multiplier:'2',max_delay_ms:'60000',...options.options},version:options.version || '1',authored:options.authored || false,...protectedArgs},null).then(resolve,reject);
    } catch (error) { reject(error); }
    try {
      const owner = await received;
      check(!attempt.cancelled, 'Closed');
      this.owner = owner; this.generation++; clients.add(this); return this;
    } catch (error) {
      if (attempt.cancelled) throw failure('Closed');
      throw error;
    } finally {
      // A cancelled attempt belongs to close until its late handle is disposed.
      if (!attempt.cancelled && this.#opening === attempt) this.#opening = null;
    }
  }
  execute(definition, args, handler, options = {}) {
    try { outer(definition); outer(args); check(this.owner, 'InvalidHandle'); check(typeof handler === 'function'); }
    catch (error) { return Promise.reject(error); }
    const generation = this.generation, owner = this.owner;
    let identity, now;
    try { identity = this.options.context ? copy(this.options.context()) : null; outer(identity); now = integer((this.options.clock || Date.now)()); }
    catch (error) { return Promise.reject(error); }
    let resolve, reject, cancelled, completed;
    const result = new Promise((yes,no) => { resolve = yes; reject = no; });
    const cancellation = new Promise(yes => { cancelled = yes; });
    const completion = new Promise(yes => { completed = yes; });
    const job = {client:this,definition:copy(definition),args:copy(args),handler,identity,now,generation,owner,resolve,reject,cancellation,cancelled,completion,completed,dead:false,finishing:false};
    // The host enqueues synchronously, before allocating the core ticket.
    jobs.push(job);
    job.cancel = () => { if (!job.finishing) { job.dead = true; cancelled(); reject(failure('Cancelled')); } };
    if (options.signal) {
      job.signal = options.signal;
      job.signal.addEventListener('abort',job.cancel,{once:true});
      if (job.signal.aborted) job.cancel();
    }
    void pump(); return result;
  }
  async run(job) {
    let request, admitted = false;
    const live = () => !job.dead && job.generation === this.generation && job.owner === this.owner;
    const scoped = async (op,args = {}) => { check(live() && admitted && !job.finishing,'InvalidHandle'); return this.call(op,{request,...args},job.owner); };
    try {
      check(live(),'Cancelled');
      request = await this.call('request',{},job.owner);
      check(live(),'Cancelled');
      while (!admitted) {
        const epoch = releaseEpoch;
        admitted = await this.call('begin',{request,definition:job.definition,value:job.args,identity:job.identity,now_ms:job.now},job.owner);
        check(live(),'Cancelled');
        if (!admitted) await waitRelease(epoch,job.cancellation);
        check(live(),'Cancelled');
      }
      const scope = {
        args:await scoped('scope_args'),
        fetch:query => scoped('scope_fetch',{query}),
        insert:(table,fields) => scoped('scope_insert',{table,fields}),
        replace:(table,id,fields) => scoped('scope_replace',{table,id,fields}),
        remove:(table,id) => scoped('scope_remove',{table,id}),
        batch:writes => scoped('scope_batch',{writes}),
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
      if (job.generation === this.generation) await this.drain().catch(error => this.callbackErrors.push(error));
    } catch (error) { job.reject(error); }
    finally {
      if (request && job.owner === this.owner) {
        try { await this.call(admitted ? 'abort' : 'cancel_request',{request},job.owner); } catch (_) { /* close already disposed the ticket */ }
      }
      if (job.signal) job.signal.removeEventListener('abort',job.cancel);
      job.completed();
    }
  }
  fetch(query) { return this.call('fetch',{query}); }
  configureEntities(scope,tables) { return this.call('entity_configure',{scope,tables}); }
  capturedEntity() {
    check(typeof this.options.context === 'function','InvalidCommand');
    const identity=copy(this.options.context());outer(identity);
    return {identity,now_ms:integer((this.options.clock || Date.now)())};
  }
  async entityMutation(op,args) {
    const generation=this.generation;
    const result=await this.call(op,{...args,...this.capturedEntity()});
    if(generation===this.generation) {
      this.wakeSync();
      await this.drain().catch(error=>this.callbackErrors.push(error));
    }
    return result;
  }
  entityBatch(writes) { return this.entityMutation('entity_batch',{writes}); }
  patch(table,id,fields) { return this.entityMutation('entity_patch',{table,id,fields}); }
  syncProgress() { return this.call('sync_progress'); }
  observeProgress(callback) { return this.subscribe('observe_progress',{},callback); }
  async pullOnce(transport) {
    check(typeof transport==='function');
    const owner=this.owner,generation=this.generation;
    const live=()=>owner===this.owner && generation===this.generation;
    const batch=await this.call('begin_pull',{},owner);
    let consumed=false;
    try {
      const body=string(await transport(batch.since,batch.scope));check(live(),'Closed');
      const bytes=new TextEncoder().encode(body);check(bytes.length<=1048576,'LimitExceeded');
      const decoder=new TextDecoder('utf-8',{fatal:true});
      for(let offset=0;offset<bytes.length;) {
        check(live(),'Closed');
        let end=Math.min(offset+262144,bytes.length),args;
        while(true) {
          while(end<bytes.length && (bytes[end]&0xc0)===0x80) end--;
          check(end>offset,'LimitExceeded');
          args={request:batch.request,offset:String(offset),chunk:decoder.decode(bytes.subarray(offset,end))};
          if(new TextEncoder().encode(JSON.stringify({op:'pull_chunk',owner,args})).length<=1048576) break;
          end=offset+Math.floor((end-offset)/2);
        }
        await this.call('pull_chunk',args,owner);offset=end;
      }
      check(live(),'Closed');
      await this.resumeOwned(()=>this.call('apply_pull',{request:batch.request},owner),owner,generation);
      check(live(),'Closed');consumed=true;
      await this.drain().catch(error=>this.callbackErrors.push(error));
      return this.syncProgress();
    } finally {
      if(!consumed && live()) try {await this.call('abort_pull',{request:batch.request},owner);} catch(_) { /* StalePull already retired it. */ }
    }
  }
  async reset(mode = 'preserve') {
    check(jobs.length===0,'Busy');check(['preserve','discard'].includes(mode));
    await this.call('reset',{mode});
    this.generation++;this.callbacks.clear();this.stopSync();
    this.syncRun=null;this.syncing=false;released();
  }
  queueStatus() { return this.call('queue_status'); }
  preparePush(now = (this.options.clock || Date.now)(), jitter = 0) { return this.call('prepare_push',{now_ms:integer(now),jitter_basis_points:integer(jitter)}); }
  receiveWire(body) { return this.call('receive_wire',{body}); }
  pendingReconciliations() { return this.call('pending_reconciliations'); }
  reconcile(key,plan) { return this.call('reconcile',{key,plan}); }
  rollback(key) { return this.call('rollback',{key}); }
  cancel(key) { return this.call('cancel',{key}); }
  forget(key) { return this.call('forget',{key}); }
  async subscribe(op,args,callback) {
    const generation = this.generation;
    const token = await this.call(op,args);
    check(generation === this.generation,'InvalidHandle');
    this.callbacks.set(token,callback); return token;
  }
  observeData(query, callback, mode = 'rows', columns = []) { return this.subscribe('observe_data',{query,mode,columns},callback); }
  observeStatus(callback) { return this.subscribe('observe_status',{},callback); }
  observeOutcome(key,callback) { return this.subscribe('observe_outcome',{key},callback); }
  async unsubscribe(subscription) {
    this.callbacks.delete(subscription);
    return this.call('unsubscribe',{subscription});
  }
  async drain(now = Date.now()) {
    check(!this.draining,'Busy'); this.draining = true;
    const generation = this.generation; let delivered = 0;
    try {
      let more;
      do {
        const page = await this.call('drain',{now_ms:integer(now)});
        more = page.has_more;
        for (const event of page.events) {
          if (generation !== this.generation || !this.owner) break;
          const callback = this.callbacks.get(event.subscription);
          if (!callback) continue;
          try { await callback(copy(event)); } catch (error) { this.callbackErrors.push(error); }
          delivered++;
        }
      } while (more && generation === this.generation && this.owner);
      return delivered;
    } finally { this.draining = false; }
  }
  startSync(transport, reconcile = null) { this.transport = transport; this.plan = reconcile; this.wakeSync(); }
  stopSync() { this.transport = null; this.syncWakePending = false; clearTimeout(this.timer); this.timer = null; }
  wakeSync(delay = 0) {
    if (!this.transport || !this.owner) return;
    if (this.syncing || jobs.length) { this.syncWakePending = true; return; }
    this.syncWakePending = false;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => { void this.sync(); },delay);
  }
  async resumeOwned(operation, owner, generation) {
    while (owner === this.owner && generation === this.generation) {
      const epoch = releaseEpoch;
      try { return await operation(); }
      catch (error) {
        if (error.kind !== 'Busy') throw error;
        // The response/plan lives in the operation closure. No network call or
        // application mapper is repeated while an admitted handler owns core.
        await waitRelease(epoch,new Promise(() => {}));
      }
    }
    return null;
  }
  async sync() {
    if (!this.transport || !this.owner) return;
    if (this.syncing || jobs.length) { this.syncWakePending = true; return; }
    const owner = this.owner, generation = this.generation;
    const live = () => owner === this.owner && generation === this.generation;
    const run=Symbol('sync');this.syncRun=run;
    this.syncing = true; this.syncWakePending = false;
    let due = null;
    try {
      let more;
      do {
        while (jobs.length && live()) {
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
          for (const kind of failures.values()) this.syncErrors.push(failure(kind));
          this.syncWakePending = true;
        }
        if (this.plan) for (const pending of await this.pendingReconciliations()) {
          const plan = await this.plan(copy(pending));
          if (!live()) return;
          await this.resumeOwned(() => this.reconcile(pending.key,plan),owner,generation);
          if (!live()) return;
        }
        await this.drain();
      } while (more && live() && this.transport);
    } catch (error) { this.syncErrors.push(error); }
    finally {
      if(this.syncRun===run) {
        this.syncRun=null;this.syncing = false;
        const pending = this.syncWakePending; this.syncWakePending = false;
        if (live() && this.transport) {
          const now = BigInt(integer((this.options.clock || Date.now)()));
          const delay = due === null ? null : Number(BigInt(due) > now ? BigInt(due) - now : 1n);
          if (pending) this.wakeSync();
          else if (delay !== null) this.wakeSync(Math.min(delay,2147483647));
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
    const finishing = jobs.find(job => job.client === this && job.finishing);
    if (finishing) await finishing.completion;
    const owner = this.owner;
    this.generation++; this.callbacks.clear();
    for (const job of jobs) if (job.client === this) job.cancel();
    await this.call('close',{},owner); this.owner = null; clients.delete(this);
    this.syncRun=null;this.syncing=false;this.syncWakePending=false;released();
  }
}
