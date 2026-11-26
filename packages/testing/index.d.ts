import type {Database,Runtime,HostOptions,Schema,Migration,Handle,JsonValue,RemoteOutcome,PullTransport} from '@hoyadb/client/host';
import type {ApplicationJson,ApplicationPage} from '@hoyadb/client/application';
export interface MemoryOptions extends HostOptions {runtime:Runtime;schema:Schema;namespace:string;store?:Handle|null;migrations?:Migration[];options?:{capacity?:string;history?:string;initial_delay_ms?:string;multiplier?:string;max_delay_ms?:string}}
export function openMemoryDatabase(options:MemoryOptions):Promise<Database>;
export function createClock(initial?:string|number|bigint):{():string|number|bigint;set(value:string|number|bigint):void};
export interface FakeOptions {runtime:Pick<Runtime,'decodeJson'|'encodeJson'>;handlers?:Record<string,(envelope:JsonValue)=>RemoteOutcome|Promise<RemoteOutcome>>;entityHandlers?:Record<string,(envelope:JsonValue)=>RemoteOutcome|{status:'acknowledged'}|Promise<RemoteOutcome|{status:'acknowledged'}>>;pull?:PullTransport}
export function createFakeTransport(options:FakeOptions):{push(body:string):Promise<string>;pull(since:string|null,scope:string):Promise<string>;pushes:JsonValue[][]};
/** API-0012 plain envelope: a command (idempotencyKey) or an entity write (table, recordId, writeId). */
export type ApplicationEnvelope = {[key:string]:ApplicationJson};
export type ApplicationOutcome = {status:'applied'|'corrected';result:ApplicationJson}|{status:'rejected';reason:string};
/** Without a runtime the fake serves the application binding with plain values. Handlers are looked up
 * by own name; a missing command or entity handler rejects with InvalidCommand (no default outcome).
 * Handlers return exactly one outcome shape; correlation fields are attached by the fake. */
export interface ApplicationFakeOptions {runtime?:undefined;handlers?:Record<string,(envelope:ApplicationEnvelope)=>ApplicationOutcome|Promise<ApplicationOutcome>>;entityHandlers?:Record<string,(envelope:ApplicationEnvelope)=>ApplicationOutcome|{status:'acknowledged'}|Promise<ApplicationOutcome|{status:'acknowledged'}>>;pull?:(since:string|null,scope:string)=>ApplicationPage|Promise<ApplicationPage>}
export function createFakeTransport(options?:ApplicationFakeOptions):{push(envelopes:ApplicationEnvelope[]):Promise<ApplicationEnvelope[]>;pull(since:string|null,scope:string):Promise<ApplicationPage>;pushes:ApplicationEnvelope[][]};
