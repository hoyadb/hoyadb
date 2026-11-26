import test from 'node:test';
import assert from 'node:assert/strict';
import {Database,createRuntime,j} from '../src/index.js';
import {schemas} from './schemas.mjs';
import {timerDelay} from '../src/time.js';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
async function fixture(options={}){
 const frames=[];let request=10;
 const runtime=createRuntime({open:()=>reply('1'),call:input=>{const f=JSON.parse(input);frames.push(f);let value=null;if(f.op==='request')value=String(++request);if(f.op==='begin')value=true;if(f.op==='scope_args')value=j.object({});if(f.op==='scope_context')value={identity:j.null(),now_ms:'100'};if(f.op==='finish')value={key:'key',result:f.args.result};if(f.op==='drain')value={events:[],has_more:false};return reply(value);}},{namespace:()=>new Uint8Array(16)});
 const db=new Database(runtime,{namespace:'00'.repeat(16),context:()=>j.null(),clock:()=>100,...options});await db.open({schema:schemas[0],engine:{kind:'memory',store:null}});return {db,frames};
}
test('G22 injected IDs are sampled for ordinary/scoped insert and managed links before RPC',async()=>{
 let sampled=0;const {db,frames}=await fixture({idGenerator:()=>`test-fixed-id-${++sampled}`});
 try{await db.insert('authors',{name:j.string('one')});await db.link('articles','a','tags','t');await db.batch([{op:'insert',table:'tags',fields:{label:j.string('two')}}]);
 await db.execute({name:'proposal-v1',arguments:{},reference_paths:[]},j.object({}),async scope=>{await scope.insert('tags',{label:j.string('three')});await scope.link('articles','a','tags','t');return j.null();});
 assert.equal(sampled,5);assert.deepEqual(frames.filter(f=>['insert','link','scope_insert','scope_link'].includes(f.op)).map(f=>f.args.proposed_id),['test-fixed-id-1','test-fixed-id-2','test-fixed-id-4','test-fixed-id-5']);assert.equal(frames.find(f=>f.op==='batch').args.writes[0].proposed_id,'test-fixed-id-3');
 }finally{await db.close();}
});
test('G22 an exhausted generator rejects a whole batch before its single native mutation',async()=>{
 let sampled=0;const {db,frames}=await fixture({idGenerator:()=>++sampled===1?'one':undefined});
 try{await assert.rejects(db.batch([{op:'insert',table:'tags',fields:{}},{op:'insert',table:'tags',fields:{}}]),{kind:'InvalidRequest'});assert.equal(frames.filter(f=>f.op==='batch').length,0);}finally{await db.close();}
});
test('G22 automatic entity writes retain the strict core allocator despite injected ordinary IDs',async()=>{
 const {db,frames}=await fixture({idGenerator:()=>{throw new Error('must not run');}});
 try{await db.entityBatch([{op:'insert',table:'tags',fields:{label:j.string('entity')}}]);assert.equal(frames.find(f=>f.op==='entity_batch').args.writes[0].proposed_id,null);}finally{await db.close();}
});
test('G19 default prepare samples the injected bounded jitter instead of hard-coding zero',async()=>{
 const {db,frames}=await fixture({jitter:()=>2500});try{await db.preparePush();assert.equal(frames.find(f=>f.op==='prepare_push').args.jitter_basis_points,'2500');await db.preparePush(100,0);assert.equal(frames.filter(f=>f.op==='prepare_push')[1].args.jitter_basis_points,'0');}finally{await db.close();}
});
test('G19 timer delay clamps exact i64 differences without BigInt or unsafe Number coercion',()=>{
 assert.equal(timerDelay('9223372036854775807','0'),2147483647);assert.equal(timerDelay('9007199254740993','9007199254740992'),1);assert.equal(timerDelay('1001','999'),2);assert.equal(timerDelay('3','3'),1);assert.equal(timerDelay('2','3'),1);
});
test('G05 an ordinary committed write drains postcommit callbacks once',async()=>{
 const {db,frames}=await fixture();try{await db.insert('authors',{name:j.string('one')});assert.deepEqual(frames.map(f=>f.op),['insert','drain']);}finally{await db.close();}
});
test('G06 command identity must be explicitly provided before allocating a core ticket',async()=>{
 const {db,frames}=await fixture({context:undefined});try{await assert.rejects(db.execute({name:'identity-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.null()),{kind:'InvalidCommand'});assert.equal(frames.filter(frame=>frame.op==='request').length,0);}finally{await db.close();}
});
