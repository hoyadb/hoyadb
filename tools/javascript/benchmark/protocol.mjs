// TC-0037 harness validation, not a production SDK or database implementation.
export function requireValue(condition,message='invalid benchmark evidence'){if(!condition)throw Error(message);}
export function canonical(value){
 if(value===null||typeof value==='boolean'||typeof value==='string')return JSON.stringify(value);
 if(typeof value==='number'){requireValue(Number.isFinite(value));return JSON.stringify(value);}
 if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';
 requireValue(value&&typeof value==='object'&&Object.getPrototypeOf(value)===Object.prototype);
 return '{'+Object.keys(value).sort().map(key=>JSON.stringify(key)+':'+canonical(value[key])).join(',')+'}';
}
export function exact(actual,expected){requireValue(canonical(actual)===canonical(expected),'benchmark typed value mismatch');return true;}
export function validateMetadata(metadata,parent){
 requireValue(metadata.status==='reviewed_for_implementation'&&metadata.measurements==='not_started');
 exact(metadata.classes,['browser-sdk/memory','browser-sdk/opfs-protected']);
 for(const key of ['fixture','schema','dataset','cohorts','sampling','operation_details'])exact(metadata[key],parent[key]);
 requireValue(metadata.cohorts.length===51);
 exact(metadata.sampling,{cold_processes:10,independent_runs:5,samples:30,warmups:5});
 requireValue(/^[a-f0-9]{64}$/.test(metadata.source_workload.sha256));return metadata;
}
export function nanoseconds(start,end){
 requireValue(Number.isFinite(start)&&Number.isFinite(end)&&start>=0&&end>=start,'invalid performance timestamp');
 const result=Math.round((end-start)*1000000);requireValue(Number.isSafeInteger(result)&&result>=0,'unsafe performance duration');return result;
}
export function clockResolution(read,count=10000){
 requireValue(Number.isSafeInteger(count)&&count>0);let previous=read(),minimum=null;requireValue(Number.isFinite(previous));
 for(let i=0;i<count;i++){const next=read();requireValue(Number.isFinite(next)&&next>=previous);if(next>previous)minimum=minimum===null?next-previous:Math.min(minimum,next-previous);previous=next;}return minimum;
}
export function validateWorkers(ids,count=10){requireValue(Array.isArray(ids)&&ids.length===count&&ids.every(id=>typeof id==='string'&&id.length>0)&&new Set(ids).size===count,'worker execution count/identity mismatch');return ids;}
export function checkedResource(value){
 if(value===null)return {status:'unknown',ordinary_worker_linear_bytes:null,harness_worker_linear_bytes:null};
 requireValue(value&&Object.keys(value).join(',')==='harness_worker_linear_bytes');const bytes=value.harness_worker_linear_bytes;
 requireValue(Number.isSafeInteger(bytes)&&bytes>=0&&bytes%65536===0);
 return {status:'observed_harness_only',ordinary_worker_linear_bytes:null,harness_worker_linear_bytes:bytes};
}
