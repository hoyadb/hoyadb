// API-0012 / SPEC-0042: application values over the API-0011 owner. No database algorithms live here.
import {openDatabase as openHost,defineSchema as defineHostSchema} from './index.js';
import {Query} from './query.js';
import {timerDelay} from './time.js';
import {check,failure,publicFailure} from './errors.js';
import {j,copyOwned,string,unsigned} from './values.js';
import {table,column,relation,defineApplicationSchema,applicationSchema,isApplicationSchema,hasApplicationTables,encodeFields,decodeRow,resolveColumn,encodeValue,encodeJson,decodeJson,milliseconds,readonly} from './application-values.js';
export {table,column,relation,applicationSchema,isApplicationSchema,encodeJson,decodeJson};
export const defineSchema=input=>hasApplicationTables(input)?defineApplicationSchema(input):defineHostSchema(input);
const definitions=new WeakMap(),records=new WeakMap();
const operators={'=':'Eq','!=':'Ne','<':'Lt','<=':'Le','>':'Gt','>=':'Ge',in:'In',Eq:'Eq',Ne:'Ne',Lt:'Lt',Le:'Le',Gt:'Gt',Ge:'Ge',In:'In'};
const own=(object,key,value)=>Object.defineProperty(object,key,{value,enumerable:true,configurable:true,writable:true});
// Disposal settles once: a failed registration owns no token, and an owner that closed has already
// dropped the subscription (false). Any other unsubscribe failure is returned to the caller.
const dispose=(raw,ready,token)=>ready.then(()=>token()===null?undefined:raw.unsubscribe(token()).catch(error=>{if(error?.kind==='Closed')return false;throw error;}),()=>undefined);

export function defineCommand(name,input){
 string(name);check(input&&[Object.prototype,null].includes(Object.getPrototypeOf(input))&&!Object.getOwnPropertySymbols(input).length,'InvalidCommand');
 for(const key of Object.keys(input)){check(['args','handler','referencePaths'].includes(key)&&Object.hasOwn(Object.getOwnPropertyDescriptor(input,key),'value'),'InvalidCommand');}
 check(typeof input.handler==='function','InvalidCommand');
 let argsInput,pathsInput;try{argsInput=readonly(input.args??{});pathsInput=readonly(input.referencePaths??[]);}catch{throw failure('InvalidCommand');}
 check(argsInput&&!Array.isArray(argsInput)&&typeof argsInput==='object','InvalidCommand');
 const arguments_=Object.fromEntries(Object.entries(argsInput).map(([key,value])=>{
  const descriptor=typeof value==='string'?{type:value}:value;
  const kinds={any:'Any',string:'String',reference:'String',integer:'Integer',float:'Float',boolean:'Boolean',array:'Array',object:'Object'};
  check(descriptor&&Object.keys(descriptor).every(key=>['type','optional'].includes(key))&&Object.hasOwn(kinds,descriptor.type)&&[true,false,undefined].includes(descriptor.optional),'InvalidCommand');
  return [key,{kind:kinds[descriptor.type],optional:descriptor.optional??false}];
 }));
 const paths=pathsInput.map(path=>{check(Array.isArray(path)&&path.length>0,'InvalidCommand');return path.map(part=>typeof part==='string'?{key:string(part)}:{index:unsigned(part)});});
 for(const [key,value]of Object.entries(argsInput))if((typeof value==='string'?value:value.type)==='reference')paths.push([{key}]);
 const definition=Object.freeze({name});definitions.set(definition,{wire:copyOwned({name,arguments:arguments_,reference_paths:paths}),handler:input.handler});return definition;
}

