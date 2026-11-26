// Harness orchestration only: all database semantics are delegated to SDKBenchmark.
import {warmRun}from'./run.mjs';
const kind=error=>typeof error?.kind==='string'?error.kind:'BenchmarkFailure';
export async function cohortFlow({api,platform,engine,cohort,run,worker_id}){
 // The controller's quiet gate is checked before creating a worker or a store.
 const info=await api.start({engine,cohort,run,worker_id});let fixture,document,error;
 try{
  fixture=await platform.launch(info,engine);
  const harness=fixture.harness;await harness.seed();await harness.setQueue(info.cohort.Q);
  if(info.cohort.operation==='reactivity')await harness.observe(info.cohort);
  await api.lines(info.run_id,'READY\n');
  document=await warmRun({harness,cohort:info.cohort,run,identity:info.identity,emit:line=>api.lines(info.run_id,line),onSample:sample=>api.sample(info.run_id,sample)});
 }catch(cause){error=kind(cause);document={format:1,identity:info.identity,protocol:{runs:5,warmups:5,samples:30},runs:[{run,status:'failed',samples:[],error}]};}
 return api.finish(info.run_id,await retire(fixture,document,error,true));
}
async function retire(fixture,document,error,observed){
 let closed=false,terminated=false,lifecycle=null;
 if(fixture){
  if(observed)try{await fixture.harness.unobserve();}catch(cause){error??=kind(cause);}
  try{await fixture.close();closed=true;}catch(cause){error??=kind(cause);}
  try{fixture.terminate();terminated=true;}catch(cause){error??=kind(cause);}
  try{lifecycle=await fixture.cleanup();}catch(cause){error??=kind(cause);}
 }
 document.lifecycle={...lifecycle,owner_closed:closed,worker_terminated:terminated};
 // Untimed storage observations stay outside the exactly validated lifecycle record.
 document.footprint=fixture?.footprint??null;
 if(error){document.runs[0].status='failed';document.runs[0].error=error;}
 return document;
}
// One fresh worker per cold run: READY precedes the untimed full-row proofs.
export async function coldFlow({api,platform,engine,operation,n,run,worker_id}){
 const info=await api.startCold({engine,operation,n,run,worker_id});let fixture,document,error;const emit=line=>api.lines(info.run_id,line);
 try{
  fixture=await platform.launchCold(info,engine,{onReady:()=>emit('READY\n')});
  await fixture.harness.proof('initial',emit);await fixture.harness.proof('final',emit);
  document={format:1,identity:info.identity,protocol:{runs:10,warmups:0,samples:1},runs:[{run,status:'ok',samples:[{...fixture.sample,phase:'measured',index:0}]}]};
 }catch(cause){error=kind(cause);document={format:1,identity:info.identity,protocol:{runs:10,warmups:0,samples:1},runs:[{run,status:'failed',samples:[],error}]};}
 return api.finish(info.run_id,await retire(fixture,document,error,false));
}
// Page permit loop: each run uses exactly one parent-issued permit; any none or failure stops it.
export async function matrixLoop({api,flows}){
 const runs=[];
 for(;;){
  const reply=await api.next();
  if(reply?.status!=='permit')return{stopped:reply?.status==='none'&&typeof reply.reason==='string'?reply.reason:'invalid_reply',runs};
  const {kind:flow,...coordinates}=reply.coordinates??{};
  if(!['warm','cold','resources'].includes(flow))return{stopped:'unknown_permit_kind',runs};
  let result;
  try{result=await flows[flow](coordinates);}catch(cause){runs.push({coordinates:reply.coordinates,status:'failed',error:kind(cause)});return{stopped:'failed_flow',runs};}
  runs.push({coordinates:reply.coordinates,status:result?.status});
  if(!['accepted','accepted_contaminated'].includes(result?.status))return{stopped:'failed_flow',runs};
 }
}
