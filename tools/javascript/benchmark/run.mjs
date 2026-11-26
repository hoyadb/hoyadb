// Harness-only endpoint accounting and finite orchestration. No SDK semantics.
import {requireValue,validateWorkers}from'./protocol.mjs';
const mutationOps=new Set(['insert','replace','remove','batch','finish','patch','entity_batch','apply_pull','merge_commit','reset']);
export class FrameMetrics{
 #frames=[];#epoch=0;#last=null;
 constructor(channel,now){this.channel={open:async(input,key)=>{const output=await channel.open(input,key);this.#frames.push({input,output,at:now()});return output;},call:async input=>{const output=await channel.call(input);this.#frames.push({input,output,at:now()});return output;},close:()=>channel.close?.()};}
 snapshot(){this.#frames=[];this.#last=null;return ++this.#epoch;}
 since(epoch){requireValue(epoch===this.#epoch);let input=0,output=0;for(const frame of this.#frames){input+=new TextEncoder().encode(frame.input).length;output+=new TextEncoder().encode(frame.output).length;try{if(mutationOps.has(JSON.parse(frame.input).op))this.#last=frame.at;}catch{/* raw invalid frames remain byte-accounted */}}this.#frames=[];return {input_bytes:input,output_bytes:output};}
 get lastMutationReply(){return this.#last;}
}
const failure=error=>typeof error?.kind==='string'&&/^[A-Za-z]+$/.test(error.kind)?error.kind:'BenchmarkFailure';
export async function warmRun({harness,cohort,run,identity,emit,onSample=async()=>{}}){
 requireValue(Number.isInteger(run)&&run>=1&&run<=5);const samples=[];let failed=false,proofError=null;
 await harness.proof('initial',emit);
 for(const [phase,count]of[['warmup',5],['measured',30]]){
  for(let index=0;index<count;index++){
   let sample;try{sample={...await harness.sample(cohort,index),phase,index};requireValue(sample.status==='ok'&&sample.correct===true&&Number.isSafeInteger(sample.value)&&sample.value>=0);}catch(error){sample={phase,index,status:'error',correct:false,error:failure(error)};failed=true;}
   samples.push(sample);await onSample(sample);if(failed)break;
  }if(failed)break;
 }
 try{await harness.proof('final',emit);}catch(error){proofError=failure(error);failed=true;}
 return {format:1,identity,protocol:{runs:5,warmups:5,samples:30},runs:[{run,status:failed?'failed':'ok',samples,...(proofError?{proof_error:proofError}:{})}]};
}
export async function coldExecutions(count,launch){requireValue(count===10);const results=[];for(let index=0;index<count;index++)results.push(await launch(index));validateWorkers(results.map(value=>value.worker_id),count);return results;}
