// API-0011 / TC-0036: these channel stubs prove translation, not database semantics.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createRuntime,Database,openDatabase,defineSchema,j,query} from '../src/index.js';
import {copyOwned,immutableOwned} from '../src/values.js';
import {utf8Length} from '../src/utf8.js';
import {openMemoryDatabase} from '../../testing/src/index.js';
const reply=value=>JSON.stringify({abi:'1',ok:true,value});
const namespace=()=>new Uint8Array(16).fill(3);
import {schemas} from './schemas.mjs';
function channel(){const frames=[];return {frames,open:async(input,key)=>{frames.push({frame:JSON.parse(input),key});return reply('1');},call:async input=>{const frame=JSON.parse(input);frames.push({frame});return reply(null);}};}
const options=(runtime,schema=schemas[0])=>({runtime,schema,engine:{kind:'opfs',name:'general.sqlite'},migrations:[],encryptionKey:new Uint8Array(32).fill(11)});
function withoutCapabilities(work){const names=['structuredClone','crypto','TextEncoder','TextDecoder','BigInt'];const old=names.map(name=>[name,Object.getOwnPropertyDescriptor(globalThis,name)]);try{for(const name of names)Object.defineProperty(globalThis,name,{configurable:true,writable:true,value:undefined});return work();}finally{for(const [name,descriptor]of old)if(descriptor)Object.defineProperty(globalThis,name,descriptor);else delete globalThis[name];}}

