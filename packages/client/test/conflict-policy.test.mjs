// TC-0044 CP13: controlled facade replies prove host lifecycle and reporting.
import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime} from '../src/index.js';
import {schemas} from './schemas.mjs';
const ok=value=>JSON.stringify({abi:'1',ok:true,value});
const bad=kind=>JSON.stringify({abi:'1',ok:false,error:{kind}});
const settle=()=>new Promise(resolve=>setImmediate(resolve));
const gate=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});return {promise,resolve};};
const kept=()=>({kind:'kept_current',discarded_keys:['k','child'],unmapped:['local:id'],skipped_writes:'18446744073709551615'});
const plan={writes:[],mappings:{}};
const openOptions={schema:schemas[1],engine:{kind:'memory',store:null}};
function fixture(options={},handler=()=>undefined){
 const frames=[],facts=[];let opens=0,closes=0;
 const runtime=createRuntime({open:()=>ok(String(++opens)),call:async input=>{
  const frame=JSON.parse(input);frames.push(frame);if(frame.op==='close')closes++;
  const result=await handler(frame);if(result!==undefined)return result;
  if(frame.op==='drain')return ok({events:[],has_more:false});
  if(['reconcile_resolving','rollback_resolving','discard'].includes(frame.op))return ok(kept());
  return ok(null);
 }});
 const db=new Database(runtime,{namespace:'00'.repeat(16),onBackgroundError:fact=>facts.push(fact),...options});
 return {db,frames,facts,get opens(){return opens;},get closes(){return closes;}};
}

test('CP13 policy is installed before owner publication and reapplied on reopen',async()=>{
 const release=gate();const f=fixture({conflict:{default:'discard'}},async frame=>{
  if(frame.op==='set_conflict_policy'){await release.promise;return ok(null);}
 });
 let finished=false;const opening=f.db.open(openOptions).then(()=>{finished=true;});
 await settle();
 try{assert.equal(finished,false,'open must await policy installation');assert.equal(f.db.owner,null);
  assert.deepEqual(f.frames.find(x=>x.op==='set_conflict_policy')?.args,{policy:{default:'discard',commands:{}}});
 }finally{release.resolve();await opening;await f.db.close();}
 await f.db.open(openOptions);await f.db.close();
 assert.equal(f.frames.filter(x=>x.op==='set_conflict_policy').length,2);
});

test('CP13 configuration failure disposes the unpublished owner and allows reopen',async()=>{
 let fail=true;const f=fixture({conflict:{default:'discard'}},frame=>frame.op==='set_conflict_policy'&&fail?bad('InvalidRequest'):undefined);
 try{await assert.rejects(f.db.open(openOptions),{kind:'InvalidRequest'});assert.equal(f.db.owner,null);assert.equal(f.closes,1);
  fail=false;await f.db.open(openOptions);assert.equal(f.db.owner,'2');
 }finally{await f.db.close();}
});

test('CP13 close during configuration waits, disposes once and never publishes',async()=>{
 const release=gate();const f=fixture({conflict:{default:'discard'}},frame=>frame.op==='set_conflict_policy'?release.promise.then(()=>ok(null)):undefined);
 const opening=f.db.open(openOptions);const outcome=opening.then(()=>null,error=>error.kind);await settle();
 const closing=f.db.close();await settle();
 try{assert.equal(f.closes,0,'do not race disposal with configuration');assert.equal(f.db.owner,null);}
 finally{release.resolve();await closing;}
 assert.equal(await outcome,'Closed');assert.equal(f.closes,1);assert.equal(f.db.owner,null);
});

test('CP13 legacy unit operations use resolving calls and notify exactly once per KeptCurrent',async()=>{
 const calls=[];const f=fixture({onResolution:(key,value)=>calls.push([key,value])});await f.db.open(openOptions);
 try{assert.equal(await f.db.reconcile('a',plan),null);assert.equal(await f.db.reconcileGuarded('b',plan,[]),null);assert.equal(await f.db.rollback('c'),null);
  assert.deepEqual(calls.map(x=>x[0]),['a','b','c']);
  assert.deepEqual(f.frames.filter(x=>x.op.endsWith('_resolving')).map(x=>[x.op,x.args]),[
   ['reconcile_resolving',{key:'a',plan,guards:[]}],['reconcile_resolving',{key:'b',plan,guards:[]}],['rollback_resolving',{key:'c'}]]);
 }finally{await f.db.close();}
});

