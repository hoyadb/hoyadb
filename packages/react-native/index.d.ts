/** Public native host factories; database value/query/sync types stay shared. */
import type {Database,Handle,HostOptions,Migration,OpenOptions,Runtime,Schema} from '@hoyadb/client/host';
export type * from '@hoyadb/client/host';
export {j,query} from '@hoyadb/client/host';
export {defineSchema,table,column,relation,defineCommand,applicationSchema} from '@hoyadb/client/application';
import type {ApplicationSchema,ApplicationOpenOptions,ApplicationDatabase} from '@hoyadb/client/application';

export interface NativeHostOptions extends HostOptions {
  schema:Schema;
  migrations?:Migration[];
  /** Store open options, named as in the shared OpenOptions. */
  options?:OpenOptions['options'];
}
export interface ProtectedNativeOptions extends NativeHostOptions {
  name:string;
  encryptionKey:Uint8Array;
  backupExcluded?:boolean;
}
export interface MemoryNativeOptions extends NativeHostOptions {store?:Handle|null}

export function createNativeRuntime():Runtime;
export function openNativeDatabase<S extends ApplicationSchema>(options:Omit<ApplicationOpenOptions<S>,'runtime'|'engine'|'filename'>&{name:string;encryptionKey:Uint8Array;backupExcluded?:boolean}):Promise<ApplicationDatabase<S>>;
export function openNativeDatabase(options:ProtectedNativeOptions):Promise<Database>;
export function openMemoryDatabase<S extends ApplicationSchema>(options:Omit<ApplicationOpenOptions<S>,'runtime'|'engine'|'filename'>&{store?:Handle|null}):Promise<ApplicationDatabase<S>>;
export function openMemoryDatabase(options:MemoryNativeOptions):Promise<Database>;
