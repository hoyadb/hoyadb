// TC-0046 packed artifact application: no Facade, runtime or test-only database implementation.
import assert from 'node:assert/strict';
import {openDatabase,defineSchema,table,column,defineCommand} from '@hoyadb/client/node';
const schema=defineSchema({version:1,tables:{notes:table({columns:{body:column.string(),at:column.timestamp()}})}});
const db=await openDatabase({schema,engine:'memory'});
try{
 const add=db.command(defineCommand('add-note-v1',{args:{body:'string'},handler:(db,args,ctx)=>db.table('notes').insert({body:args.body,at:ctx.now()})}));
 const row=await add.execute({body:'packed'});assert.equal(row.body,'packed');assert.ok(row.at instanceof Date);
 assert.equal((await db.query('notes').where('body','=','packed').fetch())[0].id,row.id);
 assert.equal((await db.queueStatus()).commands.length,1);
}finally{await db.close();}
console.log('PASS public Node application bootstrap and handler');
