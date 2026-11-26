import {defineSchema,table,column,defineCommand,openDatabase} from '@hoyadb/client/node';
const schema=defineSchema({version:1,tables:{notes:table({columns:{body:column.string(),at:column.timestamp(),count:column.integer({bigint:true})}})}});
const db=await openDatabase({schema,engine:'memory'});
await db.configureEntitiesDurable('scope',['notes']);
const resolved:string=await db.resolveEntityId('notes','saved-id');void resolved;
const rows=await db.query('notes').where('body','=','test').fetch();
const at:Date|null=rows[0].at;const count:bigint|null=rows[0].count;
const command=db.command(defineCommand('test-v1',{args:{body:'string'},handler:(scope,args,ctx)=>scope.table('notes').insert({body:args.body,at:ctx.now(),count:1n})}));
await command.execute({body:'test'});
// @ts-expect-error schema shorthand keeps command argument types
await command.execute({body:123});
await db.close();void at;void count;
import {createFakeTransport} from '@hoyadb/testing';
const backend=createFakeTransport({handlers:{'add-v1':envelope=>({status:'applied',result:envelope.args})},pull:()=>({checkpoint:'v1',complete:true,changes:[]})});
const withBackend=await openDatabase({schema,engine:'memory',transport:backend});
await withBackend.close();

// TC-0044 CP13: the ordinary application entry exposes lossless immutable reports.
const withPolicy=await openDatabase({schema,engine:'memory',conflict:{default:'stop',commands:{'test-v1':'discard'}},onResolution(key,result){
 const keyText:string=key;const countText:string=result.skipped_writes;void keyText;void countText;
 // @ts-expect-error resolution arrays are immutable snapshots
 result.discarded_keys.push('mutable');
 // @ts-expect-error full-width counters are decimal strings, never Numbers
 const lossy:number=result.skipped_writes;void lossy;
}});
await withPolicy.setConflictPolicy({default:'discard'});
const resolution=await withPolicy.reconcileResolving('key',{writes:[],mappings:{}});
if(resolution.kind==='kept_current'){const keys:readonly string[]=resolution.discarded_keys;void keys;}
await withPolicy.rollbackResolving('key');await withPolicy.discard('key');
// @ts-expect-error conflict reactions use the exact approved vocabulary
await withPolicy.setConflictPolicy({default:'retry'});
await withPolicy.close();
