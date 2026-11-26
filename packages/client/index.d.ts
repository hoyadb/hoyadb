/** API-0011 ABI1 / TC-0036. Decimal strings preserve full wire widths. */
export type I64 = string;
export type U64 = string;
export type Handle = string;
export type NullValue = {kind:'null'};
export type JsonValue = NullValue | {kind:'boolean';value:boolean} | {kind:'string'|'integer'|'float';value:string} | {kind:'array';value:JsonValue[]} | {kind:'object';value:Record<string,JsonValue>};
export type Cell = NullValue | {kind:'boolean';value:boolean} | {kind:'string'|'reference'|'integer'|'timestamp'|'float';value:string};
export type Fields = Record<string,Cell>;
export interface Row {id:string;fields:Fields}
export interface Change {table:string;id:string;before:Row|null;after:Row|null}
export interface Column {kind:'String'|'Integer'|'Float'|'Boolean'|'Timestamp'|'Reference';optional:boolean}
export type Relation = {kind:'ToOne';table:string;foreign_key:string;optional:boolean} | {kind:'HasMany';table:string;foreign_key:string} | {kind:'ManyToMany';table:string;through:string;source_relation:string;target_relation:string};
export interface Table {columns:Record<string,Column>;relations:Record<string,Relation>}
export interface Schema {version:string;tables:Record<string,Table>}
export type MigrationStep = {op:'AddTable';table:string;definition:Table} | {op:'DropTable';table:string} | {op:'AddColumn';table:string;name:string;column:Column;default:{kind:'none'}|{kind:'some';value:Cell}} | {op:'DropColumn';table:string;name:string} | {op:'RenameColumn';table:string;from:string;to:string} | {op:'Backfill';table:string;column:string;value:Cell} | {op:'SetRelations';table:string;relations:Record<string,Relation>};
export interface Migration {from_version:string;to:Schema;steps:MigrationStep[]}
export type Operator = 'Eq'|'Ne'|'Lt'|'Le'|'Gt'|'Ge'|'In';
export interface Source {table:string;id:string;relation:string}
export interface QueryData {table:string;predicates:{field:string;operator:Operator;values:Cell[]}[];order:{field:string;direction:'Asc'|'Desc'}|null;limit:string|null;source:Source|null}
export type QueryInput = Query|QueryData;
export type ObservationMode = 'membership'|'columns'|'count';
export class Query {
 constructor(table:string,source?:Source|null);
 where(field:string,operator:Exclude<Operator,'In'>,value:Cell):Query;
 whereIn(field:string,values:Cell[]):Query;
 orderBy(field:string,direction:'Asc'|'Desc'):Query;
 limit(count:string|number|bigint):Query;
 toJSON():QueryData;
 fetch():Promise<Row[]>;
 observe(callback:EventCallback,mode?:ObservationMode,columns?:string[]):Promise<Handle>;
}
export function query(table:string,source?:Source|null):Query;
export type Write = {op:'insert';table:string;fields:Fields;proposed_id?:string|null} | {op:'replace';table:string;id:string;fields:Fields} | {op:'remove';table:string;id:string};
/** SPEC-0048: explicit references resolve only earlier configured entity insert aliases. */
export type AliasedWrite = {op:'insert';table:string;fields:Fields;proposed_id?:string|null;alias?:string|null;references?:Record<string,string>} | {op:'replace'|'patch';table:string;id:string;fields:Fields;references?:Record<string,string>} | {op:'remove';table:string;id:string};
export interface AliasedBatchResult {changes:Change[];aliases:Record<string,string>}
export interface Definition {name:string;arguments:Record<string,{kind:'Any'|'String'|'Integer'|'Float'|'Boolean'|'Array'|'Object';optional:boolean}>;reference_paths:({key:string}|{index:string})[][]}
export interface Captured {identity:JsonValue;now_ms:I64}
export interface Scope {
 readonly args:JsonValue;
 readonly context:Readonly<Captured>;
 fetch(query:QueryInput):Promise<Row[]>;
 insert(table:string,fields:Fields):Promise<Row>;
 replace(table:string,id:string,fields:Fields):Promise<Row>;
 remove(table:string,id:string):Promise<boolean>;
 batch(writes:Write[]):Promise<Change[]>;
 link(table:string,id:string,relation:string,target:string,fields?:Fields):Promise<Row>;
 unlink(table:string,id:string,relation:string,target:string):Promise<string>;
}
export interface Protection {encrypted:boolean;cipher:string;version:string;backup:'host_managed'|'unsupported'|'not_applicable'}
export interface DiagnosticSnapshot {schema_version:string;pending:string;sending:string;awaiting_outcome:string;live:string;blocked:boolean;ready:boolean;pages:string;changes:string;tombstones:string}
export interface QueueEntry {kind:'command'|'entity';table:string|null;record_id:string|null;key:string;state:'queued'|'sent'|'acknowledged'|'applied'|'corrected'|'rejected'|'cancelled'|'discarded';blocked_on:string[];awaiting_local:boolean;awaiting_key:string|null;rolled_back:boolean;attempts:string;next_due_ms:I64|null}
export interface Status {pending:string;sending:string;awaiting_outcome:string;live:string;blocked:boolean;commands:QueueEntry[]}
export interface Progress {phase:'not_started'|'bootstrapping'|'ready';ready:boolean;pages:I64;changes:I64;tombstones:string}
export type Event = {kind:'data';subscription:Handle;value:{kind:'rows';rows:Row[]}|{kind:'count';count:string}|{kind:'error';error:{kind:string}}} | {kind:'status';subscription:Handle;value:Status} | {kind:'progress';subscription:Handle;value:Progress} | {kind:'outcome';subscription:Handle;value:{state:'queued'|'sent'|'acknowledged'}|{state:'applied'|'corrected';result:JsonValue}|{state:'rejected';reason:string}} | {kind:'error';subscription:Handle;value:{kind:string}};
/** SPEC-0036: invoked synchronously in event order; a returned Promise is not awaited. A throw or later rejection retires the listener. A `{kind:'error'}` event is terminal; data error values are recoverable. */
export type EventCallback = (event:Event)=>void|Promise<void>;
export type BackgroundSource='observation'|'callback'|'sync'|'merge'|'timer';
/** SPEC-0036 redacted background failure: no message, key, payload or row is ever included. */
export interface BackgroundError {readonly source:BackgroundSource;readonly kind:string;readonly token:Handle|null}
export type RemoteOutcome = {status:'applied'|'corrected';result:JsonValue}|{status:'rejected';reason:string};
export interface EntityChange {table:string;record_id:string;op:'create'|'update'|'delete';changes:Fields;removed_fields:string[];timestamp_ms:I64;field_timestamps:Record<string,I64>}
export interface PullPage {checkpoint:string;complete:boolean;changes:EntityChange[]}
export interface CorrectionGuard {table:string;id:string;expected:Fields|null}
export interface Plan {writes:({op:'put';table:string;id:string;fields:Fields}|{op:'remove';table:string;id:string}|{op:'entity';change:EntityChange})[];mappings:Record<string,string>}
export interface PendingReconciliation {key:string;outcome:RemoteOutcome}
export type ConflictReaction='stop'|'discard';
export interface ConflictPolicy {default:ConflictReaction;commands?:Record<string,ConflictReaction>}
export type Resolution={readonly kind:'completed'}|{readonly kind:'kept_current';readonly discarded_keys:readonly string[];readonly unmapped:readonly string[];readonly skipped_writes:U64};
/** SPEC-0040: each current-epoch KeptCurrent invokes this callback once. Failure is reported without retiring it or changing the committed result. */
export type ResolutionCallback=(key:string,resolution:Extract<Resolution,{kind:'kept_current'}>)=>void|PromiseLike<void>;
export type PushTransport = (body:string)=>Promise<string>|string;
export type PullTransport = (since:string|null,scope:string)=>Promise<string>|string;
export interface Transport {push:PushTransport;pull?:PullTransport}
export interface Channel {open(input:string,key:Uint8Array):Promise<string>|string;call(input:string):Promise<string>|string;close?():void;subscribeRelease?(listener:()=>void):()=>void}
export interface RuntimeOptions {namespace:()=>Uint8Array|Promise<Uint8Array>}
export class Runtime {
 constructor(channel:Channel,options?:RuntimeOptions);
 readonly channel:Channel;
 call(op:string,args?:object,owner?:Handle|null):Promise<unknown>;
 open(args:object,key:Uint8Array):Promise<Handle>;
 namespace():Promise<string>;
 jitter():Promise<number>;
 decodeJson(body:string):Promise<JsonValue>;
 encodeJson(value:JsonValue):Promise<string>;
 createMemoryStore():Promise<Handle>;
 dropMemoryStore(store:Handle):Promise<null>;
}
export function createRuntime(channel:Channel,options?:RuntimeOptions):Runtime;
export interface Metadata {fields:Record<string,{timestamp_ms:I64;authoritative:boolean}>;tombstone:{timestamp_ms:I64;authoritative:boolean}|null}
export interface MergeInput {schema:Schema;current:Row|null;metadata:Metadata;incoming:EntityChange;default:{row:Row|null;metadata:Metadata}}
export type MergeHook = (input:Readonly<MergeInput>)=>Row|null|Promise<Row|null>;
export interface HostTimers {set(work:()=>void,delay:number):unknown;clear(handle:unknown):void}
export interface HostOptions {conflict?:ConflictPolicy;onResolution?:ResolutionCallback;onBackgroundError?:(error:BackgroundError)=>void;monotonic?:()=>U64|number|bigint;timers?:HostTimers;mergeHooks?:Record<string,MergeHook>;context?:()=>JsonValue;clock?:()=>I64|number|bigint;idGenerator?:()=>string;jitter?:()=>number;namespace?:string}
export type Engine = {kind:'memory';store:Handle|null}|{kind:'opfs';name:string}|{kind:'native';directory:string;name:string;backup_excluded:boolean};
export interface OpenOptions extends HostOptions {runtime:Runtime;schema:Schema;engine:Engine;encryptionKey?:Uint8Array;migrations?:Migration[];options?:{capacity?:string;history?:string;initial_delay_ms?:I64;multiplier?:string;max_delay_ms?:I64};transport?:Transport|PushTransport;reconcile?:(pending:PendingReconciliation)=>Plan|Promise<Plan>}
export class Database {
 constructor(runtime:Runtime,options?:HostOptions);
 readonly owner:Handle|null;
 open(options:Omit<OpenOptions,'runtime'>|OpenOptions):Promise<this>;
 close():Promise<void>;
 schema():Promise<Schema>;
 protection():Promise<Protection>;
 diagnostics():Promise<DiagnosticSnapshot>;
 query(table:string,source?:Source|null):Query;
 related(table:string,id:string,relation:string):Query;
 fetch(query:QueryInput):Promise<Row[]>;
 insert(table:string,fields:Fields):Promise<Row>;
 replace(table:string,id:string,fields:Fields):Promise<Row>;
 remove(table:string,id:string):Promise<boolean>;
 batch(writes:Write[]):Promise<Change[]>;
 link(table:string,id:string,relation:string,target:string,fields?:Fields):Promise<Row>;
 unlink(table:string,id:string,relation:string,target:string):Promise<string>;
 execute(definition:Definition,args:JsonValue,handler:(scope:Scope)=>JsonValue|Promise<JsonValue>,options?:{signal?:{readonly aborted:boolean;addEventListener(type:'abort',listener:()=>void,options:{once:true}):void;removeEventListener(type:'abort',listener:()=>void):void}}):Promise<{key:string;result:JsonValue}>;
 observe(query:QueryInput,callback:EventCallback,mode?:ObservationMode,columns?:string[]):Promise<Handle>;
 observeData(query:QueryInput,callback:EventCallback,mode?:ObservationMode,columns?:string[]):Promise<Handle>;
 unsubscribe(subscription:Handle):Promise<boolean>;
 drain(now?:U64|number|bigint):Promise<number>;
 refresh():Promise<null>;
 observationStats():Promise<{groups:string;observers:string;projected_queries:string;candidate_updates:string;materialized_rows:string}>;
 queueStatus():Promise<Status>;
 observeStatus(callback:EventCallback):Promise<Handle>;
 observeOutcome(key:string,callback:EventCallback):Promise<Handle>;
 observeProgress(callback:EventCallback):Promise<Handle>;
 preparePush(now?:I64|number|bigint,jitter?:number):Promise<{envelopes:string[];next_due_ms:I64|null;has_more:boolean}>;
 receive(key:string,outcome:RemoteOutcome|null):Promise<null>;
 receiveEntity(table:string,record_id:string,write_id:string,outcome:RemoteOutcome|null):Promise<null>;
 receiveWire(body:string):Promise<{key:string;ok:boolean;error?:{kind:string}}[]>;
 requestRedelivery(key:string,now:I64|number|bigint):Promise<null>;
 pendingReconciliations():Promise<PendingReconciliation[]>;
 setConflictPolicy(policy:ConflictPolicy):Promise<null>;
 reconcile(key:string,plan:Plan):Promise<null>;
 reconcileGuarded(key:string,plan:Plan,guards:CorrectionGuard[]):Promise<null>;
 reconcileResolving(key:string,plan:Plan,guards?:CorrectionGuard[]):Promise<Resolution>;
 rollbackResolving(key:string):Promise<Resolution>;
 discard(key:string):Promise<Resolution>;
 rollback(key:string):Promise<null>;
 cancel(key:string):Promise<null>;
 forget(key:string):Promise<null>;
 configureEntities(scope:string,tables:string[]):Promise<null>;
 configureEntitiesDurable(scope:string,tables:string[]):Promise<null>;
 resolveEntityId(table:string,id:string):Promise<string>;
 reconfigureEntities(scope:string,tables:string[]):Promise<null>;
 entityLink(table:string,id:string,relation:string,target:string,fields?:Fields):Promise<Row>;
 entityUnlink(table:string,id:string,relation:string,target:string):Promise<string>;
 setMergeHooks(hooks:Record<string,MergeHook>):void;
 entityBatch(writes:Write[]):Promise<Change[]>;
 entityBatchAliased(writes:AliasedWrite[]):Promise<AliasedBatchResult>;
 patch(table:string,id:string,fields:Fields):Promise<Row>;
 syncProgress():Promise<Progress>;
 pullOnce(transport:PullTransport):Promise<Progress>;
 reset(mode?:'preserve_recovery'|'discard_recovery'):Promise<void>;
 startSync(transport:Transport|PushTransport,reconcile?:(pending:PendingReconciliation)=>Plan|Promise<Plan>):void;
 stopSync():void;
 sync():Promise<void>;
}
export function openDatabase(options:OpenOptions):Promise<Database>;
export function defineSchema(schema:Schema):Schema;
export function copyOwned<T>(value:T):T;
export function immutableOwned<T>(value:T):Readonly<T>;
export const j:{null():NullValue;boolean(value:boolean):{kind:'boolean';value:boolean};string(value:string):{kind:'string';value:string};reference(value:string):{kind:'reference';value:string};integer(value:I64|number|bigint):{kind:'integer';value:string};timestamp(value:I64|number|bigint):{kind:'timestamp';value:string};float(value:string|number):{kind:'float';value:string};array(value:JsonValue[]):{kind:'array';value:JsonValue[]};object(value:Record<string,JsonValue>):{kind:'object';value:Record<string,JsonValue>}};
