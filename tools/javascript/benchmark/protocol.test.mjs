import test from 'node:test';import assert from 'node:assert/strict';import {readFile}from'node:fs/promises';
import {canonical,validateMetadata,nanoseconds,clockResolution,exact,validateWorkers,checkedResource}from'./protocol.mjs';
const metadata=JSON.parse(await readFile(new URL('../../../benchmarks/browser-workloads.json',import.meta.url))),parent=JSON.parse(await readFile(new URL('../../../benchmarks/workloads.json',import.meta.url)));
const clone=value=>JSON.parse(JSON.stringify(value));
test('P01 exact51 frozen cohorts schema counts operation details reject divergence',()=>{
 assert.equal(validateMetadata(metadata,parent),metadata);
 for(const alter of [m=>m.cohorts.pop(),m=>m.cohorts.push(m.cohorts[0]),m=>m.schema.tables.articles.columns.title.optional=true,m=>m.sampling.samples=29,m=>m.sampling.independent_runs=1,m=>m.operation_details['queue-baseline']='queued']){
  const changed=clone(metadata);alter(changed);assert.throws(()=>validateMetadata(changed,parent));
 }
});
test('P02 real clock conversion preserves endpoints and rejects invalid duration',()=>{
 assert.equal(nanoseconds(5.25,7.125),1875000);assert.equal(nanoseconds(1,1),0);
 for(const pair of [[2,1],[0,NaN],[0,Infinity],[0,Number.MAX_SAFE_INTEGER]])assert.throws(()=>nanoseconds(...pair));
});
test('P02 empirical resolution ignores equal readings without inventing precision',()=>{
 let index=0;const ticks=[1,1,1.25,1.375,1.375];assert.equal(clockResolution(()=>ticks[index++],4),0.125);
 assert.equal(clockResolution(()=>2,10),null);assert.throws(()=>clockResolution(()=>NaN,10));
});
test('P01 independent structural comparison rejects wrong Float Null omission and ID',()=>{
 const row={id:'article:000000',fields:{score:{kind:'integer',value:'0'},ratio:{kind:'float',value:'0.25'}}};
 assert.equal(exact(row,clone(row)),true);
 for(const alter of [r=>r.id='wrong',r=>r.fields.ratio={kind:'integer',value:'0'},r=>r.fields.note={kind:'null'}]){const changed=clone(row);alter(changed);assert.throws(()=>exact(changed,row));}
 assert.equal(canonical({z:'東京🌿',a:{z:1,a:2}}),'{"a":{"a":2,"z":1},"z":"東京🌿"}');
});
test('P02 ten cold executions require distinct nonempty worker identities',()=>{
 const ids=Array.from({length:10},(_,i)=>'worker:'+i);assert.deepEqual(validateWorkers(ids),ids);
 for(const bad of [ids.slice(1),[...ids.slice(1),ids[1]],ids.map(()=>''),Array(10).fill(null)])assert.throws(()=>validateWorkers(bad));
});
test('P03 unsupported resource remains unknown and harness allocation never ordinary allocation',()=>{
 assert.deepEqual(checkedResource(null),{status:'unknown',ordinary_worker_linear_bytes:null,harness_worker_linear_bytes:null});
 assert.deepEqual(checkedResource({harness_worker_linear_bytes:65536}),{status:'observed_harness_only',ordinary_worker_linear_bytes:null,harness_worker_linear_bytes:65536});
 for(const bad of [{harness_worker_linear_bytes:-1},{harness_worker_linear_bytes:1.5},{ordinary_worker_linear_bytes:65536},{harness_worker_linear_bytes:65536,ordinary_worker_linear_bytes:65536}])assert.throws(()=>checkedResource(bad));
});
