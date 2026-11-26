// API-0012: application value translation for the JavaScript binding. These are pure translators
// between plain application values and API-0011 tagged wire values; no storage, query or sync
// algorithm lives here. Per-column host policies are kept in WeakMaps, never as wire-schema keys.
import {check,failure} from './errors.js';
import {utf8Length} from './utf8.js';
import {MAX_BYTES,integer} from './values.js';

const MAX_DEPTH=64,MAX_NODES=100000,DATE_LIMIT=8640000000000000;
const KINDS={string:'String',integer:'Integer',float:'Float',boolean:'Boolean',timestamp:'Timestamp',reference:'Reference'};
const CELL={String:'string',Integer:'integer',Float:'float',Boolean:'boolean',Timestamp:'timestamp',Reference:'reference'};

const applicationSchemas=new WeakSet(); // schemas produced by defineApplicationSchema/applicationSchema
const applicationTables=new WeakSet();  // table() results
const columnPolicies=new WeakMap();      // column() result -> {bigint}
const schemaMetadata=new WeakMap();      // branded schema -> Map(table -> {columns, foreignKeys, relations})

const own=(target,key,value)=>Object.defineProperty(target,key,{value,enumerable:true,writable:true,configurable:true});
const isPlain=value=>{if(!value||typeof value!=='object'||Array.isArray(value))return false;const proto=Object.getPrototypeOf(value);return proto===Object.prototype||proto===null;};
// Own enumerable data entries of a plain object; accessors and symbol keys are rejected.
function entries(value,kind){
 check(isPlain(value),kind);check(Object.getOwnPropertySymbols(value).length===0,kind);
 return Object.keys(value).map(key=>{const descriptor=Object.getOwnPropertyDescriptor(value,key);check(Object.prototype.hasOwnProperty.call(descriptor,'value'),kind);return [key,descriptor.value];});
}
function name(value){
 check(typeof value==='string'&&value!==''&&value!=='id'&&!value.includes('.')&&!value.startsWith('__hoya_'),'InvalidSchema');
 utf8Length(value,MAX_BYTES);return value;
}
function options(input,allowed){
 if(input===undefined)return {};
 const result={};
 for(const [key,value] of entries(input,'InvalidSchema')){check(allowed.includes(key)&&typeof value==='boolean','InvalidSchema');result[key]=value;}
 return result;
}

