import {workerError} from './harness-error.js';
import { ReactiveClient } from './reactive-client.js';
// TC-0029 R13/R15: these callbacks run on the real main thread, after worker extraction.
const field = (name, kind, value) => ({ name, kind, value });
const fields = total => [
  field('active','Boolean',false), field('notes','String',"owned\0🌿 ' --"),
  field('placed_at','Timestamp','-9223372036854775808'), field('price','Float',Number.MAX_VALUE),
  field('status','String','pending'), field('total_cents','Integer',String(total)),
];
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const equal = (actual, expected) => assert(JSON.stringify(actual) === JSON.stringify(expected), `Expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
async function rejected(operation, kind = 'BackendFailure') {
  try { await operation(); } catch(error) { equal(error.kind,kind); return; }
  throw new Error(`Expected ${kind}`);
}
export async function runReactiveChecks({ request, report }) {
  const reply = async message => {const response=await request(message);if(response.error)throw workerError(response.error,'reactive checks/reactive_conformance');return response.value;};
  const shared = await reply({op:'reactive_conformance'});
  equal(shared,['R01','R02','R03','R04','R05','R06','R07','R08','R09','R10','R11 values','R11 operators','R11 relations','R11 validation','R11 names','R12','R15']);
  report('R01–R12/R15 shared reactive and projection Rust assertions',shared);
  let hold=false, extracted, release;
  const transport=async message=>{
    const response=await request(message);
    if(hold && message.op==='reactive_drain') {hold=false;extracted();await new Promise(resolve=>{release=resolve;});}
    return response;
  };
  const client = new ReactiveClient(transport);
  try {
    await client.open();
    const first = await client.insert(fields('9223372036854775807'));
    equal(first.fields,fields('9223372036854775807'));
    assert(!first.fields.some(f=>f.name.startsWith('__hoya_')),'managed field escaped');
    const received=[];let second, acted=false, recursive=false, canceled=0, inserted;
    const one=await client.observe('rows',async event=>{
      assert(typeof window==='object' && typeof document==='object','callback must run on main thread');
      received.push(structuredClone(event));
      if(!acted) {
        acted=true;equal(event.value,[first]);equal(await client.fetch(),[first]);
        event.value[0].fields.length=0;
        await client.unsubscribe(second);
        inserted=await client.insert(fields(2));
        await rejected(()=>client.drain(0));recursive=true;
      }
    });
    second=await client.observe('rows',()=>{canceled++;});
    equal(await client.drain(0),1);equal(received.length,1);equal(canceled,0);equal(recursive,true);
    equal(await client.fetch(),[first,inserted]);
    equal(await client.drain(1),1);equal(received[1].value,[first,inserted]);
    report('R13 callback read/write, owned values, cancellation after extraction and recursive drain', {mainThread:true,initialRows:received[0].value.length,nextDrainRows:received[1].value.length,canceledCallbacks:canceled});
    let survivor=0;
    const throwing=await client.observe('rows',()=>{throw new Error('fixture callback exception');});
    const live=await client.observe('rows',()=>{survivor++;});
    await client.drain(2);equal(survivor,1);equal(client.callbackErrors.length,1);equal(await client.fetch(),[first,inserted]);
    for(const token of [one,throwing,live]) equal(await client.unsubscribe(token),true);
    const before=await client.stats();equal(before.groups,0);equal(before.observers,0);
    await client.insert(fields(3));const after=await client.stats();
    equal(after.projected_queries,before.projected_queries);equal(after.materialized_rows,before.materialized_rows);
    report('R13 exception isolation and final observer release',{callbackErrors:client.callbackErrors.length,survivingCallbacks:survivor,groups:after.groups});
    const counts=[];const count=await client.observe('count',event=>counts.push(event));
    await client.drain(100);equal(counts.map(e=>e.value),[3]);
    const countBefore=await client.stats();
    for(let i=0;i<10;i++)await client.insert(fields(10+i));
    await client.drain(115);equal(counts.length,1);await client.drain(116);equal(counts.map(e=>e.value),[3,13]);
    const countAfter=await client.stats();equal(countAfter.materialized_rows,countBefore.materialized_rows);
    const temporary=await client.insert(fields(40));await client.remove(temporary.id);await client.drain(132);equal(counts.length,2);
    await client.drain(1);equal(counts.length,2);
    await client.failNextProject();const committed=await client.insert(fields(41));
    await client.drain(133);equal(counts.at(-1).kind,'error');equal(counts.at(-1).value.kind,'BackendFailure');
    assert((await client.fetch()).some(row=>row.id===committed.id),'refresh failure relabeled committed write');
    await client.refresh();await client.drain(134);equal(counts.at(-1).kind,'count');equal(counts.at(-1).value,14);
    const unchanged=await client.stats();await rejected(()=>client.observe('columns',()=>{},['missing']),'InvalidQuery');equal(await client.stats(),unchanged);
    await client.unsubscribe(count);
    report('R13 count-only throttle, typed post-commit error and explicit recovery',{counts:counts.map(e=>e.kind==='error'?e.value.kind:e.value),countMaterializations:countAfter.materialized_rows-countBefore.materialized_rows,intervalMs:16});
    let closed=0, afterClose=0;
    await client.observe('rows',async()=>{closed++;await client.close();});
    await client.observe('rows',()=>{afterClose++;});
    await client.drain(150);equal(closed,1);equal(afterClose,0);await rejected(()=>client.fetch());
    report('R13 close from callback suppresses remaining extracted callbacks',{closed,afterClose});
    await client.open();let obsolete=0,current=0;
    const old=await client.observe('rows',()=>{obsolete++;});
    const ready=new Promise(resolve=>{extracted=resolve;});hold=true;const delayed=client.drain(200);await ready;
    await rejected(()=>client.drain(201));
    await client.open();const next=await client.observe('rows',()=>{current++;});assert(old!==next,'owner reused subscription token');
    release();await delayed;equal(obsolete,0);equal(await client.unsubscribe(old),false);
    await client.drain(201);equal(current,1);await client.unsubscribe(next);
    report('R13 replacement generation and concurrent-drain guards',{obsoleteCallbacks:obsolete,currentCallbacks:current,distinctHandles:old!==next});
    const samples=[];let started=null;
    const timed=await client.observe('rows',()=>{if(started!==null)samples.push(performance.now()-started);});
    await client.drain(250);
    for(let i=0;i<5;i++){started=performance.now();await client.insert(fields(50+i));await client.drain(300+i*16);started=null;}
    equal(samples.length,5);await client.unsubscribe(timed);
    report('R15 actual browser callback latency samples',{runtime:navigator.userAgent,origin:location.origin,scheduling:'one explicit drain immediately after each acknowledged write',milliseconds:samples,min:Math.min(...samples),max:Math.max(...samples)});
  } finally {await client.close();}
}

// TC-0031 Q01/Q03/Q05: permanent review-resolution boundary regressions.
export async function runReviewLargeChecks({request,report}) {
  const response=await request({op:'review_large_sets'});
  if(response.error)throw workerError(response.error,'Q01/review_large_sets');
  const result=response.value;
  report('Q01 actual worker oversized query/projection and SAH pool inspection',result);
  equal(result.queryError,null);equal(result.projectError,null);
  equal(result.rows,[{id:'needle',fields:[field('value','Integer','42')]}]);
  equal(result.keys,['needle']);equal(result.poolAfter,result.poolBefore);
  assert(result.operands>33000,'oversized operand partition missing');
}
export async function runReviewFailureChecks({request,report}) {
  const client=new ReactiveClient(request);
  try {
    await client.open('review-mixed.sqlite');
    const rowEvents=[],columnEvents=[],countEvents=[];
    const rows=await client.observe('rows',event=>rowEvents.push(event));
    const columns=await client.observe('columns',event=>columnEvents.push(event),['notes']);
    const count=await client.observe('count',event=>countEvents.push(event));
    await client.drain(0);rowEvents.length=0;columnEvents.length=0;
    await client.failNextQuery();const committed=await client.insert(fields(71));
    await client.drain(16);
    report('Q03 committed mixed row/count failure delivery',{rowEvents:structuredClone(rowEvents),columnEvents:structuredClone(columnEvents),countEvents:structuredClone(countEvents),committed});
    equal(rowEvents.map(e=>e.kind),['error']);equal(columnEvents.map(e=>e.kind),['error']);
    equal(countEvents.map(e=>[e.kind,e.value]),[['count',0],['count',1]]);
    equal(await client.fetch(),[committed]);
    for(const now of [17,18]) {
      await client.failNextQuery();await client.refresh();await client.drain(now);
      equal(rowEvents.length,now-15);equal(columnEvents.length,now-15);
      equal(rowEvents.at(-1).value.kind,'BackendFailure');equal(columnEvents.at(-1).value.kind,'BackendFailure');
      equal(countEvents.length,2);
    }
    const before=await client.stats();
    const anotherCount=[];const another=await client.observe('count',event=>anotherCount.push(event));
    await client.drain(19);equal(anotherCount.map(e=>e.value),[1]);
    await client.failNextQuery();await rejected(()=>client.observe('rows',()=>{}));
    const after=await client.stats();equal(after.observers,before.observers+1);equal(after.groups,before.groups);
    await client.refresh();await client.drain(20);
    equal(rowEvents.at(-1).kind,'rows');equal(rowEvents.at(-1).value,[committed]);
    equal(columnEvents.at(-1).value,[committed]);equal(countEvents.length,2);
    await client.failNextQuery();await client.insert(fields(72));await client.drain(32);
    await client.unsubscribe(rows);await client.unsubscribe(columns);
    const materialized=(await client.stats()).materialized_rows;
    await client.failNextQuery();await client.refresh();await client.drain(33);
    equal((await client.stats()).materialized_rows,materialized);
    equal((await client.stats()).observers,2);
    equal(countEvents.at(-1).value,2);
    await client.failNextProject();await client.insert(fields(73));await client.drain(34);
    equal(countEvents.at(-1).kind,'error');equal(anotherCount.at(-1).kind,'error');
    const explicitErrors=countEvents.length;
    for(const now of [35,36]) {
      await client.failNextProject();await client.refresh();await client.drain(now);
      equal(countEvents.length,explicitErrors+now-34);equal(countEvents.at(-1).value.kind,'BackendFailure');
    }
    await client.refresh();const beforeDeadline=countEvents.length;
    await client.drain(47);equal(countEvents.length,beforeDeadline);
    await client.drain(48);equal(countEvents.at(-1).kind,'count');equal(countEvents.at(-1).value,3);
    report('Q02/Q05 projected retry errors remain observable and retain count deadline',{counts:countEvents.map(e=>e.kind==='error'?e.value.kind:e.value),recoveryDeadline:48});
    await client.unsubscribe(count);await client.unsubscribe(another);
    equal((await client.stats()).groups,0);
    report('Q03/Q05 repeated explicit row errors, exact recovery, valid counts and dirty cancellation',{rowEventKinds:rowEvents.map(e=>e.kind),columnEventKinds:columnEvents.map(e=>e.kind),counts:countEvents.map(e=>e.value),observersAfterCancel:0});
  } finally {await client.close();}
}

export async function runReviewSharedChecks({request,report}) {
  const names=['Q01 large observations/navigation/reverse dependencies','Q01 owner TEMP window/intersection','Q01 oversized all-kind/Null/AND parity','Q02 repeated explicit retry errors','Q03 row/count failure isolation'];
  const cases=[],poolChecks=[],milliseconds=[];
  for(let index=0;index<names.length;index++) {
    const started=performance.now();
    let response;
    try { response=await request({op:'review_conformance',index}); } catch(error) {throw workerError(error,names[index]);}
    if(response.error)throw workerError(response.error,names[index]);
    const value=response.value;
    equal(value.cases,[names[index]]);
    for(const check of value.poolChecks)equal(check.after,check.before);
    cases.push(...value.cases);poolChecks.push(...value.poolChecks);milliseconds.push(performance.now()-started);
    report(`Shared ${names[index]}`,{milliseconds:milliseconds.at(-1),poolChecks:value.poolChecks});
  }
  report('TC-0031 Q01–Q03 shared large-set and failure regressions with SAH inventory',{cases,poolChecks,milliseconds});
}
