// TC-0036 G16 default-off instrumented security harness, never a package export.
import init,{Facade,initStorage,testFile,testPoolStats} from '../../../crates/hoyadb-web/pkg-test/hoyadb_web.js';
import {createWorkerHost} from '../../../packages/client/src/worker-host.js';
await init();const facade=new Facade();const dispatch=createWorkerHost(facade,initStorage);
let tail=Promise.resolve();
self.addEventListener('message',event=>{tail=tail.then(async()=>{
 const m=event.data;
 if(m.kind!=='control'){self.postMessage({kind:'reply',value:await dispatch(m)});return;}
 try{
  if(m.operation==='validate'){self.postMessage({kind:'reply',value:facade.validateOpen(m.input,m.length)});return;}
  await initStorage();const value=m.operation==='stats'?testPoolStats():testFile(m.operation,m.name,new Uint8Array(m.data??[]));
  const bytes=new Uint8Array(value.buffer,value.byteOffset,value.byteLength);const owned=new Uint8Array(bytes);self.postMessage({kind:'binary',value:owned.buffer},[owned.buffer]);
 }catch(error){const kind=typeof error==='string'&&['UnsupportedOperation','InvalidRequest','Busy','BackendFailure','LimitExceeded'].includes(error)?error:'BackendFailure';self.postMessage({kind:'reply',value:JSON.stringify({abi:'1',ok:false,error:{kind}})});}
 });});self.postMessage({kind:'ready'});
