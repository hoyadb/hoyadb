// API-0010 / TC-0035: synthetic app-supplied keys; never a production key service.
import {CommandClient,j,query} from './command-client.js';
import {workerError} from './harness-error.js';
const output=document.querySelector('#results'),savedKey='hoyadb-phase-six-run';
const saved=sessionStorage.getItem(savedKey);
const state=saved?JSON.parse(saved):{namespace:`hoyadb-spec23-${crypto.randomUUID()}`,runtime:navigator.userAgent,origin:location.origin,profile:new URL(location.href).searchParams.get('profile')||'release',mode:new URL(location.href).searchParams.has('host')?'host-only':'full',results:[],stage:0,done:false};
window.hoyaSecurity=state;
const render=()=>{output.textContent=JSON.stringify(state,null,2);};
const assert=(value,label)=>{if(!value)throw new Error(label);};
const ordered=value=>Array.isArray(value)?value.map(ordered):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(key=>[key,ordered(value[key])])):value;
const equal=(actual,expected,label)=>assert(JSON.stringify(ordered(actual))===JSON.stringify(ordered(expected)),label);
const report=(name,detail)=>{state.results.push({name,passed:true,detail});render();};
const key=()=>Uint8Array.from({length:32},(_,n)=>(n*7+11)%256);
const keyHex=()=>Array.from(key(),n=>n.toString(16).padStart(2,'0')).join('');
const canary='synthetic protected country 東京\0🌿';
const identity=j.object({tenant:j.string('synthetic protected identity'),nested:j.array([j.object({name:j.string('trusted original')})])});
const definition={name:'protected-command-v1',arguments:{value:{kind:'String',optional:false}},reference_paths:[]};
const filename='phase-four-protected-security.sqlite';
let worker,request,ok,reloading=false;
const clients=[];
function start(){
 worker=new Worker(`./worker.js?run=${crypto.randomUUID()}`,{type:'module'});
 const pending=new Map();let sequence=0;
 worker.onmessage=({data})=>{const item=pending.get(data.request);if(item){pending.delete(data.request);clearTimeout(item.timer);item.resolve(data);}};
 worker.onerror=()=>{for(const item of pending.values()){clearTimeout(item.timer);item.reject(workerError(null,'security worker event'));}pending.clear();};
 request=message=>{const number=++sequence,start=performance.now();return new Promise((resolve,reject)=>{
  const timeout=message.op==='security_conformance'&&message.group==='command'&&message.index===13?900000:60000;
  const timer=setTimeout(()=>{pending.delete(number);reject(new Error(`security ${message.op} deadline ${timeout}ms`));},timeout);
  pending.set(number,{resolve,reject,timer});worker.postMessage({...message,request:number});
 }).finally(()=>{(state.rpc_timings??=[]).push({operation:message.op,group:message.group,index:message.index,elapsed_ms:Math.round(performance.now()-start)});});};
 ok=async message=>{const reply=await request(message);if(reply.error)throw workerError(reply.error,`security/${message.op}`);return reply.value;};
}
async function reject(work,kind){const error=await work.then(()=>null,error=>error);assert(error,'operation must reject');if(kind)equal(error.kind,kind,'static error kind');assert(!String(error).includes(keyHex()),'error key-free');return error;}
function client(){const c=new CommandClient(message=>request(message),{context:()=>identity,clock:()=>1000});clients.push(c);return c;}
async function stop(){for(const c of clients.splice(0).reverse())await c.close();await ok({op:'shutdown'});worker.terminate();worker=null;}
async function boot(){start();state.capabilities=await ok({op:'init',namespace:state.namespace});}
async function files(){
 const directory=await(await navigator.storage.getDirectory()).getDirectoryHandle(state.namespace);
 const result=[];
 async function visit(dir,path){for await(const [name,handle]of dir.entries()){
  if(handle.kind==='directory')await visit(handle,[...path,name]);
  else {const bytes=new Uint8Array(await(await handle.getFile()).arrayBuffer());result.push({path:[...path,name],bytes});}
 }}
 await visit(directory,[]);return result.sort((a,b)=>a.path.join('/').localeCompare(b.path.join('/')));
}
async function digest(items){return Promise.all(items.map(async item=>({path:item.path.join('/'),size:item.bytes.length,sha256:Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',item.bytes)),n=>n.toString(16).padStart(2,'0')).join('')})));}
async function replace(item){let dir=await(await navigator.storage.getDirectory()).getDirectoryHandle(state.namespace);for(const name of item.path.slice(0,-1))dir=await dir.getDirectoryHandle(name);const handle=await dir.getFileHandle(item.path.at(-1));const stream=await handle.createWritable();await stream.write(item.bytes);await stream.close();}
function contains(bytes,needle){const target=typeof needle==='string'?new TextEncoder().encode(needle):needle;outer:for(let i=0;i+target.length<=bytes.length;i++){for(let j=0;j<target.length;j++)if(bytes[i+j]!==target[j])continue outer;return true;}return false;}
async function unchanged(before,label){equal(await digest(await files()),before,label);}
try{
 await boot();
 if(state.stage===0){
  // S01/S10 before creation: common fixture and client both validate supplied keys.
  await stop();const initial=await digest(await files());await boot();
  const invalid=client();await reject(invalid.open(filename),'InvalidRequest');
  for(const malformed of ['', '00'.repeat(31),'GG'.repeat(32)])await reject(invalid.call('open_secure',{path:filename,namespace:'1'.repeat(32),options:{capacity:'1000',history:'100',initial_delay_ms:'1000',multiplier:'2',max_delay_ms:'60000'},version:'1',authored:false,key_hex:malformed},null));
  await stop();await unchanged(initial,'malformed opens preserve all physical files');await boot();
  report('S01/S10 malformed and absent supplied keys create no database',{physicalFilesUnchanged:true});
  if(state.mode==='full')for(const group of ['typed','model','reactive','review','command','entity']){
   const count=group==='command'?14:group==='entity'?13:group==='review'?5:1;
   for(let index=0;index<count;index++){
    state.current={group,index};render();const started=performance.now();
    const result=await ok({op:'security_conformance',group,index,key_hex:keyHex()});
    if(Array.isArray(result))assert(result.length>0,'shared assertions present');else if(group==='review'){assert(result.cases?.length===1,'review shared assertion present');for(const check of result.poolChecks)equal(check.before,check.after,'memory TEMP operands preserve SAH inventory');}else {assert(result.case&&result.case!=='stub','shared case present');equal(result.files_before,result.files_after,'shared case restores pool inventory');}
    report(`S06 encrypted ${group}[${index}]`,{result,elapsed_ms:Math.round(performance.now()-started)});
   }
  }
  const c=client();await c.open(filename,{encryptionKey:key()});
  equal(await c.call('protection'),{encrypted:true,cipher:'SQLCipher 4.19.0',backup_exclusion:'unsupported'},'actual cipher and truthful backup capability');
  await c.configureEntities('synthetic protected scope',['customers']);
  await c.pullOnce(async()=>JSON.stringify({checkpoint:'synthetic protected checkpoint',complete:false,changes:[{table:'customers',record_id:'server:protected-pull',op:'create',changes:{country:j.string('protected field clock')},removed_fields:[],timestamp_ms:'20',field_timestamps:{country:'20'}}]}));
  let rejected=0;
  await c.execute(definition,j.object({value:j.string(canary)}),async scope=>{
   for(const mutate of [()=>{scope.context.identity.value.tenant.value='replacement';},()=>{scope.context.identity.value.nested.value[0].value.name.value='replacement';},()=>{scope.context.identity.value.nested.value.push(j.null());},()=>{scope.context={};}]){
    try{mutate();}catch(error){assert(error instanceof TypeError,'context rejects mutation');rejected++;}
   }
   await Promise.resolve();equal(scope.context.identity,identity,'capture survives await');
   await scope.insert('customers',{country:j.string(canary)});return j.string('synthetic protected result');
  });
  equal(rejected,4,'all context assignments rejected');
  const batch=await c.preparePush(1000),entry=(await c.queueStatus()).commands[0];
  assert(batch.envelopes.length===1,'one frozen command envelope');
  const envelope=JSON.parse(batch.envelopes[0]);equal(envelope.context.identity,JSON.parse(JSON.stringify({tenant:'synthetic protected identity',nested:[{name:'trusted original'}]})),'actual outgoing identity remains original');
  assert(batch.envelopes[0].includes('synthetic protected country'),'complete retry retains explicit payload');
  assert(!JSON.stringify(await c.queueStatus()).includes(keyHex()),'retry has no key');
  const diagnostics=await c.call('diagnostics');
  equal(Object.keys(diagnostics).sort(),['schema_version','pending','sending','awaiting_outcome','live','blocked','ready','pages','changes','tombstones'].sort(),'diagnostic exact allowlist');
  for(const value of Object.values(diagnostics))assert(typeof value==='boolean'||/^[0-9]+$/.test(value),'diagnostic scalar only');
  state.expected={envelope:batch.envelopes[0],key:entry.key,rows:await c.fetch(query('customers')),progress:await c.syncProgress(),diagnostics};
  // Named postcommit barrier: the final successful read acknowledges the
  // committed rows/recovery; terminate with its owner open, without shutdown.
  worker.terminate();worker=null;clients.length=0;
  await new Promise(resolve=>setTimeout(resolve,100));const raw=await files();
  for(const item of raw)for(const secret of [canary,'synthetic protected identity','synthetic protected result','synthetic protected checkpoint',keyHex(),key(),'SQLite format 3\0'])assert(!contains(item.bytes,secret),'encrypted main/sidecars contain no protected canaries or plaintext header');
  state.protectedFiles=await digest(raw);
  report('S02/S05/S09/S11/S12 protected contents, postcommit worker termination, context and diagnostics',{files:state.protectedFiles,contextRejections:rejected,cipher:'SQLCipher 4.19.0',backupExclusion:'unsupported'});
  // Correct key is supplied again by the app on every independent worker open.
  await boot();const unkeyed=client();await reject(unkeyed.openUnprotected(filename),'BackendFailure');await stop();await unchanged(state.protectedFiles,'independent unkeyed adapter cannot read cipher schema');
  await boot();const wrong=client();await reject(wrong.open(filename,{encryptionKey:new Uint8Array(32).fill(249)}),'BackendFailure');await stop();await unchanged(state.protectedFiles,'wrong key never mutates physical files');
  await boot();const legacy=client();await legacy.openUnprotected('phase-four-legacy-security.sqlite');await legacy.close();await stop();const legacyBefore=await digest(await files());
  await boot();const protectedLegacy=client();await reject(protectedLegacy.open('phase-four-legacy-security.sqlite',{encryptionKey:key()}),'BackendFailure');await stop();await unchanged(legacyBefore,'protected plaintext input never converts');
  report('S03/S10 wrong key and legacy plaintext reject without fallback or mutation',{wrongKeyUnchanged:true,legacyUnchanged:true});
  const original=(await files()).find(item=>contains(item.bytes.slice(0,516),filename));assert(original&&original.bytes.length>4096+4096,'owned encrypted physical slot found');
  const corrupt={path:original.path,bytes:new Uint8Array(original.bytes)};corrupt.bytes[4096+200]^=0x40;await replace(corrupt);const corruptBefore=await digest(await files());
  await boot();const tampered=client();await reject(tampered.open(filename,{encryptionKey:key()}),'BackendFailure');await stop();await unchanged(corruptBefore,'tampered failure commits nothing');await replace(original);
  report('S04 authenticated page corruption rejects and commits nothing',{physicalPageTampered:true,failedOpenUnchanged:true});
  state.stage=1;sessionStorage.setItem(savedKey,JSON.stringify(state));reloading=true;render();setTimeout(()=>location.reload(),100);
 }else{
  const c=client();await c.open(filename,{encryptionKey:key()});await c.configureEntities('synthetic protected scope',['customers']);
  equal(await c.fetch(query('customers')),state.expected.rows,'exact protected rows after page reload');equal(await c.syncProgress(),state.expected.progress,'durable checkpoint and clocks after reload');
  const retry=await c.preparePush(2000);equal(retry.envelopes,[state.expected.envelope],'frozen retry exact after actual reload');
  equal(await c.call('diagnostics'),state.expected.diagnostics,'diagnostic state after reload');
  report('S07/S10 actual page reload restores keyed rows queue checkpoint and exact retry',{exactRetry:true,checkpoint:true,ready:false});
 }
}catch(error){state.results.push({name:'failure',passed:false,detail:String(error),stack:error.stack});render();}
finally{
 if(!reloading){
  try{if(worker)await stop();await(await navigator.storage.getDirectory()).removeEntry(state.namespace,{recursive:true});state.cleaned=true;}catch(error){state.cleanupError=String(error);}
  worker?.terminate();state.done=true;sessionStorage.removeItem(savedKey);render();
 }
}
