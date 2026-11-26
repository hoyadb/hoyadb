// RN public factory acceptance, separate from the testing package's factory.
import {createNativeRuntime,openMemoryDatabase,j} from '@hoyadb/react-native';

const check=(value,label)=>{if(!value)throw Error(label);};
const rejects=async(work,kind,label)=>{try{await work();}catch(error){check(error.kind===kind,label+': expected '+kind+' received '+error.kind);return;}throw Error(label+': missing '+kind);};
const column={kind:'String',optional:false};
const v1={version:'1',tables:{notes:{columns:{label:column},relations:{}}}};
const v2={version:'2',tables:{notes:{columns:{caption:column},relations:{}}}};
const migration={from_version:'1',to:v2,steps:[{op:'RenameColumn',table:'notes',from:'label',to:'caption'}]};

export async function runFactoryRetention(){
 const runtime=createNativeRuntime();
 const store=await runtime.createMemoryStore();
 let db,dropped=false;
 const namespace='a8'.repeat(16),options={namespace,store};
 try{
  db=await openMemoryDatabase({...options,schema:v1,idGenerator:()=> 'retained-rn'});
  await db.insert('notes',{label:j.string('kept 東京')});
  await rejects(()=>openMemoryDatabase({...options,schema:v1}),'Busy','duplicate active open of the selected store');
  await rejects(()=>runtime.dropMemoryStore(store),'Busy','drop while the selected store is open');
  await db.close();db=null;
  await rejects(()=>openMemoryDatabase({...options,schema:v2,migrations:[{...migration,steps:[]}]}),'MigrationFailure','failed authored reopen');
  db=await openMemoryDatabase({...options,schema:v1});
  const original=await db.query('notes').fetch();
  check(original.length===1&&original[0].id==='retained-rn','RN memory factory retains the selected real store');
  check(original[0].fields.label?.value==='kept 東京','failed migration preserves the original state');
  await db.close();db=null;
  db=await openMemoryDatabase({...options,schema:v2,migrations:[migration]});
  const rows=await db.query('notes').fetch();
  check(rows.length===1&&rows[0].fields.caption?.value==='kept 東京'&&rows[0].fields.label===undefined,'RN retained authored migration');
  await db.close();db=null;
  const ephemeral=await openMemoryDatabase({namespace,schema:v2});
  try{check((await ephemeral.query('notes').fetch()).length===0,'default memory open is a new ephemeral store');}finally{await ephemeral.close();}
  await runtime.dropMemoryStore(store);dropped=true;
  return {name:'RN public memory factory retains store, Busy/drop, failed and authored migration',ok:true};
 }finally{await db?.close();if(!dropped)await runtime.dropMemoryStore(store).catch(()=>{});}
}
