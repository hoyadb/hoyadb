// Bootstrap is explicit; every measured memory operation below uses a preloaded core.
import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import init,{Facade} from '../../../crates/hoyadb-web/pkg/hoyadb_web.js';
import {createRuntime} from '../../../packages/client/src/index.js';
import {runConformance} from './conformance.mjs';
await init({module_or_path:await readFile(new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url))});
const facade=new Facade();
globalThis.fetch=()=>{throw Error('memory must not fetch');};
for(const name of ['crypto','navigator'])Object.defineProperty(globalThis,name,{configurable:true,get(){throw Error('memory must not touch '+name);}});
const runtime=createRuntime({open:(input,key)=>facade.open(input,key),call:input=>facade.call(input)},{namespace:()=>{throw Error('explicit namespace must bypass entropy');}});
try{const reports=await runConformance(runtime);console.log(JSON.stringify(reports,null,2));assert.equal(reports.filter(x=>!x.ok).length,0,'real WASM SDK conformance');}finally{facade.free();}
