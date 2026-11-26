// Installed-package application harness; no operation starts automatically.
import{createWebRuntime,openDatabase}from'@hoyadb/client';
import{createRuntime}from'@hoyadb/client/host';
import{openMemoryDatabase}from'@hoyadb/testing';
import{startControl}from'./control-client.mjs';
import{createSDKPlatform}from'./platform.mjs';
import{createHTTPAPI}from'./http.mjs';
import{cohortFlow,coldFlow,matrixLoop}from'./flow.mjs';
import{resourceFlow}from'./resources.mjs';
import{poolFootprint}from'./storage.mjs';
import{requireValue,clockResolution}from'./protocol.mjs';
const meta=await(await fetch('/benchmark/meta')).json(),output=document.querySelector('#output');
const hash=async bytes=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),byte=>byte.toString(16).padStart(2,'0')).join('');
const bootstrap=async()=>{const worker=new Worker(new URL('./node_modules/@hoyadb/client/src/worker.js',import.meta.url),{type:'module'});try{return{runtime:await createWebRuntime({worker}),terminate:()=>worker.terminate()};}catch(error){worker.terminate();throw error;}};
// Resource class only: same ordinary WASM/worker host plus a separate metrics MessagePort.
const bootstrapResources=async()=>{const worker=new Worker(new URL('./resource-worker.mjs',import.meta.url),{type:'module'});try{const runtime=await createWebRuntime({worker}),channel=new MessageChannel();worker.postMessage({kind:'harness-metrics'},[channel.port2]);let tail=Promise.resolve();const linearBytes=()=>{const next=tail.then(()=>new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('harness metric timeout')),60000);channel.port1.onmessage=event=>{clearTimeout(timer);resolve(event.data.harness_worker_linear_bytes);};channel.port1.postMessage('measure');}));tail=next.catch(()=>{});return next;};return{runtime,linearBytes,terminate:()=>{channel.port1.close();worker.terminate();}};}catch(error){worker.terminate();throw error;}};
const platform=now=>createSDKPlatform({metadata:meta.metadata,bootstrap,bootstrapResources,createRuntime,openMemoryDatabase,openDatabase,startControl,key:()=>new Uint8Array(32).fill(0x53),hash,now,poolSizes:async()=>poolFootprint(await navigator.storage.getDirectory())});
const environment=()=>({user_agent:navigator.userAgent,visibility:document.visibilityState,clock_minimum_positive_ms:clockResolution(()=>performance.now()),power:'unknown',thermal:'unknown',background:'coordinated externally'});
let active=false;
// Results and static failure kinds stay visible after an automation call times out.
async function exclusive(operation){requireValue(!active,'benchmark operation already active');active=true;output.textContent='{"status":"running"}';try{const result=await operation();output.textContent=JSON.stringify(result,null,2);return result;}catch(error){output.textContent=JSON.stringify({status:'failed',kind:typeof error?.kind==='string'?error.kind:'BenchmarkFailure',message:String(error?.message??error).slice(0,200)});throw error;}finally{active=false;}}
globalThis.hoyaBenchmark={metadata:meta.metadata,measurements_allowed:meta.measurements_allowed,
 // Verification uses a synthetic endpoint clock and cannot produce a baseline.
 verify:(engine,cohort)=>exclusive(async()=>{
  requireValue(engine==='memory'||engine==='opfs-protected');const parameters=meta.metadata.cohorts[cohort];requireValue(parameters&&((parameters.N===100&&parameters.Q===0)||(parameters.operation==='reactivity'&&parameters.observers===1&&parameters.sharing==='equivalent')),'verification-only bounded subset');
  let tick=0,fixture,proofLines=0;const stem='phase-seven-sdk-owned-bench-verify-'+engine+'-'+String(cohort);const info={cohort:parameters,owned_names:engine==='memory'?[]:[stem+'.sqlite',stem+'.neighbor.sqlite']};
  try{fixture=await platform(()=>tick++).launch(info,engine);const harness=fixture.harness;await harness.seed();await harness.setQueue(parameters.Q);if(parameters.operation==='reactivity')await harness.observe(parameters);await harness.proof('initial',async()=>{proofLines++;});await harness.sample(parameters,0);await harness.unobserve();await harness.proof('final',async()=>{proofLines++;});await fixture.close();fixture.terminate();const lifecycle=await fixture.cleanup();return{status:'verified_not_measured',engine,cohort,protection:fixture.protection,proof_lines:proofLines,proof_qualification:'SDK fixture exact-value checks; not independently accepted performance proof',lifecycle};}
  catch(error){if(fixture){await fixture.harness.unobserve().catch(()=>{});await fixture.close().catch(()=>{});fixture.terminate();await fixture.cleanup().catch(()=>{});}throw error;}
 }),
 // Cold verification: fresh-worker fixture/open/proof/cleanup with a synthetic clock; never a sample.
 verifyCold:(engine,operation,n)=>exclusive(async()=>{
  let tick=0,fixture,proofLines=0,ready=0;const stem='phase-seven-sdk-owned-bench-verify-cold-'+operation+'-n'+String(n);const info={cohort:{operation,N:n,Q:0,observers:0},owned_names:engine==='memory'?[]:[stem+'.sqlite',stem+'.neighbor.sqlite']};
  try{fixture=await platform(()=>tick++).launchCold(info,engine,{onReady:async()=>{ready++;}});await fixture.harness.proof('initial',async()=>{proofLines++;});await fixture.harness.proof('final',async()=>{proofLines++;});await fixture.close();fixture.terminate();const lifecycle=await fixture.cleanup();return{status:'verified_not_measured',engine,operation,n,protection:fixture.protection,ready_notifications:ready,proof_lines:proofLines,footprint:fixture.footprint,proof_qualification:'SDK fixture exact-value checks; not independently accepted performance proof',lifecycle};}
  catch(error){if(fixture){await fixture.close().catch(()=>{});fixture.terminate();await fixture.cleanup().catch(()=>{});}throw error;}
 }),
 // Resource verification: actual harness-worker cycles with a local acknowledgement; not accepted evidence.
 verifyResources:(engine,{profiler=false}={})=>exclusive(async()=>{
  const records=[];let proofLines=0,document;const stem='phase-seven-sdk-owned-bench-verify-resources';
  const api={startResources:async()=>({run_id:'verify',identity:null,cohort:{operation:'retained-resources',N:100,Q:0,observers:1,mode:'membership',sharing:'equivalent'},owned_names:engine==='memory'?[]:[stem+'.sqlite',stem+'.neighbor.sqlite']}),lines:async()=>{proofLines++;},checkpoint:async(_,record)=>{records.push(record);return{reply:'CONTINUE'};},finish:async(_,value)=>{document=value;return value;}};
  await resourceFlow({api,platform:platform(()=>performance.now()),engine,run:1,worker_id:'verify',performance:profiler?performance:{}});
  return{status:document.runs[0].status==='ok'?'verified_not_accepted':'failed',engine,checkpoints:records.length,proof_lines:proofLines,first:records[0]??null,last:records.at(-1)??null,ua_statuses:[...new Set(records.map(r=>r.ua_specific.status))],harness_bytes:records.map(r=>r.harness_worker_linear_bytes),document,qualification:'actual harness-worker resource flow; local acknowledgement, no independent proof capture; not accepted resource evidence'};
 }),
 resources:args=>exclusive(async()=>{requireValue(meta.measurements_allowed===true,'quiet approval required');const observation=environment();const result=await resourceFlow({api:createHTTPAPI(fetch.bind(globalThis)),platform:platform(()=>performance.now()),performance,...args});return{result,observation};}),
 // Parent-permitted sequence; stops on any none/failure, explicit re-invocation resumes.
 matrix:()=>exclusive(async()=>{requireValue(meta.measurements_allowed===true,'quiet approval required');const api=createHTTPAPI(fetch.bind(globalThis)),observation=environment(),run=now=>platform(now);const result=await matrixLoop({api,flows:{warm:args=>cohortFlow({api,platform:run(()=>performance.now()),...args}),cold:args=>coldFlow({api,platform:run(()=>performance.now()),...args}),resources:args=>resourceFlow({api,platform:run(()=>performance.now()),performance,...args})}});return{result,observation};}),
 warm:args=>exclusive(async()=>{requireValue(meta.measurements_allowed===true,'quiet approval required');const observation=environment();const result=await cohortFlow({api:createHTTPAPI(fetch.bind(globalThis)),platform:platform(()=>performance.now()),...args});return{result,observation};}),
 cold:args=>exclusive(async()=>{requireValue(meta.measurements_allowed===true,'quiet approval required');const observation=environment();const result=await coldFlow({api:createHTTPAPI(fetch.bind(globalThis)),platform:platform(()=>performance.now()),...args});return{result,observation};})
};
output.textContent='Ready. No benchmark operation has started. Measurements '+(meta.measurements_allowed?'explicitly enabled by controller.':'blocked pending quiet approval.');
