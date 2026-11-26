// Only synthetic benchmark frames cross this loopback controller channel.
import{canonical,requireValue}from'./protocol.mjs';
import{proofEmitter}from'./proof.mjs';
export function createHTTPAPI(fetch){
 const admitted=new Map();
 async function request(path,body){requireValue(new TextEncoder().encode(body).length<=262144,'controller frame budget');const response=await fetch(path,{method:'POST',headers:{'Content-Type':'application/json'},body});requireValue(response.ok,'benchmark controller rejected request');return response.json();}
 const known=run=>{requireValue(typeof run==='string'&&admitted.has(run),'unadmitted benchmark run');return admitted.get(run);};
 const route=(operation,run)=>'/benchmark/'+operation+'?run='+encodeURIComponent(run);
 // Warm and cold admissions share one bounded proof transport per exact run.
 async function admit(path,args,pattern){const info=await request(path,canonical(args));requireValue(typeof info?.run_id==='string'&&pattern.test(info.run_id)&&!admitted.has(info.run_id),'invalid controller admission');admitted.set(info.run_id,proofEmitter(body=>request(route('lines',info.run_id),body)));return info;}
 return{
  startCold:args=>admit('/benchmark/start-cold',args,/^cold\.(memory|opfs-protected)\.(create|reopen|upgrade)\.n(0|100|1000|10000)\.r([1-9]|10)$/),
  start:args=>admit('/benchmark/start',args,/^(memory|opfs-protected)\.c[0-9]{2}\.r[1-5]$/),
  startResources:args=>admit('/benchmark/start-resources',args,/^resources\.(memory|opfs-protected)\.r1$/),
  async checkpoint(run,record){known(run);return request(route('checkpoint',run),canonical(record));},
  // Transport of an already parent-issued permit; the page cannot issue, choose or recover one.
  next:()=>request('/benchmark/next','{}'),
  async lines(run,line){return known(run)(line);},
  async sample(run,sample){known(run);return request(route('sample',run),canonical(sample));},
  async finish(run,document){known(run);const reply=await request(route('finish',run),canonical(document));admitted.delete(run);return reply;}
 };
}
