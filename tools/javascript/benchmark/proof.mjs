// Bounded out-of-clock PROOF transport, never a source of expected database rows.
import{requireValue}from'./protocol.mjs';
export function proofEmitter(send,{maxBytes=262144,maxRecords=100}={}){
 requireValue(typeof send==='function'&&Number.isSafeInteger(maxBytes)&&maxBytes>0&&maxBytes<=262144&&Number.isSafeInteger(maxRecords)&&maxRecords>0&&maxRecords<=100);
 let buffer='',bytes=0,records=0,failed=null;const encoder=new TextEncoder();
 async function flush(){if(!buffer)return;const body=buffer;await send(body);buffer='';bytes=0;records=0;}
 return async line=>{
  if(failed)throw failed;
  try{
   requireValue(typeof line==='string'&&line.endsWith('\n')&&!line.slice(0,-1).includes('\n'),'incomplete proof line');const length=encoder.encode(line).length;requireValue(length<=maxBytes,'proof line exceeds transfer budget');
   if(bytes+length>maxBytes)await flush();buffer+=line;bytes+=length;if(line.startsWith('PROOF '))records++;
   if(records>=maxRecords||line==='READY\n'||line.startsWith('PROOF_END '))await flush();
  }catch(error){failed=error;throw error;}
 };
}