test('G03 portable integer/float tags preserve exact kinds without optional globals',()=>withoutCapabilities(()=>{
 assert.deepEqual(j.integer('-9223372036854775808'),{kind:'integer',value:'-9223372036854775808'});
 assert.deepEqual(j.integer('9223372036854775807'),{kind:'integer',value:'9223372036854775807'});
 assert.deepEqual(j.float('42.0'),{kind:'float',value:'42.0'});
 assert.deepEqual(j.timestamp('0'),{kind:'timestamp',value:'0'});
 assert.throws(()=>j.integer(9007199254740992),{kind:'InvalidRequest'});
 for(const value of ['-0','01','+1','9223372036854775808'])assert.throws(()=>j.integer(value),{kind:'InvalidRequest'});
}));
test('G03 owned plain trees preserve safe own keys and reject unsupported cycles',()=>withoutCapabilities(()=>{
 const input=JSON.parse('{"__proto__":{"token":"original"},"value":["東京\\u0000🌿"]}');const copied=copyOwned(input);input.value[0]='changed';
 assert.equal(copied.value[0],'東京\0🌿');assert.equal(Object.hasOwn(copied,'__proto__'),true);assert.equal({}.token,undefined);
 const cycle={};cycle.self=cycle;assert.throws(()=>copyOwned(cycle),{kind:'InvalidRequest'});
}));
test('G06 owned context rejects strict and non-strict nested mutation',()=>{
 const input={identity:{nested:['trusted']}};const snapshot=immutableOwned(input);input.identity.nested[0]='outside';
 assert.equal(snapshot.identity.nested[0],'trusted');assert.throws(()=>{snapshot.identity.nested[0]='inside';},TypeError);
 assert.throws(()=>Function('value','value.identity = {};')(snapshot),TypeError);
});
test('G03 strict UTF8 length works without TextEncoder and rejects lone surrogates',()=>withoutCapabilities(()=>{
 assert.equal(utf8Length('東京\0🌿\n'),12);assert.throws(()=>utf8Length('\ud800'),{kind:'InvalidPayload'});assert.throws(()=>utf8Length('\udc00'),{kind:'InvalidPayload'});
}));
test('G01 schema definition owns arbitrary A and B instead of installing fixture tables',()=>{
 for(const input of schemas){const schema=defineSchema(input);assert.deepEqual(schema,input);assert.notEqual(schema,input);assert.ok(Object.isFrozen(schema.tables));}
});
test('G04 query construction is lazy exact DTO translation including navigation source',()=>{
 const q=query('articles').where('score','Gt',{kind:'integer',value:'42'}).whereIn('title',[]).orderBy('score','Desc').limit('2');
 assert.deepEqual(q.toJSON(),{table:'articles',predicates:[{field:'score',operator:'Gt',values:[{kind:'integer',value:'42'}]},{field:'title',operator:'In',values:[]}],order:{field:'score',direction:'Desc'},limit:'2',source:null});
 assert.deepEqual(query('tags',{table:'articles',id:'a1',relation:'tags'}).toJSON().source,{table:'articles',id:'a1',relation:'tags'});
});
test('G01/G16 exact general open carries caller schema and binary key outside ABI JSON',async()=>{
 for(const schema of schemas){const transport=channel(),runtime=createRuntime(transport,{namespace});const opts=options(runtime,schema),key=opts.encryptionKey;const db=await openDatabase(opts);const sent=transport.frames[0];
  assert.deepEqual(sent.frame,{abi:'1',op:'open',owner:null,args:{engine:{kind:'opfs',name:'general.sqlite'},schema,migrations:[],namespace:'03'.repeat(16),options:{capacity:'1000',history:'100',initial_delay_ms:'1000',multiplier:'2',max_delay_ms:'60000'}}});
  assert.equal(sent.key.byteLength,32);assert.notEqual(sent.key,key);assert.deepEqual(key,new Uint8Array(32).fill(11));assert.equal(sent.frame.args.key,undefined);assert.equal(sent.frame.args.key_hex,undefined);assert.equal(db.owner,'1');await db.close();
 }
});
test('G16 invalid protected key rejects before any channel open',async()=>{
 const transport=channel(),runtime=createRuntime(transport,{namespace}),db=new Database(runtime);
 for(const encryptionKey of [undefined,new Uint8Array(31),new Uint8Array(33),new Uint16Array(32)])await assert.rejects(db.open({...options(runtime),encryptionKey}),{kind:'InvalidRequest'});
 assert.equal(transport.frames.length,0);
});
test('G20 explicitly preloaded memory helper bypasses entropy and protected storage',async()=>{
 const transport=channel(),runtime=createRuntime(transport,{namespace:()=>{throw new Error('entropy must not run');}});
 const db=await openMemoryDatabase({runtime,schema:schemas[1],namespace:'07'.repeat(16)});const sent=transport.frames[0];
 assert.deepEqual(sent.frame.args.engine,{kind:'memory',store:null});assert.equal(sent.key.byteLength,0);assert.equal(sent.frame.args.namespace,'07'.repeat(16));await db.close();
});
test('G03 pure core codecs use null-owner ABI without parsing opaque numeric JSON',async()=>{
 const frames=[],runtime=createRuntime({open(){throw new Error('no open');},async call(input){const f=JSON.parse(input);frames.push(f);return reply(f.op==='json_decode'?{kind:'integer',value:'9223372036854775807'}:'9223372036854775807');}},{namespace});
 assert.deepEqual(await runtime.decodeJson('9223372036854775807'),{kind:'integer',value:'9223372036854775807'});assert.equal(await runtime.encodeJson({kind:'integer',value:'9223372036854775807'}),'9223372036854775807');
 assert.deepEqual(frames.map(x=>[x.abi,x.op,x.owner]),[['1','json_decode',null],['1','json_encode',null]]);
});
test('G22 retained-memory helper calls preserve typed handle ownership',async()=>{
 const frames=[],runtime=createRuntime({open(){throw new Error('not yet');},async call(input){const frame=JSON.parse(input);frames.push(frame);return reply(frame.op==='memory_store'?'19':null);}},{namespace});
 const store=await runtime.createMemoryStore();assert.equal(store,'19');await runtime.dropMemoryStore(store);assert.deepEqual(frames,[{abi:'1',op:'memory_store',owner:null,args:{}},{abi:'1',op:'memory_drop',owner:null,args:{store:'19'}}]);
});
test('G09 pending open reserves one client and closes late owner before replacement',async()=>{
 let release,dispose;const frames=[];const runtime=createRuntime({async open(input,key){frames.push(JSON.parse(input));return new Promise(resolve=>{release=()=>resolve(reply('1'));});},async call(input){const frame=JSON.parse(input);frames.push(frame);if(frame.op==='close')return new Promise(resolve=>{dispose=()=>resolve(reply(null));});return reply(null);}},{namespace});
 const db=new Database(runtime),first=db.open(options(runtime)).then(()=>null,error=>error);
 try{await assert.rejects(db.open(options(runtime)),{kind:'Busy'});const closing=db.close();release();assert.equal((await first)?.kind,'Closed');await new Promise(resolve=>setImmediate(resolve));assert.equal(db.owner,null);assert.equal(frames.filter(x=>x.op==='close').length,1);dispose();await closing;}finally{release?.();dispose?.();await first;await db.close().catch(()=>{});}
});