// Columns, relations and tables.
function makeColumn(kind){
 return input=>{
  const {optional=false,bigint=false}=options(input,kind==='Integer'||kind==='Timestamp'?['optional','bigint']:['optional']);
  const column=Object.freeze({kind,optional});columnPolicies.set(column,Object.freeze({bigint}));return column;
 };
}
export const column=Object.freeze(Object.fromEntries(Object.entries(KINDS).map(([key,kind])=>[key,makeColumn(kind)])));
function relationOptions(input,required,allowed=required){
 const values=Object.fromEntries(entries(input,'InvalidSchema'));
 for(const key of Object.keys(values))check(allowed.includes(key),'InvalidSchema');
 for(const key of required)check(typeof values[key]==='string','InvalidSchema');
 return values;
}
export const relation=Object.freeze({
 toOne(table,input){const values=relationOptions(input,['foreignKey'],['foreignKey','optional']);check(values.optional===undefined||typeof values.optional==='boolean','InvalidSchema');return Object.freeze({kind:'ToOne',table:name(table),foreign_key:name(values.foreignKey),optional:values.optional??false});},
 hasMany(table,input){const values=relationOptions(input,['foreignKey']);return Object.freeze({kind:'HasMany',table:name(table),foreign_key:name(values.foreignKey)});},
 manyToMany(table,input){const values=relationOptions(input,['through','sourceRelation','targetRelation']);return Object.freeze({kind:'ManyToMany',table:name(table),through:name(values.through),source_relation:name(values.sourceRelation),target_relation:name(values.targetRelation)});}
});
export function table(input){
 const values=Object.fromEntries(entries(input,'InvalidSchema'));
 for(const key of Object.keys(values))check(key==='columns'||key==='relations','InvalidSchema');
 const result=Object.freeze({columns:copyColumns(values.columns),relations:copyRelations(values.relations??{})});
 applicationTables.add(result);return result;
}
function copyColumns(input){
 const result={};
 for(const [key,value] of entries(input,'InvalidSchema')){
  name(key);
  if(columnPolicies.has(value))own(result,key,value);
  else{
   const fields=Object.fromEntries(entries(value,'InvalidSchema'));
   check(Object.keys(fields).sort().join()==='kind,optional'&&Object.values(KINDS).includes(fields.kind)&&typeof fields.optional==='boolean','InvalidSchema');
   const column=Object.freeze({kind:fields.kind,optional:fields.optional});columnPolicies.set(column,Object.freeze({bigint:false}));own(result,key,column);
  }
 }
 return Object.freeze(result);
}
function copyRelations(input){
 const result={};
 for(const [key,value] of entries(input,'InvalidSchema')){
  name(key);const fields=Object.fromEntries(entries(value,'InvalidSchema'));
  const shape={ToOne:'foreign_key,kind,optional,table',HasMany:'foreign_key,kind,table',ManyToMany:'kind,source_relation,table,target_relation,through'}[fields.kind];
  check(shape&&Object.keys(fields).sort().join()===shape,'InvalidSchema');
  for(const [field,item] of Object.entries(fields))check(field==='optional'?typeof item==='boolean':typeof item==='string','InvalidSchema');
  own(result,key,Object.freeze({...fields}));
 }
 return Object.freeze(result);
}
function version(value){
 if(typeof value==='number')check(Number.isSafeInteger(value),'InvalidSchema');
 check(typeof value==='number'||typeof value==='string','InvalidSchema');
 const text=String(value);check(/^[1-9][0-9]*$/.test(text)&&text.length<=10&&Number(text)<=4294967295,'InvalidSchema');return text;
}
function brand(input){
 const values=Object.fromEntries(entries(input,'InvalidSchema'));
 check(Object.keys(values).sort().join()==='tables,version','InvalidSchema');
 const tables={},metadata=new Map();
 for(const [tableName,definition] of entries(values.tables,'InvalidSchema')){
  name(tableName);const copied=applicationTables.has(definition)?definition:table(definition);own(tables,tableName,copied);
 }
 const schema=Object.freeze({version:version(values.version),tables:Object.freeze(tables)});
 for(const [tableName,definition] of Object.entries(schema.tables)){
  const columns=new Map(Object.entries(definition.columns).map(([key,value])=>[key,Object.freeze({kind:value.kind,optional:value.optional,bigint:columnPolicies.get(value).bigint})]));
  // Mirrors core schema normalization: to-one aliases sharing a foreign key agree on target and
  // optionality; an explicit column for that key is a Reference with the same optionality; and no
  // relation name shadows a declared or inferred column.
  const foreignKeys=new Map(),targets=new Map();
  for(const value of Object.values(definition.relations))if(value.kind==='ToOne'){
   const previous=targets.get(value.foreign_key);
   check(!previous||(previous.table===value.table&&previous.optional===value.optional),'InvalidSchema');
   targets.set(value.foreign_key,{table:value.table,optional:value.optional});
   const declared=columns.get(value.foreign_key);
   if(declared)check(declared.kind==='Reference'&&declared.optional===value.optional,'InvalidSchema');
   else foreignKeys.set(value.foreign_key,Object.freeze({kind:'Reference',optional:value.optional,bigint:false}));
  }
  for(const relationName of Object.keys(definition.relations))check(!columns.has(relationName)&&!foreignKeys.has(relationName),'InvalidSchema');
  metadata.set(tableName,{columns,foreignKeys,relations:new Map(Object.entries(definition.relations))});
 }
 applicationSchemas.add(schema);schemaMetadata.set(schema,metadata);return schema;
}
/** Builds an immutable, wire-compatible schema from table()/column() helpers or raw table DTOs. */
export function defineApplicationSchema(input){return brand(input);}
/** Explicitly converts an existing wire schema (string or numeric version) with default policies. */
export function applicationSchema(wireSchema){return brand(wireSchema);}
export function isApplicationSchema(schema){return applicationSchemas.has(schema);}
/** True when a schema input declares at least one table built with the table() helper. */
export function hasApplicationTables(input){
 try{
  const tables=Object.fromEntries(entries(input,'InvalidSchema')).tables;
  return entries(tables,'InvalidSchema').some(([,value])=>applicationTables.has(value));
 }catch{return false;}
}

// Column resolution.
function tableMetadata(schema,tableName){
 check(applicationSchemas.has(schema),'InvalidSchema');
 const metadata=schemaMetadata.get(schema);check(typeof tableName==='string'&&metadata.has(tableName),'TableNotFound');
 return metadata.get(tableName);
}
const ID=Object.freeze({kind:'String',optional:false,bigint:false});
function direct(metadata,field){
 if(field==='id')return ID;
 return metadata.columns.get(field)??metadata.foreignKeys.get(field);
}
/** The column descriptor {kind, optional, bigint} for a field, `id`, a to-one foreign key or `relation.field`. */
export function resolveColumn(schema,tableName,field){
 const metadata=tableMetadata(schema,tableName);check(typeof field==='string','InvalidQuery');
 const parts=field.split('.');check(parts.length<=2,'InvalidQuery');
 if(parts.length===1){const found=direct(metadata,field);check(found,'InvalidQuery');return found;}
 const via=metadata.relations.get(parts[0]);check(via&&via.kind==='ToOne','InvalidQuery');
 const found=direct(tableMetadata(schema,via.table),parts[1]);check(found,'InvalidQuery');return found;
}

