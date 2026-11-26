// Benchmark lifecycle only; supplied factories are the actual installed SDK.
import{SDKBenchmark}from'./harness.mjs';
import{FrameMetrics}from'./run.mjs';
import{cleanupOwned}from'./storage.mjs';
import{coldOpen,upgradeOptions}from'./cold.mjs';
import{requireValue}from'./protocol.mjs';
const nameValid=name=>typeof name==='string'&&/^phase-seven-sdk-owned-bench-[a-z0-9.-]+\.sqlite$/.test(name)&&!name.includes('..');
const coldDeclared=(engine,{operation,N}={})=>(operation==='create'&&N===0)||(engine==='opfs-protected'&&((operation==='reopen'&&[100,1000,10000].includes(N))||(operation==='upgrade'&&N===1000)));
export function createSDKPlatform({metadata,bootstrap,bootstrapResources,createRuntime,openMemoryDatabase,openDatabase,startControl,key,hash,now,poolSizes}){
 const base=()=>({schema:metadata.schema,namespace:metadata.namespace,options:metadata.engine_options,context:()=>({kind:'null'}),clock:()=>1000,jitter:()=>0});
 const ownedNames=(info,engine)=>{requireValue(engine==='memory'||engine==='opfs-protected');const names=info.owned_names;requireValue(Array.isArray(names)&&(engine==='memory'?names.length===0:names.length===2&&names.every(nameValid)&&names[1]===names[0].slice(0,-7)+'.neighbor.sqlite'),'foreign benchmark identity');return names;};
 const protectedOpen=(options,runtime,name)=>openDatabase({...options,runtime,engine:{kind:'opfs',name},encryptionKey:key()});
 // Controls prove absent names and close the neighbouring canary before any ordinary worker.
 async function prepare(names,owned){
  const physical=await physicalPool();const control=await startControl();
  try{
   const poolBefore=await control.stats();
   for(const name of names)requireValue((await control.file('exists',name))[0]===0,'pre-existing benchmark file');
   // Only names proven absent above may be removed on a partial setup failure.
   owned.push(...names);
   const canaryDb=await protectedOpen(base(),control.runtime,names[1]);await canaryDb.close();
   return{poolBefore,canaryHash:await hash(await control.file('export',names[1])),physical};
  }finally{control.stop();}
 }
 // Physical pool bytes are read only while no SDK or controls worker holds the pool.
 const physicalPool=async()=>{if(!poolSizes)return{status:'unknown',reason:'unavailable'};try{return await poolSizes();}catch(error){return{status:'unknown',reason:typeof error?.name==='string'?error.name:'BackendFailure'};}};
 async function protectedCleanup(names,prepared){const cleanup=await startControl();let result;try{result=await cleanupOwned(cleanup,{names,...prepared},hash);}finally{cleanup.stop();}return{lifecycle:result.lifecycle,footprint:{...result.footprint,physical_pool_before:prepared.physical,physical_pool_after:await physicalPool()}};}
 async function removeOwned(owned){if(!owned.length)return;const cleanup=await startControl();try{for(const name of owned)if((await cleanup.file('exists',name))[0])await cleanup.file('remove',name);}finally{cleanup.stop();}}
 function retained(engine,names,prepared,{db,boot,harness,protection,sample}){
  let closed=false,terminated=false,footprint=null;
  return{harness,protection,sample,get footprint(){return footprint;},async close(){await db.close();closed=true;},terminate(){boot.terminate();terminated=true;},async cleanup(){
   requireValue(closed&&terminated,'cleanup requires retired owner/worker');
   if(engine==='memory')return{owner_closed:true,worker_terminated:true,removed_names:[],pool_before:null,pool_after:null,canary_before_sha256:null,canary_after_sha256:null};
   const result=await protectedCleanup(names,prepared);footprint=result.footprint;return result.lifecycle;
  }};
 }
 const checkProtection=(engine,protection)=>requireValue(engine==='memory'?protection.encrypted===false&&protection.cipher==='none':protection.encrypted===true&&protection.cipher==='SQLCipher','wrong benchmark protection');
 return{
 // Untimed resource class: one harness worker for all thirty cycles (browser-plan resource classes).
 async launchResources(info,engine){
  const cohort=info.cohort??{};requireValue(cohort.operation==='retained-resources'&&cohort.N===100&&cohort.Q===0&&cohort.observers===1&&cohort.mode==='membership'&&cohort.sharing==='equivalent','undeclared resource class');
  const names=ownedNames(info,engine),options=base(),owned=[];let prepared,boot,open=null,next=0,terminated=false;
  try{
   if(engine==='opfs-protected')prepared=await prepare(names,owned);
   boot=await bootstrapResources();
  }catch(error){boot?.terminate();await removeOwned(owned);throw error;}
  const runtime=boot.runtime;let footprint=null;
  return{get footprint(){return footprint;},linearBytes:()=>boot.linearBytes(),
   async open(cycle){
    requireValue(cycle===next&&open===null&&!terminated&&cycle<30,'resource cycles must be sequential and closed before reopen');next++;
    // Memory: a fresh database per cycle. Protected: cycle0 seeds, later cycles reopen the same closed store.
    const db=engine==='memory'?await openMemoryDatabase({...options,runtime}):await protectedOpen(options,runtime,names[0]);open=db;
    try{const protection=await db.protection();checkProtection(engine,protection);
     const harness=new SDKBenchmark({db,runtime,n:100,now});if(engine==='memory'||cycle===0)await harness.seed();return harness;}
    catch(error){await db.close().catch(()=>{});open=null;throw error;}
   },
   async close(harness){requireValue(open!==null&&harness.db===open);await open.close();open=null;},
   terminate(){boot.terminate();terminated=true;},
   async cleanup(){
    requireValue(open===null&&terminated,'cleanup requires retired owner/worker');
    if(engine==='memory')return{owner_closed:true,worker_terminated:true,removed_names:[],pool_before:null,pool_after:null,canary_before_sha256:null,canary_after_sha256:null};
    const result=await protectedCleanup(names,prepared);footprint=result.footprint;return result.lifecycle;
   }};
 },
 async launch(info,engine){
  const names=ownedNames(info,engine),options=base(),owned=[];let prepared,boot,db;
  try{
   if(engine==='opfs-protected')prepared=await prepare(names,owned);
   boot=await bootstrap();const metrics=new FrameMetrics(boot.runtime.channel,now),runtime=createRuntime(metrics.channel);
   db=engine==='memory'?await openMemoryDatabase({...options,runtime}):await protectedOpen(options,runtime,names[0]);
   const protection=await db.protection();checkProtection(engine,protection);
   return retained(engine,names,prepared,{db,boot,protection,harness:new SDKBenchmark({db,runtime,n:info.cohort.N,now,metrics})});
  }catch(error){await db?.close().catch(()=>{});boot?.terminate();await removeOwned(owned);throw error;}
 },
 async launchCold(info,engine,{onReady}){
  requireValue(coldDeclared(engine,info.cohort),'undeclared cold class');
  const names=ownedNames(info,engine),{operation,N}=info.cohort,options=base(),owned=[];let prepared,opened;
  try{
   if(engine==='opfs-protected')prepared=await prepare(names,owned);
   if(operation!=='create'){
    // Untimed closed v1 fixture, seeded through the public SDK by its own retired worker.
    const setup=await bootstrap();let db;
    try{db=await protectedOpen(options,setup.runtime,names[0]);await new SDKBenchmark({db,runtime:setup.runtime,n:N,now}).seed();await db.close();db=null;}
    finally{await db?.close().catch(()=>{});setup.terminate();}
   }
   const measured=operation==='upgrade'?{...options,...upgradeOptions(metadata.schema)}:options;
   opened=await coldOpen({bootstrap,now,onReady,options:measured,open:engine==='memory'?openMemoryDatabase:value=>openDatabase({...value,engine:{kind:'opfs',name:names[0]},encryptionKey:key()})});
   const protection=await opened.db.protection();checkProtection(engine,protection);
   requireValue((await opened.db.schema()).version===measured.schema.version,'unexpected cold schema version');
   return retained(engine,names,prepared,{db:opened.db,boot:opened.boot,protection,sample:opened.sample,harness:new SDKBenchmark({db:opened.db,runtime:opened.boot.runtime,n:N,now})});
  }catch(error){await opened?.db.close().catch(()=>{});opened?.boot.terminate();await removeOwned(owned);throw error;}
 }};
}
