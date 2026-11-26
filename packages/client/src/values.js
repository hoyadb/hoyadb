import {check,failure} from './errors.js';
import {utf8Length} from './utf8.js';
export const MAX_BYTES=1048576;
export const string=value=>{check(typeof value==='string');utf8Length(value,MAX_BYTES);return value;};
export function integer(value){
 if(typeof value==='number')check(Number.isSafeInteger(value)&&!Object.is(value,-0));
 check(['string','number','bigint'].includes(typeof value));const text=String(value);
 check(/^(0|[1-9][0-9]*|-[1-9][0-9]*)$/.test(text));
 const negative=text[0]==='-',digits=negative?text.slice(1):text,max=negative?'9223372036854775808':'9223372036854775807';
 check(digits.length<max.length||(digits.length===max.length&&digits<=max));return text;
}
export function unsigned(value,bits=32){
 if(typeof value==='number')check(Number.isSafeInteger(value)&&value>=0&&!Object.is(value,-0));
 check(['string','number','bigint'].includes(typeof value));const text=String(value),max={16:'65535',32:'4294967295',64:'18446744073709551615'}[bits];
 check(/^(0|[1-9][0-9]*)$/.test(text)&&max&&(text.length<max.length||(text.length===max.length&&text<=max)));return text;
}
export const handle=value=>{const result=unsigned(value,64);check(result!=='0','InvalidHandle');return result;};
export function copyOwned(input){
 const ancestors=new Set();let nodes=0,bytes=0;
 const add=count=>{bytes+=count;check(bytes<=MAX_BYTES,'LimitExceeded');};
 const quote=value=>{utf8Length(value,MAX_BYTES);return utf8Length(JSON.stringify(value),MAX_BYTES);};
 function visit(value,depth){
  check(++nodes<=100000,'LimitExceeded');
  if(value===null){add(4);return null;}
  if(typeof value==='boolean'){add(value?4:5);return value;}
  if(typeof value==='string'){add(quote(value));return value;}
  check(value&&typeof value==='object');check(depth<64,'LimitExceeded');
  check(!ancestors.has(value));check(Object.getOwnPropertySymbols(value).length===0);
  const array=Array.isArray(value);check(array||Object.getPrototypeOf(value)===Object.prototype||Object.getPrototypeOf(value)===null);
  ancestors.add(value);const result=array?[]:{};add(2);
  const keys=Object.keys(value);if(array)check(keys.length===value.length);
  for(let n=0;n<keys.length;n++){
   const key=keys[n];if(array)check(key===String(n));
   const descriptor=Object.getOwnPropertyDescriptor(value,key);check(descriptor&&Object.prototype.hasOwnProperty.call(descriptor,'value'));
   if(n)add(1);if(!array)add(quote(key)+1);
   const child=visit(descriptor.value,depth+1);
   Object.defineProperty(result,key,{value:child,enumerable:true,writable:true,configurable:true});
  }
  ancestors.delete(value);return result;
 }
 return visit(input,0);
}
const mutation=()=>{throw new TypeError('Trusted snapshot is read-only');};
export function immutableOwned(input){
 function wrap(value){
  if(value===null||typeof value!=='object')return value;
  const target=Array.isArray(value)?value.map(wrap):Object.fromEntries(Object.entries(value).map(([key,item])=>[key,wrap(item)]));
  Object.freeze(target);return new Proxy(target,{set:mutation,deleteProperty:mutation,defineProperty:mutation,setPrototypeOf:mutation});
 }
 return wrap(copyOwned(input));
}
export const wire=value=>JSON.stringify(copyOwned(value));
export const j={
 null:()=>({kind:'null'}),boolean:value=>{check(typeof value==='boolean');return {kind:'boolean',value};},
 string:value=>({kind:'string',value:string(value)}),reference:value=>({kind:'reference',value:string(value)}),
 integer:value=>({kind:'integer',value:integer(value)}),timestamp:value=>({kind:'timestamp',value:integer(value)}),
 float:value=>{check(typeof value==='string'||typeof value==='number');const text=String(value);check(/^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$/.test(text)&&Number.isFinite(Number(text)));return {kind:'float',value:text};},
 array:value=>{check(Array.isArray(value));return {kind:'array',value:copyOwned(value)};},
 object:value=>{check(value&&typeof value==='object'&&!Array.isArray(value));return {kind:'object',value:copyOwned(value)};}
};
// Known wire DTOs are number-free. Decode via core first so duplicates and
// numeric tokens cannot be silently consumed by the host JSON parser.
export function unboxWire(value){
 if(value.kind==='null')return null;
 if(['string','boolean'].includes(value.kind))return value.value;
 if(value.kind==='array')return value.value.map(unboxWire);
 if(value.kind==='object')return Object.fromEntries(Object.entries(value.value).map(([key,item])=>[key,unboxWire(item)]));
 throw failure('InvalidPayload');
}
