// API-0012: ordinary packaged WASM, loaded once. Node exposes memory, never plaintext files.
export * from './index.js';
export {createWebRuntime} from './web.js';
export {table,column,relation,defineCommand,defineSchema,applicationSchema,ApplicationDatabase} from './application.js';
import {createRuntime,openDatabase as openHost} from './index.js';
import {isApplicationSchema,openApplicationDatabase} from './application.js';
import {readFile} from 'node:fs/promises';
import {randomBytes} from 'node:crypto';
import {check} from './errors.js';
let shared;
export function createNodeRuntime(){
 if(shared)return shared;
 const attempt=(async()=>{const {default:init,Facade}=await import('./wasm/hoyadb_web.js');await init({module_or_path:await readFile(new URL('./wasm/hoyadb_web_bg.wasm',import.meta.url))});const facade=new Facade();return createRuntime({open:(input,key)=>facade.open(input,key),call:input=>facade.call(input)},{namespace:()=>new Uint8Array(randomBytes(16))});})();
 shared=attempt;void attempt.catch(()=>{if(shared===attempt)shared=undefined;});return attempt;
}
export async function openDatabase(options){
 if(!isApplicationSchema(options.schema))return openHost(options);
 if(options.filename||options.engine?.kind&&options.engine.kind!=='memory'){
  check(options.encryptionKey instanceof Uint8Array&&options.encryptionKey.byteLength===32);check(false,'UnsupportedOperation');
 }
 return openApplicationDatabase({...options,createRuntime:createNodeRuntime});
}
