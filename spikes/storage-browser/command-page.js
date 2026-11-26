import {workerError} from './harness-error.js';
import {runCommandChecks,runCommandPagingChecks,restartChecks} from './command-tests.js';
const output=document.querySelector('#results');
const key='hoyadb-phase-four-run';
const pagingOnly=new URL(location.href).searchParams.has('paging');
const saved=sessionStorage.getItem(key);
const state=saved?JSON.parse(saved):{namespace:`hoyadb-spec23-${crypto.randomUUID()}`,runtime:navigator.userAgent,origin:location.origin,results:[],done:false};
window.hoyaCommands=state;
const render=()=>{output.textContent=JSON.stringify(state,null,2);};
const worker=new Worker(`./worker.js?run=${crypto.randomUUID()}`,{type:'module'});
const pending=new Map();let sequence=0,reloading=false;
worker.onmessage=({data})=>{const item=pending.get(data.request);if(item){pending.delete(data.request);clearTimeout(item.timer);item.resolve(data);}};
worker.onerror=()=>{for(const item of pending.values()){clearTimeout(item.timer);item.reject(workerError(null,'command-page/worker event'));}pending.clear();};
function request(message){const request=++sequence;const operation=message.op==='command_rpc'?`command_rpc/${/^\{\"op\":\"([a-z_]+)\"/.exec(message.input)?.[1]||'unknown'}`:message.op;const start=performance.now();return new Promise((resolve,reject)=>{const timeout=message.op==='command_conformance'&&message.index===13?900000:60000;const timer=setTimeout(()=>{pending.delete(request);reject(new Error(`worker ${operation} timed out after ${timeout}ms`));},timeout);pending.set(request,{resolve,reject,timer});worker.postMessage({...message,request});}).finally(()=>{(state.rpc_timings??=[]).push({operation,elapsed_ms:Math.round(performance.now()-start)});});}
async function ok(message){const response=await request(message);if(response.error)throw workerError(response.error,`command-page/${message.op}${Number.isInteger(message.index)?`[${message.index}]`:''}`);return response.value;}
function bounded(work,timeout=60000){let timer;return Promise.race([work,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(`host scenario deadline ${timeout}ms exceeded`)),timeout);})]).finally(()=>clearTimeout(timer));}
const report=(name,detail)=>{state.results.push({name,passed:true,detail});render();};
try{
 state.capabilities=await ok({op:'init',namespace:state.namespace});
 if(!saved){
  if(new URL(location.href).searchParams.has('shared')){
   for(let index=0;index<14;index++){state.current=index;render();const start=performance.now();const result=await ok({op:'command_conformance',index});if(!result.case||result.case==='stub')throw new Error('shared case missing');if(JSON.stringify(result.files_before)!==JSON.stringify(result.files_after))throw new Error('pooled inventory changed');report(`C20 shared ${index} ${result.case}`,{...result,elapsed_ms:Math.round(performance.now()-start)});}
  }
  if(!pagingOnly)await bounded(runCommandChecks({request,report}));
  const pagingStart=performance.now();await bounded(runCommandPagingChecks({request,report}),900000);report('large-message host scenario wall time',{elapsed_ms:Math.round(performance.now()-pagingStart),watchdog_ms:900000,rpc_deadline_ms:60000,http_deadline_ms:5000});
 }
 reloading=pagingOnly?false:await bounded(restartChecks({request,report,state}));
 if(reloading){sessionStorage.setItem(key,JSON.stringify(state));render();setTimeout(()=>location.reload(),100);}
}catch(error){state.results.push({name:'failure',passed:false,detail:String(error),stack:error.stack});render();}
finally{
 if(!reloading){
  try{await ok({op:'shutdown'});await(await navigator.storage.getDirectory()).removeEntry(state.namespace,{recursive:true});state.cleaned=true;}catch(error){state.cleanupError=String(error);}
  worker.terminate();state.done=true;sessionStorage.removeItem(key);render();
 }
}
