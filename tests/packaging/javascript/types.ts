import {defineSchema,j,query,createRuntime,Database,type Schema,type Cell,type JsonValue,type Scope} from '@hoyadb/client/host';
import {createWebRuntime} from '@hoyadb/client';
import init,{Facade,initStorage} from '@hoyadb/client/wasm';
void init;void Facade;void initStorage;
import {openMemoryDatabase,createClock,createFakeTransport} from '@hoyadb/testing';
const schema:Schema=defineSchema({version:'1',tables:{meters:{columns:{name:{kind:'String',optional:false}},relations:{}}}});
const clock=createClock('1000');clock.set('1001');
const runtime=createRuntime({open:()=>'',call:()=>''},{namespace:()=>new Uint8Array(16)});
const dbPromise=openMemoryDatabase({runtime,schema,namespace:'00'.repeat(16),clock,idGenerator:()=> 'test-fixed-id-001'});
void dbPromise.then(async db=>{const rows=await db.query('meters').where('name','Eq',j.string('one')).orderBy('name','Asc').limit(1).fetch();const id:string=rows[0].id;await db.replace('meters',id,{name:j.string('two')});await db.execute({name:'one-v1',arguments:{},reference_paths:[]},j.object({}),async(scope:Scope)=>{const captured:JsonValue=scope.context.identity;await scope.insert('meters',{name:j.string('async')});return captured;});});
const fake=createFakeTransport({runtime,handlers:{'one-v1':()=>({status:'applied',result:j.integer('9223372036854775807')})}});void fake;
const cell:Cell=j.timestamp('-9223372036854775808');void cell;void createWebRuntime;void Database;void query;
// @ts-expect-error raw unsafe Numbers are not Cell DTOs
const invalid:Cell=9007199254740992;
// @ts-expect-error queries expose exact core operators
query('meters').where('name','contains',j.string('one'));
// @ts-expect-error schemas require known primitive kinds
const bad:Schema={version:'1',tables:{bad:{columns:{bad:{kind:'Decimal',optional:false}},relations:{}}}};
// DEF-0026: an actual DOM AbortController signal remains a valid execute cancellation option.
const controller=new AbortController();
void dbPromise.then(db=>db.execute({name:'one-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.null(),{signal:controller.signal}));
// @ts-expect-error cancellation requires listener registration and removal, not only aborted state
void dbPromise.then(db=>db.execute({name:'one-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.null(),{signal:{aborted:true}}));
