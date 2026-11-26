// DEF-0026 regression 1: packed RN/client/testing public entries, ES2022 only, never executed.
import {createNativeRuntime,j,openMemoryDatabase} from '@hoyadb/react-native';
import type {Database,Definition,JsonValue} from '@hoyadb/react-native';
import {openMemoryDatabase as openTestingMemory} from '@hoyadb/testing';
const schema={version:'1',tables:{notes:{columns:{body:{kind:'String' as const,optional:false}},relations:{}}}};
const definition:Definition={name:'note-v1',arguments:{},reference_paths:[]};
const handler=async():Promise<JsonValue>=>j.null();
// An RN host's own cancellation object with only the members the shared client uses.
const signal={aborted:false,
 addEventListener(type:'abort',listener:()=>void,options:{once:true}){void type;void listener;void options;},
 removeEventListener(type:'abort',listener:()=>void){void type;void listener;}};
export async function probe():Promise<void>{
 const runtime=createNativeRuntime();
 const native:Database=await openMemoryDatabase({schema,store:null});
 const shared:Database=await openTestingMemory({runtime,schema,namespace:'a8'.repeat(16)});
 await native.execute(definition,j.object({}),handler,{signal});
 await shared.execute(definition,j.object({}),handler,{signal});
 // @ts-expect-error the capability must expose its aborted state
 await native.execute(definition,j.object({}),handler,{signal:{addEventListener(){},removeEventListener(){}}});
 // @ts-expect-error completion cleanup removes the abort listener
 await shared.execute(definition,j.object({}),handler,{signal:{aborted:false,addEventListener(){}}});
 await Promise.all([native.close(),shared.close()]);
}
