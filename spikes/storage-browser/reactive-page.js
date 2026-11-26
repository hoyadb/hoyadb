import {workerError} from './harness-error.js';
import { runReactiveChecks, runReviewLargeChecks, runReviewFailureChecks, runReviewSharedChecks } from './reactive-tests.js';
// TC-0029 R13: owned synthetic OPFS namespace; no origin-wide cleanup.
const output=document.querySelector('#results');
const namespace=`hoyadb-spec23-${crypto.randomUUID()}`;
const worker=new Worker(`./worker.js?run=${crypto.randomUUID()}`,{type:'module'});
const pending=new Map();let sequence=0;
window.hoyaReactive={namespace,runtime:navigator.userAgent,origin:location.origin,results:[],done:false};
const render=()=>{output.textContent=JSON.stringify(window.hoyaReactive,null,2);};
worker.onmessage=({data})=>{const item=pending.get(data.request);if(item){pending.delete(data.request);clearTimeout(item.timer);item.resolve(data);}};
worker.onerror=()=>{for(const item of pending.values()){clearTimeout(item.timer);item.reject(workerError(null,'reactive-page/worker event'));}pending.clear();};
function request(message){const request=++sequence;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(request);reject(new Error(`worker ${message.op} timed out`));},60000);pending.set(request,{resolve,reject,timer});worker.postMessage({...message,request});});}
async function ok(message){const response=await request(message);if(response.error)throw workerError(response.error,`reactive-page/${message.op}${Number.isInteger(message.index)?`[${message.index}]`:''}`);return response.value;}
try {
  await ok({op:'init',namespace});
  const checks={request,report:(name,detail)=>{window.hoyaReactive.results.push({name,passed:true,detail});render();}};
  const review=new URL(location.href).searchParams.get('review');
  if(review==='large') await runReviewLargeChecks(checks);
  else if(review==='failure') await runReviewFailureChecks(checks);
  else { await runReactiveChecks(checks); await runReviewLargeChecks(checks); await runReviewSharedChecks(checks); await runReviewFailureChecks(checks); }
} catch(error){window.hoyaReactive.results.push({name:'failure',passed:false,detail:String(error)});}
finally {
  try{await ok({op:'shutdown'});await(await navigator.storage.getDirectory()).removeEntry(namespace,{recursive:true});window.hoyaReactive.cleaned=true;}catch(error){window.hoyaReactive.cleanupError=String(error);}
  worker.terminate();window.hoyaReactive.done=true;render();
}
