import NativeHoyaDB from './NativeHoyaDB';
import {createRuntime,Database} from '@hoyadb/client/host';
import {nativeChannel,nativeNamespace} from './channel.js';
export {j,query} from '@hoyadb/client/host';
export {defineSchema,table,column,relation,defineCommand,applicationSchema} from '@hoyadb/client/application';
import {isApplicationSchema,openApplicationDatabase} from '@hoyadb/client/application';

export function createNativeRuntime() {
  return createRuntime(nativeChannel(NativeHoyaDB), {namespace:()=>nativeNamespace(NativeHoyaDB)});
}

export async function openNativeDatabase(options) {
  const {name,backupExcluded=true,encryptionKey,schema,migrations,options:storeOptions,...host} = options;
  if(isApplicationSchema(schema))return openApplicationDatabase({...host,schema,migrations,options:storeOptions,encryptionKey,runtime:createNativeRuntime(),monotonic:()=>NativeHoyaDB.uptime(),engine:{kind:'native',directory:'@hoyadb/default',name,backup_excluded:backupExcluded}});
  const database = new Database(createNativeRuntime(), {monotonic:()=>NativeHoyaDB.uptime(),...host});
  return database.open({engine:{kind:'native',directory:'@hoyadb/default',name,backup_excluded:backupExcluded},encryptionKey,schema,migrations,options:storeOptions});
}

export async function openMemoryDatabase(options) {
  // store selects an explicit retained memory store; the default null is a new ephemeral store.
  const {schema,migrations,store=null,options:storeOptions,...host} = options;
  if(isApplicationSchema(schema))return openApplicationDatabase({...host,schema,migrations,options:storeOptions,runtime:createNativeRuntime(),monotonic:()=>NativeHoyaDB.uptime(),engine:{kind:'memory',store}});
  const database = new Database(createNativeRuntime(), {monotonic:()=>NativeHoyaDB.uptime(),...host});
  return database.open({engine:{kind:'memory',store},schema,migrations,options:storeOptions});
}
