// Untimed fixture verification ONLY: injected synthetic endpoint clock, no samples/baseline.
import {readFile}from'node:fs/promises';
import init,{Facade}from'../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {createRuntime,j}from'@hoyadb/client/host';
import {openMemoryDatabase}from'@hoyadb/testing';
import {SDKBenchmark}from'./harness.mjs';
const metadata=JSON.parse(await readFile(new URL('../../../benchmarks/browser-workloads.json',import.meta.url)));
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});
const facade=new Facade(),runtime=createRuntime({open:(input,key)=>facade.open(input,key),call:input=>facade.call(input)});
const cohorts=metadata.cohorts.filter(value=>(value.N===100&&value.Q===0)||(value.operation==='reactivity'&&value.observers===1&&value.sharing==='equivalent'));
let count=0;
try{
 for(const cohort of cohorts){
  let db,clock=0;
  try{
   db=await openMemoryDatabase({runtime,schema:metadata.schema,namespace:metadata.namespace,context:()=>j.null(),clock:()=>1000,jitter:()=>0});
   const harness=new SDKBenchmark({db,runtime,n:cohort.N,now:()=>clock++});await harness.seed();await harness.setQueue(cohort.Q);if(cohort.operation==='reactivity')await harness.observe(cohort);await harness.proof('initial',async()=>{});await harness.sample(cohort,0);await harness.unobserve();await harness.proof('final',async()=>{});console.log('PASS correctness-only '+cohort.operation+(cohort.mode?'/'+cohort.mode:''));count++;
  }finally{await db?.close();}
 }
 console.log('PASS '+count+' actual WASM/core fixture cases; synthetic endpoint clock; NO performance measurements or accepted samples');
}finally{facade.free();}
