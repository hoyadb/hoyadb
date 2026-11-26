// API-0011: one preloaded worker realm per app, never a worker per database.
import {check,failure} from './errors.js';
import {createRuntime} from './runtime.js';
function workerChannel(worker,onRetire){
 check(worker&&typeof worker.postMessage==='function');
 let resolveReady,rejectReady,readyDone=false,closed=false,active=null,tail=Promise.resolve();
 const ready=new Promise((yes,no)=>{resolveReady=yes;rejectReady=no;});void ready.catch(()=>{});
 function retire(kind){
  if(closed)return;closed=true;
  const error=failure(kind);if(!readyDone)rejectReady(error);active?.reject(error);active=null;
  try{worker.removeEventListener('message',message);worker.removeEventListener('error',failed);worker.removeEventListener('messageerror',failed);worker.terminate();}finally{onRetire?.();}
 }
 function failed(event){event?.preventDefault?.();retire('BackendFailure');}
 function message(event){
  const value=event.data;
  if(!value||typeof value!=='object'){retire('InvalidPayload');return;}
  if(value.kind==='ready'&&Object.keys(value).length===1&&!readyDone){readyDone=true;resolveReady();return;}
  if(value.kind==='reply'&&Object.keys(value).sort().join(',')==='kind,value'&&typeof value.value==='string'&&active){const pending=active;active=null;pending.resolve(value.value);return;}
  retire('InvalidPayload');
 }
 worker.addEventListener('message',message);worker.addEventListener('error',failed);worker.addEventListener('messageerror',failed);
 function submit(input,key){
  const run=async()=>{check(!closed,'Closed');await ready;check(!closed,'Closed');return new Promise((resolve,reject)=>{
   active={resolve,reject};try{if(key===undefined)worker.postMessage({kind:'call',input});else{check(key instanceof Uint8Array);const owned=key.byteOffset===0&&key.byteLength===key.buffer.byteLength?key:new Uint8Array(key);worker.postMessage({kind:'open',input,key:owned.buffer},[owned.buffer]);}}catch{retire('BackendFailure');}
  });};
  const result=tail.then(run);tail=result.then(()=>undefined,()=>undefined);return result;
 }
 return {ready,open:(input,key)=>submit(input,key),call:input=>submit(input),close:()=>retire('Closed')};
}
export function createWorkerChannel(worker){return workerChannel(worker);}
let shared;
export function createWebRuntime(options={}){
 if(!options.worker&&shared)return shared.pending;
 const attempt={pending:null,retired:false};
 const clear=()=>{attempt.retired=true;if(shared===attempt)shared=undefined;};
 const pending=(async()=>{
  const worker=options.worker??new Worker(new URL('./worker.js',import.meta.url),{type:'module'});
  const channel=workerChannel(worker,clear);await channel.ready;
  const namespace=options.namespace??(()=>{const bytes=new Uint8Array(16);check(typeof globalThis.crypto?.getRandomValues==='function','BackendFailure');globalThis.crypto.getRandomValues(bytes);return bytes;});
  return createRuntime(channel,{namespace});
 })();
 attempt.pending=pending;if(!options.worker&&!attempt.retired){shared=attempt;void pending.catch(clear);}return pending;
}