// Integer and time values.
function canonical(value,kind){try{return integer(value);}catch{throw failure(kind);}}
const safe=text=>Number.isSafeInteger(Number(text));
function milliseconds_(value,kind){
 if(value instanceof Date){const time=value.getTime();check(Number.isInteger(time),kind);return String(time);}
 check(typeof value==='number'||typeof value==='bigint'||typeof value==='string',kind);
 return canonical(value,kind);
}
/** Canonical signed 64-bit milliseconds from a Date, safe integer, bigint or decimal string. */
export function milliseconds(value){return milliseconds_(value,'InvalidRequest');}
const wide=text=>{check(typeof BigInt==='function','UnsupportedOperation');return BigInt(text);};

/** Encodes one application value for a resolved column descriptor. */
export function encodeValue(descriptor,value){
 const kind='InvalidRow';
 if(value===null){check(descriptor.optional,kind);return {kind:'null'};}
 switch(descriptor.kind){
 case 'String':check(typeof value==='string',kind);utf8Length(value,MAX_BYTES);return {kind:'string',value};
 case 'Reference':check(typeof value==='string'&&value!=='',kind);utf8Length(value,MAX_BYTES);return {kind:'reference',value};
 case 'Boolean':check(typeof value==='boolean',kind);return {kind:'boolean',value};
 case 'Float':check(typeof value==='number'&&Number.isFinite(value)&&!Object.is(value,-0),kind);return {kind:'float',value:String(value)};
 case 'Integer':{
  if(descriptor.bigint){check(typeof value==='bigint'||typeof value==='string',kind);return {kind:'integer',value:canonical(value,kind)};}
  check(typeof value==='number'||typeof value==='string',kind);const text=canonical(value,kind);check(safe(text),kind);return {kind:'integer',value:text};
 }
 case 'Timestamp':{const text=milliseconds_(value,kind);if(!descriptor.bigint)check(safe(text)&&Math.abs(Number(text))<=DATE_LIMIT,kind);return {kind:'timestamp',value:text};}
 default:throw failure('InvalidSchema');
 }
}
function decodeValue(descriptor,cell){
 const kind='InvalidPayload';check(isPlain(cell),kind);
 if(cell.kind==='null'){check(descriptor.optional,kind);return null;}
 check(cell.kind===CELL[descriptor.kind],kind);
 switch(descriptor.kind){
 case 'String':case 'Reference':check(typeof cell.value==='string',kind);return cell.value;
 case 'Boolean':check(typeof cell.value==='boolean',kind);return cell.value;
 case 'Float':{check(typeof cell.value==='string',kind);const number=Number(cell.value);check(Number.isFinite(number),kind);return number;}
 case 'Integer':{
  const text=canonical(cell.value,kind);
  if(descriptor.bigint)return wide(text);
  check(safe(text),kind);return Number(text);
 }
 case 'Timestamp':{
  const text=canonical(cell.value,kind);
  if(descriptor.bigint)return wide(text);
  const time=Number(text);check(safe(text)&&Math.abs(time)<=DATE_LIMIT,kind);return new Date(time);
 }
 default:throw failure('InvalidSchema');
 }
}

/** Encodes plain row fields; missing fields stay absent and `id` is never a field. */
export function encodeFields(schema,tableName,plain){
 const metadata=tableMetadata(schema,tableName),result={};
 for(const [key,value] of entries(plain,'InvalidRow')){
  const descriptor=metadata.columns.get(key)??metadata.foreignKeys.get(key);check(descriptor,'InvalidRow');
  own(result,key,encodeValue(descriptor,value));
 }
 return result;
}
/** Decodes a wire row into a flat `{id, ...fields}` snapshot with column-typed values. */
export function decodeRow(schema,tableName,wireRow){
 const metadata=tableMetadata(schema,tableName),kind='InvalidPayload';
 check(isPlain(wireRow)&&typeof wireRow.id==='string',kind);
 const result={};own(result,'id',wireRow.id);
 for(const [key,cell] of entries(wireRow.fields,kind)){
  const descriptor=metadata.columns.get(key)??metadata.foreignKeys.get(key);check(descriptor,kind);
  own(result,key,decodeValue(descriptor,cell));
 }
 return result;
}

