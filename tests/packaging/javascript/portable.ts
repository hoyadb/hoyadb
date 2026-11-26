// DEF-0026 / TC-0036: strict ES2022-only portable host/testing consumer; no DOM or ambient host types.
import {createRuntime,defineSchema,j,type Definition,type JsonValue,type Schema} from '@hoyadb/client/host';
import {openMemoryDatabase} from '@hoyadb/testing';
const schema:Schema=defineSchema({version:'1',tables:{meters:{columns:{name:{kind:'String',optional:false}},relations:{}}}});
const definition:Definition={name:'one-v1',arguments:{},reference_paths:[]};
const runtime=createRuntime({open:()=>'',call:()=>''},{namespace:()=>new Uint8Array(16)});
const handler=async():Promise<JsonValue>=>j.null();
// A host-supplied capability with exactly the members execute uses: aborted, abort add/remove.
const listeners=new Set<()=>void>();
const signal={aborted:false,
 addEventListener(type:'abort',listener:()=>void,options:{once:true}){void type;void options;listeners.add(listener);},
 removeEventListener(type:'abort',listener:()=>void){void type;listeners.delete(listener);}};
void openMemoryDatabase({runtime,schema,namespace:'00'.repeat(16)}).then(async db=>{
 await db.configureEntitiesDurable('scope',['meters']);
 const resolved:string=await db.resolveEntityId('meters','saved-id');
 const aliased=await db.entityBatchAliased([{op:'insert',table:'meters',alias:'m',fields:{name:j.string('new')}},{op:'patch',table:'meters',id:resolved,fields:{name:j.string('partial')},references:{}}]);
 const mapped:string=aliased.aliases.m;void mapped;void aliased.changes;
 // @ts-expect-error the old operation does not accept the additive patch shape
 await db.entityBatch([{op:'patch',table:'meters',id:resolved,fields:{}}]);
 await db.setConflictPolicy({default:'stop'});
 const result=await db.reconcileResolving('key',{writes:[],mappings:{}},[{table:'meters',id:'row',expected:null}]);
 if(result.kind==='kept_current'){const count:string=result.skipped_writes;void count;}
 await db.rollbackResolving('key');await db.discard('key');
 await db.execute(definition,j.object({}),handler,{signal});
 await db.execute(definition,j.object({}),handler,{});
 await db.execute(definition,j.object({}),handler);
 // @ts-expect-error the capability must expose its aborted state
 await db.execute(definition,j.object({}),handler,{signal:{addEventListener(){},removeEventListener(){}}});
 // @ts-expect-error completion cleanup removes the abort listener
 await db.execute(definition,j.object({}),handler,{signal:{aborted:false,addEventListener(){}}});
 // @ts-expect-error aborted is a boolean state, not a string
 await db.execute(definition,j.object({}),handler,{signal:{aborted:'no',addEventListener(){},removeEventListener(){}}});
 // @ts-expect-error execute registers only the abort event
 await db.execute(definition,j.object({}),handler,{signal:{aborted:false,addEventListener(type:'change'){void type;},removeEventListener(type:'change'){void type;}}});
 // @ts-expect-error a signal is an object capability, not a Boolean flag
 await db.execute(definition,j.object({}),handler,{signal:true});
});
