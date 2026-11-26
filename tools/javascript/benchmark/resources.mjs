// Resource observations are untimed and never feed the latency summarizer.
import{requireValue,canonical}from'./protocol.mjs';
export async function memoryObservation({performance,harnessLinearBytes=null,timers={set:(fn,delay)=>setTimeout(fn,delay),clear:token=>clearTimeout(token)},timeout=60000}){
 requireValue(harnessLinearBytes===null||(Number.isSafeInteger(harnessLinearBytes)&&harnessLinearBytes>=0&&harnessLinearBytes%65536===0));
 const result={ordinary_worker_linear_bytes:null,harness_worker_linear_bytes:harnessLinearBytes,js_heap_bytes:null,ua_specific:{status:'unknown',reason:'unavailable'}};
 if(typeof performance?.measureUserAgentSpecificMemory!=='function')return result;
 requireValue(Number.isSafeInteger(timeout)&&timeout>0);let timer;
 try{
  const timedout=Symbol('timeout');const value=await Promise.race([Promise.resolve().then(()=>performance.measureUserAgentSpecificMemory()),new Promise(resolve=>{timer=timers.set(()=>resolve(timedout),timeout);})]);
  if(value===timedout){result.ua_specific={status:'unknown',reason:'timeout'};return result;}
  requireValue(value&&Number.isSafeInteger(value.bytes)&&value.bytes>=0&&Array.isArray(value.breakdown),'invalid profiler result');
  const serialized=canonical(value);requireValue(new TextEncoder().encode(serialized).length<=1048576,'profiler record budget');
  result.ua_specific={status:'observed',value:JSON.parse(serialized)};
 }catch{result.ua_specific={status:'unknown',reason:'unavailable_or_rejected'};}
 finally{if(timer!==undefined)timers.clear(timer);}
 return result;
}
export async function resourceCycles({open,observe,verify,close,checkpoint}){
 await checkpoint({phase:'baseline',cycle:null});
 for(let cycle=0;cycle<30;cycle++){
  const db=await open(cycle);
  // Every cycle validates the complete fixture; only first/final emit independent proof.
  try{await observe(db);await verify(db,cycle===0?'initial':cycle===29?'final':null);await checkpoint({phase:'after_open',cycle});}
  finally{await close(db);}
  await checkpoint({phase:'after_close',cycle});
 }
}
const failure=error=>typeof error?.kind==='string'?error.kind:'BenchmarkFailure';
// One harness worker per resource run; checkpoints are acknowledged by the controller.
export async function resourceFlow({api,platform,engine,run,worker_id,performance}){
 const info=await api.startResources({engine,run,worker_id});const emit=line=>api.lines(info.run_id,line);let fixture,error,lifecycle=null,terminated=false;
 try{
  fixture=await platform.launchResources(info,engine);
  await resourceCycles({open:cycle=>fixture.open(cycle),observe:harness=>harness.observe(info.cohort),
   verify:(harness,phase)=>harness.proof(phase??'intermediate',phase?emit:async()=>{}),
   close:async harness=>{try{await harness.unobserve();}finally{await fixture.close(harness);}},
   checkpoint:async point=>{const observed=await memoryObservation({performance,harnessLinearBytes:await fixture.linearBytes()});let record={...point,...observed};const size=new TextEncoder().encode(canonical(record)).length;
    // A profiler record beyond the bounded controller frame stays unknown with its size; never truncated.
    if(size>258048)record={...record,ua_specific:{status:'unknown',reason:'exceeds_bounded_transport',serialized_bytes:size}};const reply=await api.checkpoint(info.run_id,record);requireValue(reply?.reply==='CONTINUE','missing controller acknowledgement');}});
 }catch(cause){error=failure(cause);}
 if(fixture){try{fixture.terminate();terminated=true;}catch(cause){error??=failure(cause);}try{lifecycle=await fixture.cleanup();}catch(cause){error??=failure(cause);}}
 const document={format:1,identity:info.identity,protocol:{cycles:30,checkpoints:61},lifecycle:{...lifecycle,worker_terminated:terminated},footprint:fixture?.footprint??null,runs:[{run,status:error?'failed':'ok',...(error?{error}:{})}]};
 return api.finish(info.run_id,document);
}
