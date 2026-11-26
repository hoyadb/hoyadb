// API-0012 row optionality regression. Compile only; the runnable Todo tour covers runtime.
import {defineSchema,table,column,openDatabase} from '@hoyadb/client/node';
const schema = defineSchema({version:1,tables:{notes:table({columns:{
  title:column.string(), note:column.string({optional:true}),
  count:column.integer({bigint:true}), maybeCount:column.integer({bigint:true,optional:true}),
  at:column.timestamp(), maybeAt:column.timestamp({optional:true}),
  done:column.boolean(), score:column.float({optional:true}),
  uncertain:column.string<{optional:true}|{}>({optional:true})
}})}});
async function rowTypeChecks() {
  const db = await openDatabase({schema,engine:'memory'});
  const [row] = await db.query('notes').fetch();
  const title:string = row.title;
  const count:bigint = row.count;
  const at:Date = row.at;
  const done:boolean = row.done;
  const note:string|null|undefined = row.note;
  const maybeCount:bigint|null|undefined = row.maybeCount;
  const maybeAt:Date|null|undefined = row.maybeAt;
  const score:number|null|undefined = row.score;
  // @ts-expect-error A union policy that may be optional must remain conservative.
  const uncertainRequired:string = row.uncertain;
  void uncertainRequired;
  if (row.note !== null) {
    // @ts-expect-error A null check alone does not rule out an absent optional field.
    row.note.length;
  }
  // @ts-expect-error Missing optional fields are undefined, not just null.
  const missingUndefined:string|null = row.note;
  if (row.note !== null && row.note !== undefined) row.note.toUpperCase();
  void [title,count,at,done,note,maybeCount,maybeAt,score,missingUndefined];
  await db.close();
}
void rowTypeChecks;