test('CP13 callback exceptions are redacted, contained and do not uninstall the callback',async()=>{
 let calls=0;const f=fixture({onResolution:()=>{calls++;throw new Error('private command key');}});await f.db.open(openOptions);
 try{assert.equal(await f.db.rollback('a'),null);assert.equal(await f.db.rollback('b'),null);assert.equal(calls,2);
  assert.deepEqual(f.facts,[{source:'callback',kind:'CallbackFailure',token:null},{source:'callback',kind:'CallbackFailure',token:null}]);
  assert.deepEqual(f.db.syncErrors,[]);
 }finally{await f.db.close();}
});

test('CP13 returned promises are not awaited and each rejection is reported once',async()=>{
 const rejectors=[];const f=fixture({onResolution:()=>new Promise((_,reject)=>rejectors.push(reject))});await f.db.open(openOptions);
 try{await f.db.rollback('a');await f.db.rollback('b');assert.equal(rejectors.length,2);
  for(const reject of rejectors)reject(new Error('private'));await settle();assert.equal(f.facts.length,2);
 }finally{await f.db.close();}
});

test('CP13 a hostile thenable settles once and callback remains installed',async()=>{
 let calls=0;const f=fixture({onResolution:()=>{calls++;return {then(_yes,no){no('one');no('two');throw new Error('three');}};}});await f.db.open(openOptions);
 try{await f.db.rollback('a');await settle();await f.db.rollback('b');await settle();assert.equal(calls,2);assert.equal(f.facts.length,2);}
 finally{await f.db.close();}
});

test('CP13 resolution is owned and immutable, including full-u64 counts',async()=>{
 const calls=[];const f=fixture({onResolution:(key,value)=>{calls.push(value);assert.throws(()=>value.discarded_keys.push('mutated'),TypeError);}});await f.db.open(openOptions);
 try{await f.db.rollback('a');assert.deepEqual(calls,[kept()]);assert.ok(Object.isFrozen(calls[0]));assert.ok(Object.isFrozen(calls[0].discarded_keys));}
 finally{await f.db.close();}
});

test('CP13 malformed resolution fails closed before callbacks',async()=>{
 let reply;const calls=[];const f=fixture({onResolution:(...args)=>calls.push(args)},frame=>frame.op==='rollback_resolving'?ok(reply):undefined);await f.db.open(openOptions);
 try{for(const value of [null,{kind:'completed',extra:true},{...kept(),skipped_writes:'18446744073709551616'},{...kept(),skipped_writes:'01'},{...kept(),unmapped:[null]},{...kept(),discarded_keys:'k'}]){
   reply=value;await assert.rejects(f.db.rollback('a'),{kind:'InvalidPayload'});
  }
  // Runtime rejects numeric wire values before the resolution decoder runs.
  reply={...kept(),skipped_writes:1};await assert.rejects(f.db.rollback('a'),{kind:'InvalidRequest'});assert.deepEqual(calls,[]);
 }finally{await f.db.close();}
});

test('CP13 a failed configuration close retains the same private owner for retry',async()=>{
 let closeFails=true;const f=fixture({conflict:{default:'discard'}},frame=>frame.op==='set_conflict_policy'?bad('InvalidRequest'):frame.op==='close'&&closeFails?bad('BackendFailure'):undefined);
 await assert.rejects(f.db.open(openOptions),{kind:'BackendFailure'});assert.equal(f.db.owner,null);
 await assert.rejects(f.db.open(openOptions),{kind:'Busy'});assert.equal(f.opens,1);
 closeFails=false;await f.db.close();assert.equal(f.closes,2);
 assert.deepEqual(f.frames.filter(x=>x.op==='close').map(x=>x.owner),['1','1']);
});

