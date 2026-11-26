// Cold application fixtures delegate open/migration to the installed SDK/core.
import{nanoseconds,requireValue}from'./protocol.mjs';
export async function coldOpen({bootstrap,open,options,now,onReady}){
 let boot,db;const created=now();
 try{
  boot=await bootstrap();const invoked=now();db=await open({...options,runtime:boot.runtime});const ready=now();
  const sample={status:'ok',correct:true,value:nanoseconds(invoked,ready),worker_creation_to_sdk_ready_ns:nanoseconds(created,ready),elapsed_ms:ready-invoked,start_ms:invoked,end_ms:ready};
  // The caller records its separate READY receipt before any full-row proof.
  await onReady(sample);return{db,boot,sample};
 }catch(error){await db?.close().catch(()=>{});boot?.terminate();throw error;}
}
export function upgradeOptions(source){
 requireValue(source?.version==='1'&&source.tables?.articles?.columns&&!Object.hasOwn(source.tables.articles.columns,'benchmark_extra'),'unexpected upgrade fixture');
 const schema=JSON.parse(JSON.stringify(source)),column={kind:'String',optional:true};schema.version='2';schema.tables.articles.columns.benchmark_extra=column;
 return{schema,migrations:[{from_version:'1',to:schema,steps:[{op:'AddColumn',table:'articles',name:'benchmark_extra',column,default:{kind:'none'}}]}]};
}
