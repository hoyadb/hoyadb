// TC-0036 ordinary packaged worker; explicit instrumented cleanup is separate.
import {createWebRuntime,openDatabase,j} from '../../../tools/javascript/dist/client/src/web-entry.js';
import {schemas} from '../../../packages/client/test/schemas.mjs';
import {runConformance,runActualHttp} from './conformance.mjs';
import {runWalkthrough,runWalkthroughMigration} from './walkthrough.mjs';
import {startControl} from './control-client.mjs';
const storageKey='hoya-phase-seven-sdk-consumer',output=document.querySelector('#output');
const saved=JSON.parse(sessionStorage.getItem(storageKey)||'null');let reports=saved?.reports??[],owned=saved?.owned??[],baseline=saved?.baseline;
const show=()=>{output.textContent=JSON.stringify(reports,null,2);};show();
const record=(name,ok,error)=>{reports.push({name,ok,...(error?{error}:{})});show();};
const equal=(a,b,label)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw Error(label);};
const reject=async(work,kind)=>{try{await work();}catch(error){equal(error.kind,kind,'expected '+kind+' received '+error.kind);return;}throw Error('missing rejection '+kind);};
const key=new Uint8Array(32).fill(23),namespace='7777'.repeat(8),reloadName='phase-seven-sdk-owned-normal-reload-20261024.sqlite';
function remember(name){if(!owned.includes(name))owned.push(name);sessionStorage.setItem(storageKey,JSON.stringify({owned,reports,baseline}));return name;}
const common={namespace,context:()=>j.null(),clock:()=>1000,jitter:()=>0};
async function cleanup(){const control=await startControl();try{for(const name of owned)if((await control.file('exists',name))[0])await control.file('remove',name);const stats=await control.stats();if(baseline)equal(stats,baseline,'foreign pool inventory baseline preserved');return stats;}finally{control.stop();}}
try{
 if(!saved?.stage){baseline=await cleanup();owned=[];reports=[];}
 const runtime=await createWebRuntime();
 const open=(schema,name,extra={})=>openDatabase({...common,...extra,runtime,schema,engine:{kind:'opfs',name:remember(name)},encryptionKey:extra.encryptionKey??key});
 if(saved?.stage==='prepared'){
  try{await reject(()=>open(schemas[1],reloadName,{encryptionKey:new Uint8Array(32).fill(9)}),'BackendFailure');record('G16 ordinary package wrong-key rejection has no plaintext fallback',true);const db=await open(schemas[1],reloadName);try{equal((await db.query('sensors').fetch())[0].fields.label,j.string('page reload 東京\0🌿'),'ordinary reload persisted row');const status=await db.queueStatus();equal(status.commands[0].attempts,'1','durable attempt after actual reload');equal((await db.preparePush(1999,0)).envelopes,[],'reload no early retry');equal((await db.preparePush(2000,0)).envelopes,saved.frozen,'reload exact frozen bytes');record('G17 ordinary package actual page reload retains queue and frozen bytes',true);}finally{await db.close();}}catch(error){record('G17 ordinary package actual page reload retains queue and frozen bytes',false,error.kind??error.message);}
  runtime.channel.close();try{await cleanup();record('G16 exact task-owned logical cleanup preserves shared pool baseline',true);}catch(error){record('G16 exact task-owned logical cleanup preserves shared pool baseline',false,error.kind??error.message);}sessionStorage.removeItem(storageKey);globalThis.hoyaSdkResult={done:true,reports};
 }else{
  let counter=0;const factory=options=>open(options.schema,'phase-seven-sdk-owned-normal-case-'+(++counter)+'-20261024.sqlite',options);factory.protected=true;factory.retainedUnsupported=true;
  reports.push(...await runConformance(runtime,factory));show();
  reports.push(...await runActualHttp(runtime,options=>open(options.schema,'phase-seven-sdk-owned-normal-http-20261024.sqlite',options)));show();
  const walkthroughFactory=options=>open(options.schema,'phase-seven-sdk-owned-normal-walkthrough-20261024.sqlite',options);walkthroughFactory.protected=true;
  const walked=await runWalkthrough(runtime,walkthroughFactory);reports.push(...walked.reports);reports.push(await runWalkthroughMigration(runtime,options=>open(options.schema,'phase-seven-sdk-owned-normal-walkthrough-migration-20261024.sqlite',options)));show();
  const name='phase-seven-sdk-owned-normal-migration-20261024.sqlite',column=(kind,optional=false)=>({kind,optional});
  const v1={version:'1',tables:{sensors:{columns:{label:column('String'),old:column('String')},relations:{}},obsolete:{columns:{label:column('String')},relations:{}}}},v2={version:'2',tables:{sensors:{columns:{caption:column('String'),optional:column('String',true),added:column('String')},relations:{}},auxiliary:{columns:{label:column('String')},relations:{}}}};
  const migration={from_version:'1',to:v2,steps:[{op:'AddTable',table:'auxiliary',definition:v2.tables.auxiliary},{op:'DropTable',table:'obsolete'},{op:'AddColumn',table:'sensors',name:'optional',column:column('String',true),default:{kind:'some',value:j.null()}},{op:'AddColumn',table:'sensors',name:'added',column:column('String'),default:{kind:'some',value:j.string('initial')}},{op:'DropColumn',table:'sensors',name:'old'},{op:'RenameColumn',table:'sensors',from:'label',to:'caption'},{op:'Backfill',table:'sensors',column:'added',value:j.string('filled')},{op:'SetRelations',table:'sensors',relations:{}}]};
  let db;try{db=await open(v1,name,{idGenerator:()=> 'migrated'});await db.insert('sensors',{label:j.string('before'),old:j.string('drop')});await db.close();db=null;await reject(()=>open(v2,name,{migrations:[{...migration,steps:[]}]}),'MigrationFailure');db=await open(v1,name);equal((await db.query('sensors').fetch())[0].fields.label,j.string('before'),'failed protected reopen preserves original');await db.close();db=await open(v2,name,{migrations:[migration]});equal(await db.query('sensors').fetch(),[{fields:{added:j.string('filled'),caption:j.string('before'),optional:j.null()},id:'migrated'}],'all seven protected migrations exact endpoint');await db.close();db=null;record('G02 ordinary protected authored all-seven migration and failure preservation',true);}catch(error){record('G02 ordinary protected authored all-seven migration and failure preservation',false,error.kind??error.message);}finally{await db?.close().catch(()=>{});}
  db=await open(schemas[1],reloadName,{idGenerator:()=> 'reload-row'});await db.insert('sensors',{label:j.string('page reload 東京\0🌿')});await db.execute({name:'reload-v1',arguments:{},reference_paths:[]},j.object({}),async()=>j.integer('9223372036854775807'));const frozen=(await db.preparePush(1000,0)).envelopes;await db.close();runtime.channel.close();sessionStorage.setItem(storageKey,JSON.stringify({stage:'prepared',owned,reports,baseline,frozen}));globalThis.hoyaSdkResult={done:false,phase:'reload-ready',reports};output.textContent+='\nReload this page for the persisted-owner stage.';
 }
}catch(error){record('ordinary packaged browser consumer',false,error.kind??error.message);globalThis.hoyaSdkResult={done:true,reports};}
