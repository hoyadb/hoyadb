// Real TurboModule original-frame guards; no imported engine implementation.
import {TurboModuleRegistry} from 'react-native';
import {createNativeRuntime} from '@hoyadb/react-native';
const check=(value,label)=>{if(!value)throw Error(label);};
const key=Array.from({length:32},(_,index)=>index===31?255:index);
const schema={version:'1',tables:{comets:{columns:{label:{kind:'String',optional:false}},relations:{}}}};
const args={engine:{kind:'native',directory:'@hoyadb/default',name:'rn-guard.sqlite',backup_excluded:true},schema,migrations:[],namespace:'99'.repeat(16),options:{capacity:'1000',history:'100',initial_delay_ms:'1000',multiplier:'2',max_delay_ms:'60000'}};
const frame=value=>JSON.stringify({abi:'1',op:'open',owner:null,args:value});
async function rejected(native,input,secret){
 let raw;
 try{raw=await native.open(input,secret);}
 catch(error){
  check(['InvalidRequest','InvalidPayload','InvalidSchema'].includes(error.code),`unexpected native rejection kind: ${String(error.code)}`);
  return;
 }
 const reply=JSON.parse(raw);
 check(reply.ok===false,'original invalid frame was admitted');
 check(['InvalidRequest','InvalidPayload','InvalidSchema'].includes(reply.error?.kind),'invalid frame failed outside its validation boundary');
}

export async function runGuards(){
 const native=TurboModuleRegistry.getEnforcing('HoyaDB');
 const input=frame(args);
 await rejected(native,'{"abi":"1",'+input.slice(1),key);
 await rejected(native,frame({...args,schema:{...schema,version:1}}),key);
 await rejected(native,frame({...args,namespace:'invalid'}),key);
 await rejected(native,frame({...args,options:{...args.options,capacity:1000}}),key);
 await rejected(native,input,[]);
 await rejected(native,input,Array(31).fill(0));
 await rejected(native,input,Array(32).fill(0.5));
 await rejected(native,input,Array(32).fill(256));
 for(const name of ['../outside','back\\slash','nul\0name'])await rejected(native,frame({...args,engine:{...args.engine,name}}),key);
 await rejected(native,frame({...args,engine:{...args.engine,directory:'/arbitrary-root'}}),key);
 const runtime=createNativeRuntime();
 const memory=await runtime.open({...args,engine:{kind:'memory',store:null}},new Uint8Array(0));
 const protection=await runtime.call('protection',{},memory);
 check(protection.encrypted===false&&protection.cipher==='none'&&protection.backup==='not_applicable','real memory profile');
 await runtime.call('close',{},memory);
 const entropy=await native.namespace();check(Array.isArray(entropy)&&entropy.length===16&&entropy.every(value=>Number.isInteger(value)&&value>=0&&value<=255),'OS entropy bytes');
 const before=native.uptime(),after=native.uptime();check(/^(0|[1-9][0-9]*)$/.test(before)&&/^(0|[1-9][0-9]*)$/.test(after),'monotonic unsigned strings');
 return {name:'actual native original-frame/key/default-root/memory/capability guards',ok:true};
}