// Business JSON.
function budget(){
 let nodes=0,bytes=0;
 return {node(depth){check(++nodes<=MAX_NODES&&depth<MAX_DEPTH,'LimitExceeded');},bytes(count){bytes+=count;check(bytes<=MAX_BYTES,'LimitExceeded');}};
}
/** Encodes plain business JSON as API-0011 JSON tags. Tag-shaped objects stay ordinary data. */
export function encodeJson(plain){
 const limits=budget(),ancestors=new Set(),kind='InvalidPayload';
 function visit(value,depth){
  limits.node(depth);
  if(value===null)return {kind:'null'};
  switch(typeof value){
  case 'boolean':return {kind:'boolean',value};
  case 'string':limits.bytes(utf8Length(value,MAX_BYTES));return {kind:'string',value};
  case 'number':
   check(Number.isFinite(value)&&!Object.is(value,-0),kind);
   if(Number.isInteger(value)){check(Number.isSafeInteger(value),kind);return {kind:'integer',value:String(value)};}
   return {kind:'float',value:String(value)};
  case 'bigint':return {kind:'integer',value:canonical(value,kind)};
  case 'object':break;
  default:throw failure(kind);
  }
  check(!ancestors.has(value),kind);ancestors.add(value);
  let result;
  if(Array.isArray(value)){
   check(Object.getOwnPropertySymbols(value).length===0&&Object.keys(value).length===value.length,kind);
   result={kind:'array',value:Array.from({length:value.length},(_,index)=>{const descriptor=Object.getOwnPropertyDescriptor(value,String(index));check(descriptor&&Object.prototype.hasOwnProperty.call(descriptor,'value'),kind);return visit(descriptor.value,depth+1);})};
  }else{
   const fields={};for(const [key,item] of entries(value,kind)){limits.bytes(utf8Length(key,MAX_BYTES));own(fields,key,visit(item,depth+1));}
   result={kind:'object',value:fields};
  }
  ancestors.delete(value);return result;
 }
 return visit(plain,0);
}
/** Decodes API-0011 JSON tags to plain values: safe integers as Number, wider integers as bigint. */
export function decodeJson(tagged){
 const limits=budget(),kind='InvalidPayload';
 function visit(value,depth){
  limits.node(depth);check(isPlain(value),kind);
  switch(value.kind){
  case 'null':return null;
  case 'boolean':check(typeof value.value==='boolean',kind);return value.value;
  case 'string':check(typeof value.value==='string',kind);return value.value;
  case 'integer':{const text=canonical(value.value,kind);return safe(text)?Number(text):wide(text);}
  case 'float':{check(typeof value.value==='string',kind);const number=Number(value.value);check(Number.isFinite(number),kind);return number;}
  case 'array':check(Array.isArray(value.value),kind);return value.value.map(item=>visit(item,depth+1));
  case 'object':{const result={};for(const [key,item] of entries(value.value,kind))own(result,key,visit(item,depth+1));return result;}
  default:throw failure(kind);
  }
 }
 return visit(tagged,0);
}

// Read-only snapshots (captured identity and context).
const mutation=()=>{throw new TypeError('Captured value is read-only');};
const traps={set:mutation,deleteProperty:mutation,defineProperty:mutation,setPrototypeOf:mutation,preventExtensions:mutation};
/** A deep read-only copy of plain JSON-like data; any nested mutation throws, also in sloppy mode. */
export function readonly(value){
 const limits=budget(),ancestors=new Set(),kind='InvalidPayload';
 function visit(item,depth){
  limits.node(depth);
  if(item===null||typeof item==='boolean'||typeof item==='string'||typeof item==='bigint')return item;
  if(typeof item==='number'){check(Number.isFinite(item),kind);return item;}
  check(typeof item==='object',kind);check(!ancestors.has(item),kind);ancestors.add(item);
  let target;
  if(Array.isArray(item)){
   check(Object.getOwnPropertySymbols(item).length===0&&Object.keys(item).length===item.length,kind);
   target=Array.from({length:item.length},(_,index)=>{const descriptor=Object.getOwnPropertyDescriptor(item,String(index));check(descriptor&&Object.prototype.hasOwnProperty.call(descriptor,'value'),kind);return visit(descriptor.value,depth+1);});
  }else{target={};for(const [key,child] of entries(item,kind))own(target,key,visit(child,depth+1));}
  ancestors.delete(item);Object.freeze(target);return new Proxy(target,traps);
 }
 return visit(value,0);
}
