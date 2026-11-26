// API-0012: brand() is a mirrored pre-check; the core schema validator stays authoritative.
// Every case runs through both brand() and a real WASM open. A core rule change that makes
// the two disagree on a mirrored rule, or makes brand() refuse a schema core accepts, fails here.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import init,{Facade} from '../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {Database,createRuntime} from '../src/index.js';
import {applicationSchema} from '../src/application.js';
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});

const col=(kind,optional=false)=>({kind,optional});
const one=(table,foreign_key,optional=false)=>({kind:'ToOne',table,foreign_key,optional});
const many=(table,foreign_key)=>({kind:'HasMany',table,foreign_key});
const tables=(extra={})=>({authors:{columns:{name:col('String')},relations:{}},...extra});
const posts=(columns,relations)=>tables({posts:{columns:{title:col('String'),...columns},relations}});
const join={kind:'ManyToMany',table:'tags',through:'post_tags',source_relation:'post',target_relation:'tag'};
const tagged=(through,relation=join)=>tables({posts:{columns:{title:col('String')},relations:{tags:relation}},tags:{columns:{label:col('String')},relations:{}},...through});
// [name, schema, brand accepts, core accepts]. `mirrored` rows are rules brand() repeats;
// `core-only` rows are rules brand() leaves to core (accepted locally, refused at open).
export const cases=[
 ['valid: plain tables',{version:'1',tables:tables()},true,true],
 ['valid: no tables',{version:'7',tables:{}},true,true],
 ['valid: to-one with inferred foreign key',{version:'1',tables:posts({},{author:one('authors','author_id')})},true,true],
 ['valid: to-one with explicit matching Reference column',{version:'1',tables:posts({author_id:col('Reference')},{author:one('authors','author_id')})},true,true],
 ['valid: optional to-one with optional Reference column',{version:'1',tables:posts({author_id:col('Reference',true)},{author:one('authors','author_id',true)})},true,true],
 ['valid: two aliases agreeing on one foreign key',{version:'1',tables:posts({},{author:one('authors','author_id'),writer:one('authors','author_id')})},true,true],
 ['valid: has-many over a declared to-one',{version:'1',tables:{authors:{columns:{name:col('String')},relations:{posts:many('posts','author_id')}},posts:{columns:{title:col('String')},relations:{author:one('authors','author_id')}}}},true,true],
 ['valid: many-to-many through explicit to-ones',{version:'1',tables:tagged({post_tags:{columns:{},relations:{post:one('posts','post_id'),tag:one('tags','tag_id')}}})},true,true],
 ['mirrored: version zero',{version:'0',tables:tables()},false,false],
 ['mirrored: version above u32',{version:'4294967296',tables:tables()},false,false],
 ['mirrored: table named id',{version:'1',tables:{id:{columns:{},relations:{}}}},false,false],
 ['mirrored: dotted table name',{version:'1',tables:{'a.b':{columns:{},relations:{}}}},false,false],
 ['mirrored: reserved table prefix',{version:'1',tables:{__hoya_x:{columns:{},relations:{}}}},false,false],
 ['mirrored: empty table name',{version:'1',tables:{'':{columns:{},relations:{}}}},false,false],
 ['mirrored: column named id',{version:'1',tables:posts({id:col('String')},{})},false,false],
 ['mirrored: dotted column name',{version:'1',tables:posts({'a.b':col('String')},{})},false,false],
 ['mirrored: reserved column prefix',{version:'1',tables:posts({__hoya_x:col('String')},{})},false,false],
 ['mirrored: empty column name',{version:'1',tables:posts({'':col('String')},{})},false,false],
 ['mirrored: relation named id',{version:'1',tables:posts({},{id:one('authors','author_id')})},false,false],
 ['mirrored: dotted relation name',{version:'1',tables:posts({},{'a.b':one('authors','author_id')})},false,false],
 ['mirrored: aliases disagree on target',{version:'1',tables:posts({},{author:one('authors','ref_id'),self:one('posts','ref_id')})},false,false],
 ['mirrored: aliases disagree on optionality',{version:'1',tables:posts({},{author:one('authors','author_id'),writer:one('authors','author_id',true)})},false,false],
 ['mirrored: explicit foreign key column is not a Reference',{version:'1',tables:posts({author_id:col('String')},{author:one('authors','author_id')})},false,false],
 ['mirrored: explicit foreign key column optionality differs',{version:'1',tables:posts({author_id:col('Reference',true)},{author:one('authors','author_id')})},false,false],
 ['mirrored: relation shadows a declared column',{version:'1',tables:posts({author:col('String')},{author:one('authors','author_id')})},false,false],
 ['mirrored: relation shadows an inferred foreign key',{version:'1',tables:posts({},{author:one('authors','writer'),writer:one('authors','other_id')})},false,false],
 ['core-only: dotted foreign key in a raw relation DTO',{version:'1',tables:posts({},{author:one('authors','a.b')})},true,false],
 ['core-only: foreign key named id in a raw relation DTO',{version:'1',tables:posts({},{author:one('authors','id')})},true,false],
 ['core-only: to-one target table is undeclared',{version:'1',tables:posts({},{author:one('missing','author_id')})},true,false],
 ['core-only: has-many target table is undeclared',{version:'1',tables:{authors:{columns:{},relations:{posts:many('missing','author_id')}}}},true,false],
 ['core-only: has-many without the inverse foreign key',{version:'1',tables:{authors:{columns:{},relations:{posts:many('posts','author_id')}},posts:{columns:{},relations:{}}}},true,false],
 ['core-only: many-to-many through table is undeclared',{version:'1',tables:tagged({})},true,false],
 ['core-only: many-to-many through relation is missing',{version:'1',tables:tagged({post_tags:{columns:{},relations:{post:one('posts','post_id')}}})},true,false],
 ['core-only: many-to-many source relation points elsewhere',{version:'1',tables:tagged({post_tags:{columns:{},relations:{post:one('tags','post_id'),tag:one('tags','tag_id')}}})},true,false],
];

async function core(schema){
 const facade=new Facade();
 try{
  const runtime=createRuntime({open:(input,key)=>facade.open(input,key),call:input=>facade.call(input)},{namespace:()=>new Uint8Array(16).fill(9)});
  const db=new Database(runtime,{namespace:'09'.repeat(16)});
  try{await db.open({schema,engine:{kind:'memory',store:null}});}catch(error){return {accepted:false,kind:error.kind};}
  await db.close();return {accepted:true};
 }finally{facade.free();}
}
function brand(schema){try{applicationSchema(schema);return {accepted:true};}catch(error){return {accepted:false,kind:error.kind};}}

for(const [name,schema,local,authoritative] of cases)test('SC '+name,async()=>{
 const js=brand(schema),rust=await core(schema);
 if(!js.accepted)assert.equal(js.kind,'InvalidSchema');
 if(!rust.accepted)assert.ok(['InvalidSchema','InvalidRequest'].includes(rust.kind),'core refuses statically: '+rust.kind);
 // Core stays authoritative: brand() never refuses a schema that core accepts.
 if(rust.accepted)assert.equal(js.accepted,true,'brand() refused a schema core accepts');
 assert.deepEqual([js.accepted,rust.accepted],[local,authoritative],'brand/core acceptance changed');
});