test('CP13 explicit methods return the same owned report and policy updates remain owner-local',async()=>{
 const calls=[];const f=fixture({onResolution:key=>calls.push(key)});await f.db.open(openOptions);
 try{assert.deepEqual(await f.db.reconcileResolving('a',plan,[]),kept());assert.deepEqual(await f.db.rollbackResolving('b'),kept());assert.deepEqual(await f.db.discard('c'),kept());
  assert.deepEqual(calls,['a','b','c']);assert.equal(await f.db.setConflictPolicy({default:'stop',commands:{'edit@1':'discard'}}),null);
  assert.deepEqual(f.frames.find(x=>x.op==='set_conflict_policy').args,{policy:{default:'stop',commands:{'edit@1':'discard'}}});
 }finally{await f.db.close();}
 await f.db.open(openOptions);await f.db.close();assert.equal(f.frames.filter(x=>x.op==='set_conflict_policy').length,1,'runtime setter does not become constructor configuration');
});

test('CP13 explicit null command map is rejected, never normalized to an empty map',async()=>{
 const f=fixture({},frame=>frame.op==='set_conflict_policy'&&frame.args.policy.commands===null?bad('InvalidRequest'):undefined);await f.db.open(openOptions);
 try{await assert.rejects(f.db.setConflictPolicy({default:'discard',commands:null}),{kind:'InvalidRequest'});}
 finally{await f.db.close();}
});

test('CP13 invalid policy DTO shapes fail before acquiring an owner',async()=>{
 for(const conflict of [{commands:{}},{default:'maybe'},{default:'stop',commands:null},{default:'stop',commands:[]},{default:'stop',commands:{'edit-v1':'maybe'}}]){
  const f=fixture({conflict},frame=>frame.op==='set_conflict_policy'?bad('InvalidRequest'):undefined);
  await assert.rejects(f.db.open(openOptions),{kind:'InvalidRequest'});assert.equal(f.opens,0,'invalid host DTO must not acquire an owner');assert.equal(f.closes,0);
 }
});

test('CP13 managed sync invokes onResolution once without treating callback failure as sync failure',async()=>{
 let pending=true,calls=0;const f=fixture({jitter:()=>0,onResolution:()=>{calls++;throw new Error('callback');}},frame=>{
  if(frame.op==='prepare_push')return ok({envelopes:[],has_more:false,next_due_ms:null});
  if(frame.op==='pending_reconciliations')return ok(pending?[{key:'a',outcome:{status:'rejected',reason:'server'}}]:[]);
  if(frame.op==='reconcile_resolving'){pending=false;return ok(kept());}
 });await f.db.open(openOptions);
 try{f.db.startSync(async()=>'[]',()=>plan);clearTimeout(f.db.timer);f.db.timer=null;await f.db.sync();f.db.stopSync();
  assert.equal(calls,1);assert.deepEqual(f.facts,[{source:'callback',kind:'CallbackFailure',token:null}]);assert.deepEqual(f.db.syncErrors,[]);
  assert.equal(f.frames.filter(x=>x.op==='reconcile_resolving').length,1);
 }finally{f.db.stopSync();await f.db.close();}
});

test('CP13 inline retirement suppresses callback-failure facts from that invocation',async()=>{
 const retired=[];const f=fixture({onResolution:()=>{retired.push(f.db.close());throw new Error('retired');}});await f.db.open(openOptions);
 await f.db.rollback('a');await Promise.all(retired);assert.deepEqual(f.facts,[]);assert.equal(f.db.owner,null);
});

test('CP13 Completed does not notify and retired epochs suppress results and late failures',async()=>{
 for(const retire of ['close','reset']){
  let reply={kind:'completed'},hold=null;const calls=[],rejectors=[];
  const f=fixture({onResolution:(key)=>{calls.push(key);return new Promise((_,reject)=>rejectors.push(reject));}},frame=>frame.op==='rollback_resolving'?(hold?hold.promise:ok(reply)):undefined);
  await f.db.open(openOptions);
  try{await f.db.rollback('completed');assert.deepEqual(calls,[]);reply=kept();await f.db.rollback('before');assert.deepEqual(calls,['before']);
   hold=gate();const pending=f.db.rollback('old');await settle();await f.db[retire]();hold.resolve(ok(kept()));await pending;
   rejectors[0](new Error('late'));await settle();assert.deepEqual(calls,['before']);assert.deepEqual(f.facts,[]);
  }finally{hold?.resolve(ok(kept()));await f.db.close();}
 }
});
