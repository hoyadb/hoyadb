// G09: a transient failed initializer is retried only by a fresh explicit open.
import {createWebRuntime,Database,j} from '../../../tools/javascript/dist/client/src/web-entry.js';import {schemas} from '../../../packages/client/test/schemas.mjs';import {startControl} from './control-client.mjs';
const names=['phase-seven-sdk-owned-retry-holder-20261024.sqlite','phase-seven-sdk-owned-retry-second-20261024.sqlite'];
let first,second,a,b;const reports=[];
try{
 const clean=await startControl();try{for(const name of names)if((await clean.file('exists',name))[0])await clean.file('remove',name);}finally{clean.stop();}
 const start=()=>createWebRuntime({worker:new Worker(new URL('../../../tools/javascript/dist/client/src/worker.js',import.meta.url),{type:'module'})});
 const options=(name,key)=>({schema:schemas[1],engine:{kind:'opfs',name},encryptionKey:new Uint8Array(32).fill(key),namespace:'0909'.repeat(8)});
 first=await start();a=new Database(first);await a.open(options(names[0],23));await a.close();
 second=await start();b=new Database(second);let failed=false;try{await b.open(options(names[1],24));}catch(error){if(error.kind!=='BackendFailure')throw error;failed=true;}if(!failed)throw Error('second live worker unexpectedly acquired exclusive pool');
 first.channel.close();await new Promise(resolve=>setTimeout(resolve,50));await b.open(options(names[1],25));await b.insert('sensors',{label:j.string('fresh explicit key after initializer retry')});if((await b.query('sensors').fetch()).length!==1)throw Error('fresh retry failed actual CRUD');await b.close();second.channel.close();reports.push({name:'G09 actual transient OPFS initialization failure permits fresh explicit open',ok:true});
}catch(error){reports.push({name:'G09 actual transient OPFS initialization failure permits fresh explicit open',ok:false,error:error.kind??error.message});}
finally{await a?.close().catch(()=>{});await b?.close().catch(()=>{});first?.channel.close();second?.channel.close();const clean=await startControl();try{for(const name of names)if((await clean.file('exists',name))[0])await clean.file('remove',name);}finally{clean.stop();}}
globalThis.hoyaRetryResult={done:true,reports};document.querySelector('#output').textContent=JSON.stringify(globalThis.hoyaRetryResult,null,2);
