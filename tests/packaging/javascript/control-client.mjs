// Test-only binary controls share one serial worker with actual SDK owners.
import {createRuntime} from '../../../packages/client/src/index.js';
export async function startControl(){
 const worker=new Worker(new URL('./control-worker.mjs',import.meta.url),{type:'module'});let ready,active,closed=false,tail=Promise.resolve();const started=new Promise(resolve=>{ready=resolve;});
 worker.addEventListener('message',event=>{if(event.data.kind==='ready'){ready();return;}const pending=active;active=null;if(!pending)return;if(event.data.kind==='binary')pending.resolve(new Uint8Array(event.data.value));else{const reply=JSON.parse(event.data.value);if(reply.ok)pending.resolve(reply.value);else pending.reject(Object.assign(Error(reply.error.kind),{kind:reply.error.kind}));}});
 await started;
 const request=message=>{const run=()=>{if(closed)throw Object.assign(Error('Closed'),{kind:'Closed'});return new Promise((resolve,reject)=>{active={resolve,reject};worker.postMessage(message,message.key?[message.key]:message.data?[message.data]:[]);});};const result=tail.then(run);tail=result.then(()=>{},()=>{});return result;};
 const serialize=async message=>JSON.stringify({abi:'1',ok:true,value:await request(message)});
 const runtime=createRuntime({open:(input,key)=>serialize({kind:'open',input,key:key.buffer}),call:input=>serialize({kind:'call',input})},{namespace:()=>new Uint8Array(16).fill(7)});
 const file=(operation,name,data)=>request({kind:'control',operation,name,...(data?{data:new Uint8Array(data).buffer}:{})});
 const stats=async()=>Array.from(new Uint32Array((await request({kind:'control',operation:'stats'})).buffer));
 const stop=()=>{closed=true;worker.terminate();active?.reject(Object.assign(Error('Closed'),{kind:'Closed'}));active=null;};
 return {runtime,file,stats,request,stop};
}
