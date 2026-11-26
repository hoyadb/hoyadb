// TC-0046 APP02/03/04/07/08/09: same public tutorial in a real dedicated-worker browser.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {readFile,mkdtemp,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve,extname,sep} from 'node:path';
const root=resolve('.'),profile=await mkdtemp(join(tmpdir(),'hoyadb-app-browser-'));
const imports={'@hoyadb/client':'/tools/javascript/dist/client/src/web-entry.js','@hoyadb/client/host':'/tools/javascript/dist/client/src/index.js','@hoyadb/client/application':'/tools/javascript/dist/client/src/application.js','@hoyadb/testing':'/tools/javascript/dist/testing/src/index.js'};
const server=createServer(async(req,res)=>{try{
 res.setHeader('Cross-Origin-Opener-Policy','same-origin');res.setHeader('Cross-Origin-Embedder-Policy','require-corp');res.setHeader('Cache-Control','no-store');
 if(req.url==='/'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>HoyaDB application integration</title><script type="importmap">'+JSON.stringify({imports})+'</script><body>Real application binding test</body>');return;}
 const path=resolve(root,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));assert.ok(path.startsWith(root+sep));
 res.setHeader('Content-Type',({'.mjs':'text/javascript','.js':'text/javascript','.wasm':'application/wasm','.json':'application/json'})[extname(path)]??'application/octet-stream');res.end(await readFile(path));
}catch{res.writeHead(404);res.end('not found');}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const url=`http://127.0.0.1:${server.address().port}/`;
const browser=spawn(process.env.HOYADB_CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',['--headless=new','--no-first-run','--no-default-browser-check','--disable-background-networking','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{stdio:'ignore'});
let socket;
try{
 let port;for(let n=0;n<100;n++){try{port=(await readFile(join(profile,'DevToolsActivePort'),'utf8')).split('\n');break;}catch{await new Promise(r=>setTimeout(r,50));}}
 assert.ok(port,'installed Chrome starts');socket=new WebSocket(`ws://127.0.0.1:${port[0]}${port[1]}`);await new Promise((yes,no)=>{socket.onopen=yes;socket.onerror=no;});
 let id=0;const pending=new Map();socket.onmessage=event=>{const value=JSON.parse(event.data);if(value.id&&pending.has(value.id)){pending.get(value.id)(value);pending.delete(value.id);}};
 async function call(method,params={},sessionId){const key=++id;let timer;const result=new Promise((yes,no)=>{pending.set(key,yes);timer=setTimeout(()=>no(Error('timeout '+method)),30000);});socket.send(JSON.stringify({id:key,method,params,...(sessionId?{sessionId}:{})}));try{const value=await result;assert.equal(value.error,undefined,JSON.stringify(value.error));return value.result;}finally{clearTimeout(timer);}}
 const {targetId}=await call('Target.createTarget',{url:'about:blank'});const {sessionId}=await call('Target.attachToTarget',{targetId,flatten:true});const c=(method,params)=>call(method,params,sessionId);
 await c('Page.enable');await c('Page.navigate',{url});
 async function evaluate(expression){const value=await c('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});assert.equal(value.exceptionDetails,undefined,JSON.stringify(value.exceptionDetails));return value.result.value;}
 for(let n=0;n<100;n++){if(await evaluate('document.readyState==="complete"&&!!document.querySelector("script[type=importmap]")'))break;await new Promise(r=>setTimeout(r,30));}
 assert.equal(await evaluate('(async()=>{await import("/examples/application.mjs");return "tutorial passed";})()'),'tutorial passed');
 // Protected storage is task-owned in the disposable browser profile. No user store is opened.
 const protection=await evaluate(`(async()=>{
  const {openDatabase,defineSchema,table,column}=await import('@hoyadb/client');
  const schema=defineSchema({version:1,tables:{notes:table({columns:{body:column.string()}})}});
  const filename='application-fixture.sqlite';const encryptionKey=new Uint8Array(32).fill(19);
  let missing=false;try{await openDatabase({schema,filename});}catch(e){missing=e.kind==='InvalidRequest';}if(!missing)throw Error('missing key did not fail closed');
  const db=await openDatabase({schema,filename,encryptionKey});await db.table('notes').insert({body:'protected'});await db.close();
  const again=await openDatabase({schema,filename,encryptionKey});const rows=await again.query('notes').fetch();const protection=await again.raw.protection();await again.close();
  if(rows[0]?.body!=='protected'||!protection.encrypted)throw Error('protected reopen failed');return protection;
 })()`);
 const report={status:'passed',date:new Date().toISOString(),browser:await call('Browser.getVersion'),checks:['same public Todo app in real browser dedicated worker','query/observer/async handler/rollback/plain backend/entity patches','protected OPFS create/write/close/reopen with caller key','missing key rejected before storage open'],protection};
 await writeFile('target/binding-application/browser.json',JSON.stringify(report,null,2)+'\n');console.log('PASS real Chrome application tutorial + protected OPFS reopen');
}finally{socket?.close();browser.kill('SIGTERM');await new Promise(r=>browser.once('exit',r));await new Promise(r=>server.close(r));await rm(profile,{recursive:true,force:true});}
