// One preloaded facade per dedicated worker. This controller owns no DB algorithm.
import {check,publicFailure} from './errors.js';
import {copyOwned} from './values.js';
import {utf8Length} from './utf8.js';
const failed=error=>JSON.stringify({abi:'1',ok:false,error:{kind:publicFailure(error).kind}});
export function createWorkerHost(facade,initStorage){
 let tail=Promise.resolve(),storage=null;
 async function dispatch(message){
  let key;
  try{
   check(message&&typeof message==='object'&&!Array.isArray(message));
   if(message.key instanceof ArrayBuffer)key=new Uint8Array(message.key);
   const keys=Object.keys(message).sort().join(',');
   check((message.kind==='open'&&keys==='input,key,kind')||(message.kind==='call'&&keys==='input,kind'));
   check(typeof message.input==='string');utf8Length(message.input,1048576);
   if(message.kind==='open'){
    check(key!==undefined&&(key.byteLength===0||key.byteLength===32));
    // Core validates the original bytes before any VFS initialization. Wrapping
    // them in a codec request would incorrectly shrink the admitted frame budget.
    const validation=copyOwned(JSON.parse(facade.validateOpen(message.input,key.byteLength)));
    check(validation?.abi==='1'&&typeof validation.ok==='boolean','InvalidPayload');
    if(!validation.ok)return JSON.stringify(validation);
    let request;try{request=copyOwned(JSON.parse(message.input));}catch{throw {kind:'InvalidPayload'};}
    check(request.op==='open');const memory=request.args?.engine?.kind==='memory';
    check(key.byteLength===(memory?0:32));
    if(!memory){check(request.args?.engine?.kind==='opfs');storage??=Promise.resolve().then(initStorage);const installing=storage;try{await installing;}catch(error){if(storage===installing)storage=null;throw error;}}
    return facade.open(message.input,key);
   }
   return facade.call(message.input);
  }catch(error){return failed(error);}
  finally{key?.fill(0);}
 }
 return message=>{const next=tail.then(()=>dispatch(message));tail=next.then(()=>undefined,()=>undefined);return next;};
}
