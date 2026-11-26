// API-0010 S10: suspend real worker replies, without a shadow storage engine.
import {CommandClient,j,query} from './command-client.js';
import {workerError} from './harness-error.js';
const output=document.querySelector('#results');
const state={namespace:`hoyadb-spec23-${crypto.randomUUID()}`,runtime:navigator.userAgent,origin:location.origin,results:[],done:false};
window.hoyaLifecycle=state;
const render=()=>{output.textContent=JSON.stringify(state,null,2);};
const report=(name,detail)=>{state.results.push({name,passed:true,detail});render();};
const assert=(value,label)=>{if(!value)throw new Error(label);};
const gates=[];
const deferred=()=>{let resolve;const promise=new Promise(yes=>{resolve=yes;});gates.push(()=>resolve());return {promise,resolve};};
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
const key=value=>new Uint8Array(32).fill(value);
const rejected=async(work,kind)=>{const error=await work.then(()=>null,error=>error);assert(error?.kind===kind,`expected static ${kind}`);return error;};
const worker=new Worker(`./worker.js?run=${crypto.randomUUID()}`,{type:'module'});
const pending=new Map();let sequence=0;
worker.onmessage=({data})=>{const item=pending.get(data.request);if(item){pending.delete(data.request);clearTimeout(item.timer);item.resolve(data);}};
worker.onerror=()=>{for(const item of pending.values()){clearTimeout(item.timer);item.reject(workerError(null,'lifecycle worker event'));}pending.clear();};
function request(message){const number=++sequence;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(number);reject(new Error(`lifecycle ${message.op} deadline60000ms`));},60000);pending.set(number,{resolve,reject,timer});worker.postMessage({...message,request:number});});}
async function ok(message){const reply=await request(message);if(reply.error)throw workerError(reply.error,`lifecycle/${message.op}`);return reply.value;}
const clients=[];
function client(send=request){const c=new CommandClient(send,{context:()=>j.null(),clock:()=>1000});clients.push(c);return c;}
const open=(c,secure,path,value=7)=>secure?c.open(path,{encryptionKey:key(value)}):c.openUnprotected(path);
const definition={name:'lifecycle-test-v1',arguments:{},reference_paths:[]};
try {
 await ok({op:'init',namespace:state.namespace});
 for(const secure of [true,false]) {
  const opened=deferred(),openReply=deferred(),closed=deferred(),closeReply=deferred();
  let openCalls=0,closeCalls=0,lateOwner,oldNamespace,newNamespace,oldKey,newKey;
  const c=client(async message=>{
   const frame=JSON.parse(message.input);
   if(['open','open_secure'].includes(frame.op)) {
    openCalls++;if(openCalls===1){oldNamespace=frame.args.namespace;oldKey=frame.args.key_hex;}else{newNamespace=frame.args.namespace;newKey=frame.args.key_hex;}
   }
   const reply=await request(message);
   if(['open','open_secure'].includes(frame.op)&&openCalls===1){lateOwner=JSON.parse(reply.value).value;opened.resolve();await openReply.promise;}
   if(frame.op==='close'&&++closeCalls===1){closed.resolve();await closeReply.promise;}
   return reply;
  });
  const name=`phase-four-lifecycle-${secure?'protected':'legacy'}.sqlite`;
  const opening=open(c,secure,name).then(()=>null,error=>error);
  await opened.promise;
  const shutdown=await request({op:'shutdown'});assert(shutdown.error?.kind==='BackendFailure','real late owner blocks shutdown');
  for(const duplicateSecure of [true,false])await rejected(open(c,duplicateSecure,'phase-four-duplicate-lifecycle.sqlite'),'Busy');
  assert(openCalls===1,'duplicate opens made no second worker RPC');
  let complete=false;const closing=c.close().then(()=>{complete=true;}),shared=c.close();await tick();
  assert(!complete,'close waits suspended opening');openReply.resolve();assert((await opening)?.kind==='Closed','cancelled open rejects Closed');
  await closed.promise;assert(!complete&&c.owner===null,'late handle never publishes and close waits acknowledgement');
  await rejected(c.call('protection',{},lateOwner),'InvalidHandle');
  await rejected(open(c,secure,'phase-four-still-closing.sqlite'),'Busy');
  closeReply.resolve();await Promise.all([closing,shared]);assert(closeCalls===1,'one actual late-handle disposal');
  await open(c,secure,`phase-four-lifecycle-new-${secure?'protected':'legacy'}.sqlite`,19);
  assert(oldNamespace!==newNamespace,'reopen captures a new namespace');
  if(secure)assert(oldKey!==newKey&&newKey==='13'.repeat(32),'fresh reopen uses only the new supplied key');
  await c.execute(definition,j.object({}),async scope=>{await scope.insert('customers',{country:j.string('lifecycle exact persisted row')});return j.null();});
  await c.close();
  const reopened=client();await open(reopened,secure,`phase-four-lifecycle-new-${secure?'protected':'legacy'}.sqlite`,19);
  const rows=await reopened.fetch(query('customers'));assert(rows.length===1&&rows[0].fields.country.value==='lifecycle exact persisted row','fresh-key reopen preserves exact committed data');await reopened.close();
  const original=client();await open(original,secure,name);assert((await original.fetch(query('customers'))).length===0,'cancelled first open was closed without inventing writes');await original.close();
  report(`S10 real ${secure?'protected':'legacy'} pending open, duplicate rejection, acknowledged close and fresh reopen`,{lateOwnerBlockedShutdown:true,duplicateOpenRpc:0,closeRpc:1,exactRowsAfterReopen:true});
 }
 // Actual engine failure produces no handle; only its reply is suspended.
 const setup=client();await setup.open('phase-four-lifecycle-failed.sqlite',{encryptionKey:key(11)});await setup.close();
 const failed=deferred(),failedReply=deferred();let fabricatedClose=0;
 const invalid=client(async message=>{const frame=JSON.parse(message.input);if(frame.op==='close')fabricatedClose++;const reply=await request(message);if(frame.op==='open_secure'){assert(JSON.parse(reply.value).ok===false,'actual wrong key fails');failed.resolve();await failedReply.promise;}return reply;});
 const badOpen=invalid.open('phase-four-lifecycle-failed.sqlite',{encryptionKey:key(99)}).then(()=>null,error=>error);await failed.promise;
 let failedClosed=false;const failureClose=invalid.close().then(()=>{failedClosed=true;});await tick();assert(!failedClosed,'failed open reply is still awaited');failedReply.resolve();assert((await badOpen)?.kind==='Closed','cancelled failed open reports Closed');await failureClose;assert(fabricatedClose===0&&invalid.owner===null,'no fabricated close for failed actual open');
 report('S10 real wrong-key pending reply closes without an invented owner',{actualEngineFailure:true,closeRpc:0});
 // A caller-owned transport failure occurs BEFORE disposal reaches the worker.
 const acquired=deferred(),release=deferred();let closes=0,physicalCloses=0;
 const retry=client(async message=>{const frame=JSON.parse(message.input);if(frame.op==='close'){closes++;if(closes===1)return {error:{kind:'BackendFailure'}};physicalCloses++;}const reply=await request(message);if(frame.op==='open_secure'){acquired.resolve();await release.promise;}return reply;});
 const retryOpen=retry.open('phase-four-lifecycle-close-retry.sqlite',{encryptionKey:key(23)}).then(()=>null,error=>error);await acquired.promise;
 const firstClose=retry.close().then(()=>null,error=>error);release.resolve();assert((await retryOpen)?.kind==='Closed','retry case cancels its opening');assert((await firstClose)?.kind==='BackendFailure','explicit simulated transport failure reaches close');
 assert((await request({op:'shutdown'})).error?.kind==='BackendFailure','undisposed real owner still blocks shutdown');await rejected(retry.openUnprotected('phase-four-orphan-replacement.sqlite'),'Busy');
 await Promise.all([retry.close(),retry.close()]);assert(closes===2&&physicalCloses===1,'concurrent retry disposes one real owner');
 report('S10 failed caller transport retains real late handle until shared close retry',{simulatedTransportFailureBeforeRpc:true,actualCloseRpc:1});
}catch(error){state.results.push({name:'failure',passed:false,detail:String(error),stack:error.stack});render();}
finally {
 for(const release of gates)release();
 try{for(const c of clients.reverse())await c.close();await ok({op:'shutdown'});await(await navigator.storage.getDirectory()).removeEntry(state.namespace,{recursive:true});state.cleaned=true;}catch(error){state.cleanupError=String(error);}
 worker.terminate();state.done=true;render();
}
