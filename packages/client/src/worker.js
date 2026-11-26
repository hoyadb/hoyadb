// General SDK worker: no fixture schemas, engines or command implementations.
import init,{Facade,initStorage} from './wasm/hoyadb_web.js';
import {createWorkerHost} from './worker-host.js';
await init();
const facade=new Facade();
const dispatch=createWorkerHost(facade,initStorage);
self.addEventListener('message',event=>{void dispatch(event.data).then(value=>self.postMessage({kind:'reply',value}));});
self.postMessage({kind:'ready'});
