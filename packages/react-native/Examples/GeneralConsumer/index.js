// Ordinary consumer: installed public packages and caller-owned schemas only.
import React,{useEffect,useState} from 'react';
import {AppRegistry,Text,View,Platform} from 'react-native';
import {createNativeRuntime,openMemoryDatabase,openNativeDatabase,j} from '@hoyadb/react-native';
import {runConformance} from './conformance.mjs';
import {runWalkthrough,runWalkthroughMigration} from './walkthrough.mjs';
import {runMixed} from './mixed.mjs';
import {runGuards} from './guards.mjs';
import {runFactoryRetention} from './retained.mjs';

const key=Uint8Array.from({length:32},(_,index)=>index===31?255:index);
const schema={version:'1',tables:{notes:{columns:{body:{kind:'String',optional:false},large:{kind:'Integer',optional:false},ratio:{kind:'Float',optional:false}},relations:{}}}};
const namespace='97979797979797979797979797979797';
const check=(condition,label)=>{if(!condition)throw Error(label);};
const equal=(actual,expected,label)=>check(JSON.stringify(actual)===JSON.stringify(expected),label);
const sleep=milliseconds=>new Promise(resolve=>setTimeout(resolve,milliseconds));
const host={schema,namespace,context:()=>j.string('trusted-consumer'),clock:()=>1000,jitter:()=>0};

async function persistence(mode){
 const options={...host,name:'rn-recovery.sqlite',encryptionKey:key,idGenerator:()=> 'local:test-fixed-id-001'};
 if(mode==='writer'){
  const db=await openNativeDatabase(options);
  try{
   const protection=await db.protection();check(protection.encrypted&&protection.cipher==='SQLCipher','encrypted profile');
   const command=await db.execute({name:'rn-persist-v1',arguments:{},reference_paths:[]},j.object({}),async scope=>{
    await scope.insert('notes',{body:j.string('東京\0🌿'),large:j.integer('9223372036854775807'),ratio:j.float('42.0')});
    return j.object({large:j.integer('9223372036854775807'),ratio:j.float('42.0')});
   });
   const batch=await db.preparePush('1001',0);check(batch.envelopes.length===1,'frozen sent envelope');
   const queue=await db.queueStatus();equal(queue.commands[0].key,command.key,'durable key');check(queue.commands[0].state==='sent','durable sent');
   return {name:'native writer durable queue/rows/profile',ok:true,key:command.key,envelope:batch.envelopes[0]};
  }finally{await db.close();}
 }
 let wrong=false;try{const bad=await openNativeDatabase({...options,encryptionKey:new Uint8Array(32)});await bad.close();}catch{wrong=true;}check(wrong,'wrong key must reject');
 const db=await openNativeDatabase(options);
 try{
  const rows=await db.query('notes').fetch();equal(rows.length,1,'restart retains rows');equal(rows[0].fields.large,j.integer('9223372036854775807'),'restart losslessi64');equal(rows[0].fields.ratio.kind,'float','restart floatkind');
  const queue=await db.queueStatus();check(queue.commands.length===1&&queue.commands[0].state==='sent','restart sent state');
  const due=queue.commands[0].next_due_ms;const retry=await db.preparePush(due,0);check(retry.envelopes.length===1,'restart exact retry');
  await db.receiveWire(await db.runtime.encodeJson(j.array([j.object({idempotencyKey:j.string(queue.commands[0].key),status:j.string('applied'),result:j.null()})])));
  await db.reconcile(queue.commands[0].key,{mappings:{'local:test-fixed-id-001':'server:rn'},writes:[]});
  equal((await db.query('notes').fetch())[0].id,'server:rn','restart guarded map');
  return {name:'independent native reader wrong-key/lossless/retry/mapping',ok:true,envelope:retry.envelopes[0]};
 }finally{await db.close();}
}

async function run(mode){
 check(global.HermesInternal!==undefined,'actual Hermes runtime required');
 if(mode==='writer'||mode==='reader')return [await persistence(mode)];
 check(mode==='all','known consumer mode');
 const runtime=createNativeRuntime();
 const reports=[];
 try{reports.push(await runGuards());}catch(error){reports.push({name:'native original-frame guards',ok:false,error:error.kind??error.message,stack:String(error.stack??'')});}
 try{reports.push(await runFactoryRetention());}catch(error){reports.push({name:'RN retained factory',ok:false,error:error.kind??error.message,stack:String(error.stack??'')});}
 try{reports.push(await runMixed());}catch(error){reports.push({name:'mixed native SDK/RN',ok:false,error:error.kind??error.message,stack:String(error.stack??'')});}
 const memory=options=>openMemoryDatabase(options);
 reports.push(...(await runConformance(runtime,memory)).map(report=>({...report,engine:'native-memory'})));
 const storedNames=new Map();let next=0;
 const protectedOpen=options=>{
  const token=options.store;
  let name=token&&storedNames.get(token);
  if(!name){name=`rn-conformance-${++next}.sqlite`;if(token)storedNames.set(token,name);}
  return openNativeDatabase({...options,name,encryptionKey:key});
 };
 protectedOpen.protected=true;protectedOpen.backupCapability='host_managed';
 reports.push(...(await runConformance(runtime,protectedOpen)).map(report=>({...report,engine:'native-protected'})));
 const walkthrough=await runWalkthrough(runtime,options=>openNativeDatabase({...options,name:'rn-walkthrough.sqlite',encryptionKey:key}));
 reports.push(...walkthrough.reports.map(report=>({...report,engine:'native-protected'})));
 reports.push(await runWalkthroughMigration(runtime,options=>openNativeDatabase({...options,name:'rn-migration.sqlite',encryptionKey:key})));
 return reports;
}

function App({mode='all',reportURL}){
 const [status,setStatus]=useState('Running HoyaDB consumer');
 useEffect(()=>{
  let active=true;
  run(mode).catch(error=>[{name:'consumer',ok:false,error:error.kind??error.message}]).then(async reports=>{
   const result={platform:Platform.OS,mode,hermes:global.HermesInternal!==undefined,reports,failed:reports.filter(report=>!report.ok).length};
   setStatus(JSON.stringify(result));
   const url=reportURL??(Platform.OS==='android'?'http://10.0.2.2:8781/report':'http://127.0.0.1:8781/report');
   for(let retry=0;retry<3;retry++){
    try{const response=await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(result)});if(response.ok)return;}catch{}
    if(active)await sleep(250);
   }
  });
  return()=>{active=false;};
 },[mode,reportURL]);
 return React.createElement(View,{style:{padding:24}},React.createElement(Text,{testID:'hoya-consumer-status'},status));
}
AppRegistry.registerComponent('HoyaConsumer',()=>App);
