import {Database,j,copyOwned} from '@hoyadb/client/host';
import {encodeJson,decodeJson} from '@hoyadb/client/application';
const fail=kind=>Object.assign(new Error(kind),{kind});
const check=(condition,kind='InvalidRequest')=>{if(!condition)throw fail(kind);};
// Runtime must already be bootstrapped. No asset loading or entropy fallback.
export async function openMemoryDatabase(options){
 check(options?.runtime&&typeof options.namespace==='string'&&/^[0-9a-f]{32}$/.test(options.namespace));
 check(options.encryptionKey===undefined);
 const db=new Database(options.runtime,options);
 await db.open({...options,engine:{kind:'memory',store:options.store??null}});return db;
}
export function createClock(initial='0'){let now=initial;return Object.assign(()=>now,{set:value=>{now=value;}});}
export function createFakeTransport({runtime,handlers={},entityHandlers={},pull}={}){
 if(runtime===undefined)return createApplicationFake({handlers,entityHandlers,pull});
 check(runtime&&typeof runtime.decodeJson==='function'&&typeof runtime.encodeJson==='function');
 const pushes=[];
 return {pushes,async push(body){
  // Opaque server JSON is parsed only by the real core codec. No host Number path.
  const input=await runtime.decodeJson(body);check(input.kind==='array','InvalidPayload');pushes.push(copyOwned(input.value));
  const outcomes=[];
  for(const envelope of input.value){
   check(envelope.kind==='object','InvalidPayload');const fields=envelope.value,entity=fields.writeId!==undefined;
   const text=key=>{check(fields[key]?.kind==='string','InvalidPayload');return fields[key].value;};
   const correlation=entity?{table:j.string(text('table')),recordId:j.string(text('recordId')),writeId:j.string(text('writeId'))}:{idempotencyKey:j.string(text('idempotencyKey'))};
   const handler=entity?entityHandlers[text('table')]:handlers[text('command')];check(typeof handler==='function','InvalidCommand');
   const outcome=copyOwned(await handler(copyOwned(envelope)));check(outcome&&typeof outcome==='object');
   let business;
   if(outcome.status==='applied'||outcome.status==='corrected'){check(Object.keys(outcome).sort().join(',')==='result,status');business={status:j.string(outcome.status),result:outcome.result};}
   else if(outcome.status==='rejected'){check(Object.keys(outcome).sort().join(',')==='reason,status'&&typeof outcome.reason==='string');business={status:j.string('rejected'),reason:j.string(outcome.reason)};}
   else if(outcome.status==='acknowledged'&&entity){check(Object.keys(outcome).join(',')==='status');business={status:j.string('acknowledged')};}
   else throw fail('InvalidRequest');
   outcomes.push(j.object({...correlation,...business}));
  }
  return runtime.encodeJson(j.array(outcomes));
 },async pull(since,scope){check(typeof pull==='function','UnsupportedOperation');return pull(since,scope);}};
}

// API-0012: the application binding hands plain decoded envelopes to push. Values are validated
// and deep-copied through the application JSON codec (bigint-exact); the fake attaches each
// outcome's correlation itself and never invents an outcome for a missing handler.
const copyJson=value=>decodeJson(encodeJson(value));
const own=(record,key)=>Object.prototype.hasOwnProperty.call(record,key)?record[key]:undefined;
const plainObject=value=>value!==null&&typeof value==='object'&&!Array.isArray(value)&&[Object.prototype,null].includes(Object.getPrototypeOf(value));
function createApplicationFake({handlers,entityHandlers,pull}){
 check(plainObject(handlers)&&plainObject(entityHandlers));
 const pushes=[];
 return {pushes,async push(envelopes){
  check(Array.isArray(envelopes),'InvalidPayload');
  const input=copyJson(envelopes),seen=new Set(),work=[];
  for(const envelope of input){
   check(plainObject(envelope),'InvalidPayload');
   const text=key=>{const value=own(envelope,key);check(typeof value==='string','InvalidPayload');return value;};
   const entity=Object.prototype.hasOwnProperty.call(envelope,'writeId');
   const correlation=entity?{table:text('table'),recordId:text('recordId'),writeId:text('writeId')}:{idempotencyKey:text('idempotencyKey')};
   const identity=JSON.stringify(entity?['entity',correlation.table,correlation.recordId,correlation.writeId]:['command',correlation.idempotencyKey]);
   check(!seen.has(identity),'InvalidPayload');seen.add(identity);
   const handler=entity?own(entityHandlers,correlation.table):own(handlers,text('command'));
   check(typeof handler==='function','InvalidCommand');work.push({envelope,entity,correlation,handler});
  }
  pushes.push(copyJson(input));
  const outcomes=[];
  for(const {envelope,entity,correlation,handler} of work)outcomes.push({...correlation,...business(await handler(copyJson(envelope)),entity)});
  return outcomes;
 },async pull(since,scope){check(typeof pull==='function','UnsupportedOperation');return copyJson(await pull(since,scope));}};
}
// Exactly one business outcome shape; correlation is never accepted from a handler.
function business(outcome,entity){
 check(plainObject(outcome));
 const keys=Object.keys(outcome);
 for(const key of keys){const descriptor=Object.getOwnPropertyDescriptor(outcome,key);check(Object.prototype.hasOwnProperty.call(descriptor,'value'));}
 const shape=keys.slice().sort().join(','),status=outcome.status;
 if((status==='applied'||status==='corrected')&&shape==='result,status')return {status,result:copyJson(outcome.result)};
 if(status==='rejected'&&shape==='reason,status'&&typeof outcome.reason==='string')return {status,reason:outcome.reason};
 if(status==='acknowledged'&&entity&&shape==='status')return {status};
 throw fail('InvalidRequest');
}
