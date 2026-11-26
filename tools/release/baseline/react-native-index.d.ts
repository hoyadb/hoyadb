/** Public native host factories; database value/query/sync types stay shared. */
import type {Database,Handle,HostOptions,Migration,OpenOptions,Runtime,Schema} from '@hoyadb/client/host';
export type * from '@hoyadb/client/host';
export {j,query,defineSchema} from '@hoyadb/client/host';

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
export function openNativeDatabase(options:ProtectedNativeOptions):Promise<Database>;
export function openMemoryDatabase(options:MemoryNativeOptions):Promise<Database>;
