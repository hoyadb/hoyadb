import {workerError} from './harness-error.js';
import {runEntityChecks,entityRestart} from './entity-tests.js';
const output=document.querySelector('#results'),key='hoyadb-phase-five-run';
const saved=sessionStorage.getItem(key);
const state=saved?JSON.parse(saved):{namespace:`hoyadb-spec23-${crypto.randomUUID()}`,runtime:navigator.userAgent,origin:location.origin,results:[],done:false};
window.hoyaEntities=state;
const render=()=>{output.textContent=JSON.stringify(state,null,2);};
const worker=new Worker(`./worker.js?run=${crypto.randomUUID()}`,{type:'module'});
const pending=new Map();let sequence=0,reloading=false;
worker.onmessage=({data})=>{const item=pending.get(data.request);if(item){pending.delete(data.request);clearTimeout(item.timer);item.resolve(data);}};
worker.onerror=()=>{for(const item of pending.values()){clearTimeout(item.timer);item.reject(workerError(null,'entity-page/worker event'));}pending.clear();};
function request(message){const request=++sequence,start=performance.now();return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(request);reject(new Error('entity worker request deadline60000ms'));},60000);pending.set(request,{resolve,reject,timer});worker.postMessage({...message,request});}).finally(()=>{(state.rpc_timings??=[]).push({operation:message.op,elapsed_ms:Math.round(performance.now()-start)});});}
async function ok(message){const response=await request(message);if(response.error)throw workerError(response.error,`entity-page/${message.op}${Number.isInteger(message.index)?`[${message.index}]`:''}`);return response.value;}
function bounded(work){let timer;return Promise.race([work,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('entity host scenario deadline60000ms')),60000);})]).finally(()=>clearTimeout(timer));}
const report=(name,detail)=>{state.results.push({name,passed:true,detail});render();};
const sharedCases=['automatic_entities','admission_and_inverse','two_client_resolution','pull_atomicity_bootstrap_and_reset','reset_inventory','lifecycle_comparison_guard','pull_recovery_protection','progress_transition_guard','reset_modes_and_coordinator_guard','managed_links_and_mapped_rows','reconfiguration_guards','typed_correction_clocks','reconfigure_inventory','pull_tombstone_mapping_target','pull_after_rolled_back_sent_cancellation'];
try {
 state.capabilities=await ok({op:'init',namespace:state.namespace});
 if(!saved){
  if(new URL(location.href).searchParams.has('shared'))for(const [index,expected] of sharedCases.entries()){
   const start=performance.now(),result=await ok({op:'entity_conformance',index});
   if(result.case!==expected||JSON.stringify(result.files_before)!==JSON.stringify(result.files_after))throw new Error('shared entity case or inventory invalid');
   report(`E shared ${index} ${result.case}`,{...result,elapsed_ms:Math.round(performance.now()-start)});
  }
  await bounded(runEntityChecks({request,report}));
 }
 reloading=await bounded(entityRestart({request,report,state}));
 if(reloading){sessionStorage.setItem(key,JSON.stringify(state));render();setTimeout(()=>location.reload(),100);}
}catch(error){state.results.push({name:'failure',passed:false,detail:String(error),stack:error.stack});render();}
finally {
 if(!reloading){
  try{await ok({op:'shutdown'});await(await navigator.storage.getDirectory()).removeEntry(state.namespace,{recursive:true});state.cleaned=true;}catch(error){state.cleanupError=String(error);}
  worker.terminate();state.done=true;sessionStorage.removeItem(key);render();
 }
}
