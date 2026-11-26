// Harness-only resource worker: the unchanged ordinary installed WASM module and
// production worker host. A separate MessagePort reports this worker's linear
// memory between operations; the production reply channel is never instrumented.
import init,{Facade,initStorage}from'./node_modules/@hoyadb/client/src/wasm/hoyadb_web.js';
import{createWorkerHost}from'./node_modules/@hoyadb/client/src/worker-host.js';
const output=await init();const facade=new Facade();const dispatch=createWorkerHost(facade,initStorage);
self.addEventListener('message',event=>{
 if(event.data?.kind==='harness-metrics'&&event.ports.length===1){const port=event.ports[0];port.onmessage=()=>port.postMessage({harness_worker_linear_bytes:output.memory.buffer.byteLength});return;}
 void dispatch(event.data).then(value=>self.postMessage({kind:'reply',value}));
});
self.postMessage({kind:'ready'});
