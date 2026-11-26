import {check,failure,publicFailure} from './errors.js';
import {copyOwned,handle,wire} from './values.js';
import {utf8Length} from './utf8.js';
export class Runtime {
 constructor(channel,options={}){check(channel&&typeof channel.open==='function'&&typeof channel.call==='function');this.channel=channel;this.options=options;}
 async request(op,args={},owner=null,key){
  const input=wire({abi:'1',op,owner,args});let text;
  try{text=await(key===undefined?this.channel.call(input):this.channel.open(input,key));}catch(error){throw publicFailure(error);}
  check(typeof text==='string','InvalidPayload');utf8Length(text,1048576);let reply;
  try{reply=copyOwned(JSON.parse(text));}catch(error){throw error?.kind?error:failure('InvalidPayload');}
  check(reply&&reply.abi==='1'&&typeof reply.ok==='boolean','InvalidPayload');
  check(Object.keys(reply).sort().join(',')===(reply.ok?'abi,ok,value':'abi,error,ok'),'InvalidPayload');
  if(!reply.ok){check(reply.error&&Object.keys(reply.error).join(',')==='kind','InvalidPayload');throw publicFailure(reply.error);}
  return reply.value;
 }
 call(op,args={},owner=null){return this.request(op,args,owner);}
 async open(args,key){return handle(await this.request('open',args,null,key));}
 async entropy(){check(typeof this.options.namespace==='function','InvalidRequest');const bytes=await this.options.namespace();check(bytes instanceof Uint8Array&&bytes.byteLength===16,'InvalidRequest');return bytes;}
 async namespace(){return Array.from(await this.entropy(),byte=>byte.toString(16).padStart(2,'0')).join('');}
 async jitter(){
  // Rejection sampling avoids modulo bias. Core alone computes retry deadlines.
  for(let attempt=0;attempt<16;attempt++){const bytes=await this.entropy();for(let i=0;i<16;i+=2){const sample=bytes[i]*256+bytes[i+1];if(sample<62525)return sample%2501;}}
  throw failure('BackendFailure');
 }
 decodeJson(body){return this.call('json_decode',{body});}
 encodeJson(value){return this.call('json_encode',{value});}
 async createMemoryStore(){return handle(await this.call('memory_store'));}
 dropMemoryStore(store){return this.call('memory_drop',{store:handle(store)});}
}
export const createRuntime=(channel,options)=>new Runtime(channel,options);
