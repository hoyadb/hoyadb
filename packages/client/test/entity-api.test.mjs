import test from 'node:test';import assert from 'node:assert/strict';import {Database,createRuntime,j} from '../src/index.js';import {schemas} from './schemas.mjs';import {ApplicationDatabase} from '../src/application.js';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
async function fixture(){const frames=[];let captured=0;const runtime=createRuntime({open:()=>reply('1'),call:input=>{const frame=JSON.parse(input);frames.push(frame);return reply(frame.op==='drain'?{events:[],has_more:false}:frame.op==='reconcile_resolving'?{kind:'completed'}:frame.op==='resolve_entity_id'?'canonical-id':frame.op==='entity_batch_aliased'?{changes:[],aliases:{p:'entity:core'}}:null);}},{namespace:()=>new Uint8Array(16)});const db=new Database(runtime,{namespace:'00'.repeat(16),context:()=>{captured++;return j.object({tenant:j.string('trusted')});},clock:()=>1000,idGenerator:()=>{throw Error('entity must not request an ordinary ID');}});await db.open({schema:schemas[0],engine:{kind:'memory',store:null}});return {db,frames,captured:()=>captured};}
test('G17 managed entity links and unlink forward one trusted capture and no ID proposal',async()=>{const {db,frames,captured}=await fixture();try{await db.entityLink('articles','one','tags','two',{label:j.string('join')});await db.entityUnlink('articles','one','tags','two');assert.equal(captured(),2);for(const frame of frames.filter(f=>['entity_link','entity_unlink'].includes(f.op))){assert.deepEqual(frame.args.context,{identity:j.object({tenant:j.string('trusted')}),now_ms:'1000'});assert.equal('proposed_id'in frame.args,false);}assert.equal(frames.filter(f=>f.op==='drain').length,2);}finally{await db.close();}});
test('G17 receipt outcome and reconciliation methods preserve explicit core correlation shapes',async()=>{const {db,frames}=await fixture();try{await db.receive('key',null);await db.receiveEntity('articles','one','write',{status:'applied',result:j.integer('9223372036854775807')});await db.reconfigureEntities('next',['articles']);assert.deepEqual(frames.filter(f=>f.op!=='drain').map(f=>[f.op,f.args]),[['receive',{key:'key',outcome:null}],['receive_entity',{table:'articles',record_id:'one',write_id:'write',outcome:{status:'applied',result:j.integer('9223372036854775807')}}],['entity_reconfigure',{config:{scope:'next',tables:['articles']}}]]);}finally{await db.close();}});
test('G19 explicit local reconciliation wakes delivery after the actual committed call',async()=>{const {db,frames}=await fixture();try{await db.reconcile('key',{writes:[],mappings:{}});assert.deepEqual(frames.map(f=>f.op),['reconcile_resolving','drain']);}finally{await db.close();}});
test('A06 aliased batches normalize explicit references and capture one context',async()=>{const {db,frames,captured}=await fixture();try{
 const writes=[{op:'insert',table:'articles',alias:'p',fields:{title:j.string('parent')}},{op:'insert',table:'articles',fields:{title:j.string('child')},references:{parent_id:'p'}},{op:'replace',table:'articles',id:'existing',fields:{title:j.string('changed')}},{op:'remove',table:'articles',id:'removed'}];
 const result=await db.entityBatchAliased(writes);
 assert.deepEqual(result,{changes:[],aliases:{p:'entity:core'}});
 assert.equal(captured(),1);
 assert.deepEqual(frames[0],{abi:'1',op:'entity_batch_aliased',owner:'1',args:{writes:[{...writes[0],proposed_id:null,references:{}},{...writes[1],proposed_id:null,alias:null},{...writes[2],references:{}},writes[3]],context:{identity:j.object({tenant:j.string('trusted')}),now_ms:'1000'}}});
 assert.deepEqual(frames.map(f=>f.op),['entity_batch_aliased','drain']);
 assert.equal('proposed_id'in writes[0],false);
}finally{await db.close();}});
test('A07 aliased patch forwards only supplied fields',async()=>{const {db,frames}=await fixture();try{
 const fields={title:j.string('same-value')};await db.entityBatchAliased([{op:'patch',table:'articles',id:'existing',fields}]);
 assert.deepEqual(frames[0].args.writes,[{op:'patch',table:'articles',id:'existing',fields,references:{}}]);
}finally{await db.close();}});

test('D06 durable configuration and ID resolution use the shared facade without mutation context',async()=>{const {db,frames,captured}=await fixture();try{
 await db.configureEntitiesDurable('durable',['articles']);
 assert.equal(await db.resolveEntityId('articles','original-id'),'canonical-id');
 assert.equal(captured(),0);
 assert.deepEqual(frames.map(({op,args})=>({op,args})),[{op:'entity_configure_durable',args:{config:{scope:'durable',tables:['articles']}}},{op:'resolve_entity_id',args:{table:'articles',id:'original-id'}}]);
}finally{await db.close();}});
test('D06 application delegates durable configuration and resolution, retaining configuration on failure',async()=>{
 const calls=[];let reject=false;const raw={async configureEntitiesDurable(scope,tables){calls.push(['configure',scope,tables]);if(reject)throw Error('configure rejected');},async resolveEntityId(table,id){calls.push(['resolve',table,id]);return 'canonical-id';}};
 const app=new ApplicationDatabase({},raw,{});const tables=['articles'];
 await app.configureEntitiesDurable('durable',tables);tables.push('later');
 assert.deepEqual(app.entities,{scope:'durable',tables:['articles']});
 assert.equal(await app.resolveEntityId('articles','original-id'),'canonical-id');
 reject=true;await assert.rejects(app.configureEntitiesDurable('rejected',['other']),/configure rejected/);
 assert.deepEqual(app.entities,{scope:'durable',tables:['articles']});
 assert.deepEqual(calls.slice(1),[['resolve','articles','original-id'],['configure','rejected',['other']]]);
});
