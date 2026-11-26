// Public native channel + public language SDK coexistence, no private engine API.
import {TurboModuleRegistry} from 'react-native';
import {createNativeRuntime,j} from '@hoyadb/react-native';
import {createRuntime,Database} from '@hoyadb/client/host';
import Foreign from './src/NativeHoyaConsumer';

const deferred=()=>{let resolve;const promise=new Promise(yes=>resolve=yes);return{promise,resolve};};
const check=(condition,label)=>{if(!condition)throw Error(label);};
const schema={version:'1',tables:{mixed_notes:{columns:{label:{kind:'String',optional:false}},relations:{}}}};

export async function runMixed(){
 const native=createNativeRuntime();
 const falseBegin=deferred(),resumeReply=deferred(),nativeWake=deferred();
 let hold=true;
 const runtime=createRuntime({
  ...native.channel,
  async call(input){
   const reply=await native.channel.call(input);
   const frame=JSON.parse(input),value=JSON.parse(reply);
   if(hold&&frame.op==='begin'&&value.ok&&value.value===false){hold=false;falseBegin.resolve();await resumeReply.promise;}
   return reply;
  },
 },{namespace:()=>{throw Error('explicit namespace must bypass entropy');}});
 const capabilities=TurboModuleRegistry.getEnforcing('HoyaDB');
 const db=new Database(runtime,{namespace:'a7'.repeat(16),context:()=>j.string('rn-consumer'),clock:()=>1000,monotonic:()=>capabilities.uptime()});
 await db.open({engine:{kind:'memory',store:null},schema});
 let releaseListener;
 try{
  await Foreign.prepareForeign();
  await Foreign.startForeign();
  let entered=false;
  const queued=db.execute({name:'rn-after-foreign-v1',arguments:{},reference_paths:[]},j.object({}),async scope=>{
   entered=true;await scope.fetch(db.query('mixed_notes'));return j.null();
  });
  await falseBegin.promise;
  check(!entered,'RN must wait while native-language scope owns admission');
  releaseListener=native.channel.subscribeRelease(()=>nativeWake.resolve());
  await Foreign.releaseForeign();
  await nativeWake.promise;
  check(!entered,'suspended false reply has not admitted handler');
  resumeReply.resolve();
  await queued;
  check(entered,'foreign post-C-ABI event wakes RN without polling');
  releaseListener();releaseListener=null;
  // Reverse direction uses the actual public foreign SDK command, not C calls.
  const active=deferred(),finish=deferred();
  const parent=db.execute({name:'rn-before-foreign-v1',arguments:{},reference_paths:[]},j.object({}),async()=>{active.resolve();await finish.promise;return j.null();});
  await active.promise;
  let foreignEntered=false;
  const nativePending=Foreign.startForeign().then(()=>{foreignEntered=true;});
  await new Promise(resolve=>setTimeout(resolve,20));
  check(!foreignEntered,'foreign SDK does not bypass admitted RN scope');
  finish.resolve();await parent;await nativePending;await Foreign.releaseForeign();
  return {name:'actual mixed native SDK/RN FIFO and suspended-false-reply release',ok:true};
 }finally{
  releaseListener?.();resumeReply.resolve();
  await db.close();await Foreign.closeForeign();
 }
}