export class ApplicationQuery {
 constructor(view,table,wire=null){resolveColumn(view.schemaValue,table,'id');this.view=view;this.tableName=table;this.wire=wire??new Query(table);}
 where(field,operator,value){const op=Object.hasOwn(operators,operator)?operators[operator]:null;check(op,'InvalidQuery');const descriptor=resolveColumn(this.view.schemaValue,this.tableName,field);
  if(op==='In'){check(Array.isArray(value),'InvalidQuery');return new ApplicationQuery(this.view,this.tableName,this.wire.whereIn(field,value.map(v=>encodeValue(descriptor,v))));}
  return new ApplicationQuery(this.view,this.tableName,this.wire.where(field,op,encodeValue(descriptor,value)));
 }
 whereIn(field,values){return this.where(field,'in',values);}
 orderBy(field,direction='asc'){resolveColumn(this.view.schemaValue,this.tableName,field);const value={asc:'Asc',desc:'Desc',Asc:'Asc',Desc:'Desc'}[direction];check(value,'InvalidQuery');return new ApplicationQuery(this.view,this.tableName,this.wire.orderBy(field,value));}
 limit(count){return new ApplicationQuery(this.view,this.tableName,this.wire.limit(count));}
 async fetch(){return (await this.view.source.fetch(this.wire)).map(row=>this.view.row(this.tableName,row));}
 observe(callback,onError){check(this.view instanceof ApplicationDatabase,'UnsupportedOperation');return this.view.observeQuery(this,callback,onError);}
 observeCount(callback,onError){check(this.view instanceof ApplicationDatabase,'UnsupportedOperation');return this.view.observeQuery(this,callback,onError,true);}
}
class View {
 constructor(schema,source){this.schemaValue=schema;this.source=source;}
 query(table){return new ApplicationQuery(this,table);}
 related(table,id,name){const relations=this.schemaValue.tables[table]?.relations;check(relations&&Object.hasOwn(relations,name),'InvalidQuery');const descriptor=relations[name];return new ApplicationQuery(this,descriptor.table,new Query(descriptor.table,{table,id,relation:name}));}
 row(table,wire){const result=decodeRow(this.schemaValue,table,wire);records.set(result,{table,wire:copyOwned(wire)});
  for(const name of Object.keys(this.schemaValue.tables[table].relations))if(!Object.hasOwn(result,name))Object.defineProperty(result,name,{enumerable:false,get:()=>this.related(table,wire.id,name)});
  return result;
 }
 table(name){resolveColumn(this.schemaValue,name,'id');const view=this;
  return Object.freeze({
   async insert(fields){const encoded=encodeFields(view.schemaValue,name,fields);return view.row(name,await view.insertRow(name,encoded));},
   findById(id){string(id);return Object.freeze({
    async fetch(){return (await view.query(name).where('id','=',id).fetch())[0]??null;},
    async update(fields){const encoded=encodeFields(view.schemaValue,name,fields);if(view instanceof ApplicationDatabase){const row=await view.source.patch(name,id,encoded);view.wakeSync();return view.row(name,row);}
     const current=(await view.source.fetch(new Query(name).where('id','Eq',j.string(id))))[0];check(current,'RowNotFound');return view.row(name,await view.source.replace(name,id,{...current.fields,...encoded}));},
    remove(){return view.removeRow(name,id);}
   });},
   link:(id,relation,target,fields={})=>view.linkRow(name,id,relation,target,fields),
   unlink:async(id,relation,target)=>{const result=await view.unlinkRow(name,id,relation,target);if(view instanceof ApplicationDatabase)view.wakeSync();return result;}
  });
 }
 insertRow(table,fields){return this.source.insert(table,fields);}
 removeRow(table,id){return this.source.remove(table,id);}
 linkJoin(through,table,id,name,target,fields){return this.source.link(table,id,name,target,fields);}
 unlinkRow(table,id,name,target){return this.source.unlink(table,id,name,target);}
 async linkRow(table,id,name,target,fields){const relations=this.schemaValue.tables[table].relations;check(Object.hasOwn(relations,name),'InvalidQuery');const through=relations[name].through;check(through,'InvalidQuery');const row=this.row(through,await this.linkJoin(through,table,id,name,target,encodeFields(this.schemaValue,through,fields)));if(this instanceof ApplicationDatabase)this.wakeSync();return row;}
}
function resultJson(input){
 const ancestors=new Set();let nodes=0;
 function visit(value,depth){
  check(++nodes<=100000&&depth<64,'LimitExceeded');
  const record=records.get(value);
  if(record)return j.object(Object.fromEntries([['id',j.string(record.wire.id)],...Object.entries(record.wire.fields).map(([key,cell])=>[key,cell.kind==='reference'?j.string(cell.value):cell.kind==='timestamp'?j.integer(cell.value):cell])]));
  if(value===null||typeof value!=='object')return encodeJson(value);
  check(!ancestors.has(value)&&Object.getOwnPropertySymbols(value).length===0,'InvalidPayload');
  const array=Array.isArray(value);check(array||Object.getPrototypeOf(value)===Object.prototype||Object.getPrototypeOf(value)===null,'InvalidPayload');
  ancestors.add(value);const values=[];let index=0;
  for(const key of Object.keys(value)){
   if(array)check(key===String(index++),'InvalidPayload');
   const descriptor=Object.getOwnPropertyDescriptor(value,key);check(Object.hasOwn(descriptor,'value'),'InvalidPayload');
   values.push([key,visit(descriptor.value,depth+1)]);
  }
  if(array)check(index===value.length,'InvalidPayload');ancestors.delete(value);
  return array?j.array(values.map(([,item])=>item)):j.object(Object.fromEntries(values));
 }
 return visit(input,0);
}
function returned(value,view){
 const record=records.get(value);if(record)return view.row(record.table,record.wire);
 if(Array.isArray(value))return value.map(v=>returned(v,view));
 if(value&&typeof value==='object'){const result={};for(const [key,item]of Object.entries(value))own(result,key,returned(item,view));return result;}
 return value;
}
export class ApplicationDatabase extends View {
 constructor(schema,raw,options){super(schema,raw);this.raw=raw;this.options=options;this.transport=options.transport??null;this.mapper=options.reconcile??null;this.entities=options.entities??null;this.epoch=0;this.running=false;this.pullRun=null;this.pullTimer=null;this.conflicted=new Set();}
 async insertRow(table,fields){const changes=await this.raw.entityBatch([{op:'insert',table,fields,proposed_id:this.entities?.tables.includes(table)?null:this.raw.proposedId()}]);this.wakeSync();return changes[0].after;}
 async removeRow(table,id){const changes=await this.raw.entityBatch([{op:'remove',table,id}]);this.wakeSync();return changes.length>0;}
 // API-0011 §4: a configured entity through table is written only through the entity entry
 // points (captured context, core-allocated join ID); core alone decides the join semantics.
 entityThrough(through){return typeof through==='string'&&this.entities?.tables.includes(through)===true;}
 linkJoin(through,table,id,name,target,fields){return this.entityThrough(through)?this.raw.entityLink(table,id,name,target,fields):this.raw.link(table,id,name,target,fields);}
 unlinkRow(table,id,name,target){const relations=this.schemaValue.tables[table]?.relations,through=relations&&Object.hasOwn(relations,name)?relations[name].through:undefined;return this.entityThrough(through)?this.raw.entityUnlink(table,id,name,target):this.raw.unlink(table,id,name,target);}
 command(definition){const stored=definitions.get(definition);check(stored,'InvalidCommand');const run=async(args,options)=>{
  const input=encodeJson(args);let result;
  const receipt=await this.raw.execute(stored.wire,input,async scope=>{
   const view=new View(this.schemaValue,scope);const now=Number(scope.context.now_ms);check(Number.isSafeInteger(now),'InvalidPayload');
   const context=Object.freeze({identity:readonly(decodeJson(scope.context.identity)),now:()=>new Date(now)});
   result=await stored.handler(view,readonly(decodeJson(scope.args)),context);const encoded=resultJson(result);result=returned(result,this);return encoded;
  },options);
  this.wakeSync();return {key:receipt.key,result:returned(result,this)};
 };return Object.freeze({execute:async(args,options)=>(await run(args,options)).result,executeWithReceipt:run});}
 observeQuery(query,callback,onError,count=false){check(typeof callback==='function');let active=true,token=null,disposal=null;
  const stop=()=>{active=false;return disposal??=dispose(this.raw,ready,()=>token);};
  const ready=this.raw.observeData(query.wire,event=>{
   if(!active)return;
   if(event.kind==='error'||event.value?.kind==='error'){const terminal=event.kind==='error',error=failure(terminal?event.value.kind:event.value.error.kind);if(onError)return onError(error);if(terminal)throw error;
    // A recoverable data error keeps its listener (API-0011 §7); without an error callback the background sink reports it.
    this.report('observation',error);return;}
   if(event.kind==='data')return callback(count?Number(event.value.count):event.value.rows.map(row=>this.row(query.tableName,row)));
  },count?'count':'columns',count?[]:[...Object.keys(this.schemaValue.tables[query.tableName].columns),...Object.values(this.schemaValue.tables[query.tableName].relations).filter(r=>r.kind==='ToOne').map(r=>r.foreign_key)]).then(async value=>{token=value;if(!active){await this.raw.unsubscribe(value);token=null;}});
  Object.defineProperty(stop,'ready',{value:ready});void ready.catch(error=>{if(active&&onError){try{onError(error);}catch{} }});return stop;
 }
 observeOutcome(key,callback){return this.subscription('observeOutcome',[key],callback,event=>({...event.value,...(event.value?.result?{result:decodeJson(event.value.result)}:{})}));}
 observeStatus(callback){return this.subscription('observeStatus',[],callback,event=>event.value);}
 observeProgress(callback){return this.subscription('observeProgress',[],callback,event=>event.value);}
 subscription(method,args,callback,convert){let active=true,token=null;const ready=this.raw[method](...args,event=>{
  // SPEC-0036: the terminal error event is never a value. The owner has already retired the listener and reported it.
  if(active&&event.kind!=='error')return callback(convert(event));}).then(async value=>{token=value;if(!active){await this.raw.unsubscribe(value);token=null;}});let disposal;const stop=()=>{active=false;return disposal??=dispose(this.raw,ready,()=>token);};Object.defineProperty(stop,'ready',{value:ready});void ready.catch(()=>{});return stop;}
 queueStatus(){return this.raw.queueStatus();}
 syncProgress(){return this.raw.syncProgress();}
 async configureEntities(scope,tables){await this.raw.configureEntities(scope,tables);this.entities={scope,tables:[...tables]};}
 async configureEntitiesDurable(scope,tables){await this.raw.configureEntitiesDurable(scope,tables);this.entities={scope,tables:[...tables]};}
 resolveEntityId(table,id){return this.raw.resolveEntityId(table,id);}
 async reconfigureEntities(scope,tables){await this.raw.reconfigureEntities(scope,tables);this.entities={scope,tables:[...tables]};}
 async preparePush(now,jitter){const batch=await this.raw.preparePush(now===undefined?undefined:milliseconds(now),jitter);return {...batch,envelopes:await Promise.all(batch.envelopes.map(async body=>decodeJson(await this.raw.runtime.decodeJson(body))))};}
 async receive(outcomes){const result=await this.raw.receiveWire(await this.raw.runtime.encodeJson(encodeJson(outcomes)));this.wakeSync();return result;}
 async pending(p){return {...p,...await this.raw.call('reconciliation_context',{key:p.key}),outcome:{...p.outcome,...(p.outcome.result?{result:decodeJson(p.outcome.result)}:{})}};}
 async pendingReconciliations(){
  const owner=this.raw.owner,generation=this.raw.generation,live=()=>check(owner&&owner===this.raw.owner&&generation===this.raw.generation,'Closed');
  const listed=await this.raw.pendingReconciliations();live();
  const pending=await Promise.all(listed.map(async p=>{
   try{const result=await this.pending(p);live();return result;}catch(error){
    // Context is advisory and a listed key can settle before this second read.
    // Omit only a confirmed retired key; preserve genuine context/lifecycle errors.
    live();if(error.kind!=='Protocol')throw error;
    const current=await this.raw.pendingReconciliations();live();if(current.some(entry=>entry.key===p.key))throw error;
    return null;
   }
  }));live();return pending.filter(p=>p!==null);
 }
 plan(plan){check(plan&&Array.isArray(plan.writes),'InvalidPayload');return {mappings:copyOwned(plan.mappings),writes:plan.writes.map(write=>write.op==='put'?{...copyOwned({...write,fields:{}}),fields:encodeFields(this.schemaValue,write.table,write.fields)}:write.op==='entity'?{op:'entity',change:this.change(write.change)}:copyOwned(write))};}
 filledPlan(context,plan){const mappings=context.kind==='entity'?{...Object.fromEntries(Object.keys(context.created).map(id=>[id,id])),...copyOwned(plan.mappings)}:plan.mappings;return this.plan({...plan,mappings});}
 reconcile(key,plan){return this.#reconcile(key,plan,false);}
 reconcileResolving(key,plan){return this.#reconcile(key,plan,true);}
 async #reconcile(key,plan,resolving){
  let context;try{context=await this.raw.call('reconciliation_context',{key});}catch(error){if(error.kind!=='Protocol')throw error;}
  // Context is advisory: state can change across the await; the core validates the final plan.
  const encoded=context?this.filledPlan(context,plan):this.plan(plan);
  const result=await (resolving?this.raw.reconcileResolving(key,encoded):this.raw.reconcile(key,encoded));this.conflicted.delete(key);this.wakeSync();return result;
 }
 async rollback(key){try{return await this.raw.rollback(key);}finally{this.conflicted.delete(key);this.wakeSync();}}
 async rollbackResolving(key){try{return await this.raw.rollbackResolving(key);}finally{this.conflicted.delete(key);this.wakeSync();}}
 async discard(key){const result=await this.raw.discard(key);this.conflicted.delete(key);this.wakeSync();return result;}
 setConflictPolicy(policy){return this.raw.setConflictPolicy(policy);}
 async cancel(key){try{return await this.raw.cancel(key);}finally{this.conflicted.delete(key);this.wakeSync();}}
 async forget(key){try{return await this.raw.forget(key);}finally{this.conflicted.delete(key);this.wakeSync();}}
 change(change){return {...copyOwned({...change,changes:{}}),changes:encodeFields(this.schemaValue,change.table,change.changes)};}
 adapted(transport){
  const epoch=this.epoch,push=typeof transport==='function'?transport:transport?.push;check(typeof push==='function');
  return async body=>{
   const response=await push.call(transport,decodeJson(await this.raw.runtime.decodeJson(body)));
   check(epoch===this.epoch&&this.raw.owner,'Closed');
   return this.raw.runtime.encodeJson(encodeJson(response));
  };
 }
 async pushPass(transport,mapper,epoch,cancelled,state={}){
  const owner=this.raw.owner,generation=this.raw.generation;
  const live=()=>{check(epoch===this.epoch&&owner&&owner===this.raw.owner&&generation===this.raw.generation,'Closed');};
  const owned=operation=>this.raw.resumeOwned(()=>{live();return operation();},owner,generation,cancelled);let more,due=null;
  do{
   live();const batch=await owned(()=>this.raw.preparePush());live();more=batch.has_more;due=batch.next_due_ms;state.due=due;
   if(batch.envelopes.length){
    const response=await Promise.race([this.adapted(transport)('['+batch.envelopes.join(',')+']'),cancelled]);live();
    const failures=new Map();await owned(async()=>{
     const entries=await this.raw.receiveWire(response);
     for(const entry of entries)if(!entry.ok&&entry.error.kind!=='Busy')failures.set(entry.key,entry.error.kind);
     if(entries.some(entry=>!entry.ok&&entry.error.kind==='Busy'))throw failure('Busy');
    });live();if(failures.size)throw failure(failures.values().next().value);
   }
   if(mapper){const pending=await owned(()=>this.pendingReconciliations());live();for(const item of pending){
    const key=item.key,context=copyOwned({kind:item.kind,created:item.created});
    if(state.background&&this.conflicted.has(key))continue;const plan=await Promise.race([Promise.resolve().then(()=>mapper(item)),cancelled]);live();
    // Capture application values once; only admission of this exact proposal retries.
    const proposal=this.filledPlan(context,plan);
    try{await owned(()=>this.raw.reconcile(key,proposal));live();this.conflicted.delete(key);this.wakeSync();}catch(error){if(error.kind!=='RollbackConflict'||!state.background)throw error;if(!this.conflicted.has(key)){this.conflicted.add(key);this.report('sync',error);}continue;}live();
   }}
  }while(more);
  return due;
 }
 async sync(){
  check(this.transport,'InvalidRequest');check(!this.explicitSync&&!this.pushRun,'Busy');this.explicitSync=true;
  const epoch=this.epoch;let cancel;const cancelled=new Promise((_,reject)=>{cancel=()=>reject(failure('Closed'));});void cancelled.catch(()=>{});this.explicitCancel=cancel;
  const run=(async()=>{await this.pushPass(this.transport,this.mapper,epoch,cancelled);check(epoch===this.epoch&&this.raw.owner,'Closed');await this.pull();})();this.explicitRun=run;
  try{await run;}
  finally{if(this.explicitRun===run){this.explicitRun=null;this.explicitCancel=null;this.explicitSync=false;}if(this.running)this.wakeSync();}
 }
 wakeSync(delay=0){
  if(!this.running||!this.raw.owner)return;if(this.pushRun||this.explicitSync){this.pushAgain=true;return;}
  clearTimeout(this.pushTimer);this.pushTimer=setTimeout(()=>{this.pushTimer=null;void this.backgroundPush();},delay);
 }
 async backgroundPush(){
  if(!this.running||!this.raw.owner)return;if(this.pushRun||this.explicitSync){this.wakeSync(10);return;}
  const epoch=this.epoch;let cancel;const cancelled=new Promise((_,reject)=>{cancel=()=>reject(failure('Closed'));});void cancelled.catch(()=>{});
  this.pushCancel=cancel;this.pushAgain=false;const state={due:null,background:true};let busy=false;
  const run=this.pushPass(this.transport,this.mapper,epoch,cancelled,state);this.pushRun=run;
  try{await run;}catch(error){if(this.running&&epoch===this.epoch){if(error.kind==='Busy')busy=true;else this.report('sync',error);}}
  finally{
   if(this.pushRun===run){this.pushRun=null;this.pushCancel=null;}
   if(this.running&&epoch===this.epoch){
    if(this.pushAgain||busy)this.wakeSync(busy?10:0);
    else if(state.due!==null){try{this.wakeSync(timerDelay(state.due,milliseconds((this.options.clock??Date.now)())));}catch(error){this.report('sync',error);}}
   }else if(this.running)this.wakeSync();
  }
 }
 pull(){if(!this.transport?.pull||!this.entities?.tables.length)return Promise.resolve();if(this.pullRun)return this.pullRun;
  const epoch=this.epoch,transport=this.transport;const live=()=>epoch===this.epoch&&this.raw.owner;
  let cancel;const cancelled=new Promise((_,reject)=>{cancel=()=>reject(failure('Closed'));});this.pullCancel=cancel;void cancelled.catch(()=>{});
  const run=this.raw.pullOnce(async(since,scope)=>{
   check(live(),'Closed');const page=await Promise.race([Promise.resolve().then(()=>transport.pull(since,scope)),cancelled]);check(live(),'Closed');check(page&&Array.isArray(page.changes),'InvalidPayload');
   return this.raw.runtime.encodeJson(encodeJson({...page,changes:page.changes.map(c=>this.change(c))}));
  }).finally(()=>{if(this.pullRun===run){this.pullRun=null;this.pullCancel=null;}});this.pullRun=run;return run;
 }
 report(source,error){const sink=this.options.onBackgroundError;if(typeof sink!=='function')return;try{const value=sink(Object.freeze({source,kind:publicFailure(error).kind,token:null}));if(value&&typeof value.then==='function')value.catch(()=>{});}catch{}}
 startSync(transport=this.transport,reconcile=this.mapper,options={}){
  const interval=options.pullIntervalMs??this.options.pullIntervalMs??30000;check(Number.isSafeInteger(interval)&&interval>0);check(typeof transport==='function'||typeof transport?.push==='function');
  this.stopSync();this.transport=transport;this.mapper=reconcile;this.running=true;this.wakeSync();const epoch=this.epoch;
  const tick=async()=>{if(!this.running||epoch!==this.epoch)return;try{await this.pull();}catch(error){if(this.running&&epoch===this.epoch&&error.kind!=='Busy')this.report('sync',error);}if(this.running&&epoch===this.epoch)this.pullTimer=setTimeout(tick,interval);};void tick();
 }
 stopSync(){this.running=false;this.epoch++;this.conflicted.clear();this.pullCancel?.();this.pushCancel?.();this.explicitCancel?.();clearTimeout(this.pullTimer);clearTimeout(this.pushTimer);this.pullTimer=null;this.pushTimer=null;}
 async reset(mode){const pending=[this.pullRun,this.pushRun,this.explicitRun].filter(Boolean);this.stopSync();await Promise.allSettled(pending);await this.raw.reset(mode);}
 async close(){const pending=[this.pullRun,this.pushRun,this.explicitRun].filter(Boolean);this.stopSync();await Promise.allSettled(pending);await this.raw.close();}
}
export async function openApplicationDatabase(options){
 const schema=isApplicationSchema(options.schema)?options.schema:applicationSchema(options.schema);
 const engine=options.engine==='memory'||options.engine===undefined&&!options.filename?{kind:'memory',store:null}:options.filename?{kind:'opfs',name:options.filename}:options.engine;
 if(engine.kind!=='memory')check(options.encryptionKey instanceof Uint8Array&&options.encryptionKey.byteLength===32);
 const runtime=options.runtime??await options.createRuntime?.();check(runtime,'InvalidRequest');
 const host={...options,runtime,schema,engine,transport:undefined,clock:()=>milliseconds((options.clock??Date.now)()),context:()=>encodeJson((options.context??(()=>null))())};
 const raw=await openHost(host);const db=new ApplicationDatabase(schema,raw,options);
 try{if(options.entities)await db.configureEntities(options.entities.scope,options.entities.tables);return db;}catch(error){await raw.close();throw error;}
}
