/** API-0012 / TC-0046. The plain application surface; /host retains tagged DTOs. */
import type {Schema,Table,Column,Relation,Runtime,HostOptions,OpenOptions,Status,Progress,QueryData,BackgroundError,RemoteOutcome,ConflictPolicy,Resolution} from './index.js';
export type {ConflictReaction,ConflictPolicy,Resolution,ResolutionCallback} from './index.js';
export type ApplicationJson = null|boolean|number|bigint|string|ApplicationJson[]|{[key:string]:ApplicationJson};
export type ApplicationField = string|number|boolean|bigint|Date|null;
declare const valueType:unique symbol;
declare const schemaBrand:unique symbol;
declare const tableBrand:unique symbol;
export interface ApplicationTableDefinition extends Table {readonly [tableBrand]:true}
export interface ApplicationColumn<T=ApplicationField, Optional extends boolean=boolean> extends Column {readonly [valueType]?:T;optional:Optional}
export interface ColumnOptions {optional?:boolean}
type OptionalPolicy<O extends ColumnOptions> = O extends unknown?'optional' extends keyof O?Exclude<O['optional'],undefined>|(undefined extends O['optional']?false:never):false:never;
export const column:{
 string<const O extends ColumnOptions={}>(options?:O):ApplicationColumn<string,OptionalPolicy<O>>;
 boolean<const O extends ColumnOptions={}>(options?:O):ApplicationColumn<boolean,OptionalPolicy<O>>;
 float<const O extends ColumnOptions={}>(options?:O):ApplicationColumn<number,OptionalPolicy<O>>;
 reference<const O extends ColumnOptions={}>(options?:O):ApplicationColumn<string,OptionalPolicy<O>>;
 integer<const O extends ColumnOptions&{bigint:true}>(options:O):ApplicationColumn<bigint,OptionalPolicy<O>>;
 integer<const O extends ColumnOptions&{bigint?:false}={}>(options?:O):ApplicationColumn<number,OptionalPolicy<O>>;
 timestamp<const O extends ColumnOptions&{bigint:true}>(options:O):ApplicationColumn<bigint,OptionalPolicy<O>>;
 timestamp<const O extends ColumnOptions&{bigint?:false}={}>(options?:O):ApplicationColumn<Date,OptionalPolicy<O>>;
};
export const relation:{
 toOne(table:string,options:{foreignKey:string;optional?:boolean}):Relation;
 hasMany(table:string,options:{foreignKey:string}):Relation;
 manyToMany(table:string,options:{through:string;sourceRelation:string;targetRelation:string}):Relation;
};
export function table<C extends Record<string,ApplicationColumn>>(input:{columns:C;relations?:Record<string,Relation>}):ApplicationTableDefinition&{columns:C};
export interface ApplicationSchema<T extends Record<string,Table>=Record<string,Table>> extends Schema {readonly [schemaBrand]:true;tables:T}
export function defineSchema<T extends Record<string,ApplicationTableDefinition>>(input:{version:number|string;tables:T}):ApplicationSchema<T>;
export function defineSchema(input:Schema):Schema;
export function applicationSchema(input:Schema|{version:number;tables:Record<string,Table>}):ApplicationSchema;
export function isApplicationSchema(input:unknown):input is ApplicationSchema;
export type PlainRow = {id:string}&Record<string,ApplicationField|ApplicationQuery|undefined>;
type ColumnValue<C> = C extends ApplicationColumn<infer V,boolean>?V:ApplicationField;
type ColumnOptional<C> = C extends ApplicationColumn<unknown,infer O>?O:boolean;
export type RowOf<T extends Table> = {id:string}
 &{[K in keyof T['columns'] as true extends ColumnOptional<T['columns'][K]>?never:K]:ColumnValue<T['columns'][K]>}
 &{[K in keyof T['columns'] as true extends ColumnOptional<T['columns'][K]>?K:never]?:ColumnValue<T['columns'][K]>|null}
 &Record<string,ApplicationField|ApplicationQuery|undefined>;
export interface Unsubscribe {():Promise<unknown>;readonly ready:Promise<void>}
export class ApplicationQuery<R=PlainRow> {
 where(field:string,operator:'='|'!='|'<'|'<='|'>'|'>='|'in'|'Eq'|'Ne'|'Lt'|'Le'|'Gt'|'Ge'|'In',value:unknown):ApplicationQuery<R>;
 whereIn(field:string,values:unknown[]):ApplicationQuery<R>;
 orderBy(field:string,direction?:'asc'|'desc'|'Asc'|'Desc'):ApplicationQuery<R>;
 limit(count:number|string|bigint):ApplicationQuery<R>;
 fetch():Promise<R[]>;
 observe(callback:(rows:R[])=>void|Promise<void>,onError?:(error:Error&{kind:string})=>void):Unsubscribe;
 observeCount(callback:(count:number)=>void|Promise<void>,onError?:(error:Error&{kind:string})=>void):Unsubscribe;
}
export interface RecordHandle<R=PlainRow> {fetch():Promise<R|null>;update(fields:Record<string,unknown>):Promise<R>;remove():Promise<boolean>}
export interface ApplicationTable<R=PlainRow> {
 insert(fields:Record<string,unknown>):Promise<R>;
 findById(id:string):RecordHandle<R>;
 link(id:string,relation:string,target:string,fields?:Record<string,unknown>):Promise<PlainRow>;
 unlink(id:string,relation:string,target:string):Promise<string>;
}
export interface HandlerDatabase {
 query(table:string):ApplicationQuery;
 table(table:string):ApplicationTable;
 related(table:string,id:string,relation:string):ApplicationQuery;
}
export interface HandlerContext {readonly identity:Readonly<ApplicationJson>;now():Date}
export type ArgumentKind='any'|'string'|'reference'|'integer'|'float'|'boolean'|'array'|'object';
export type Argument=ArgumentKind|{type:ArgumentKind;optional?:boolean};
export type ArgumentValue<T> = T extends {type:infer K}?ArgumentValue<K>:T extends 'string'|'reference'?string:T extends 'integer'?number|bigint:T extends 'float'?number:T extends 'boolean'?boolean:T extends 'array'?ApplicationJson[]:T extends 'object'?Record<string,ApplicationJson>:ApplicationJson;
export type ArgumentValues<A extends Record<string,Argument>>={[K in keyof A]:ArgumentValue<A[K]>};
declare const definitionType:unique symbol;
export interface CommandDefinition<A=Record<string,ApplicationJson>,R=unknown> {readonly name:string;readonly [definitionType]:{args:A;result:R}}
export function defineCommand<const A extends Record<string,Argument>,R>(name:string,definition:{args:A;referencePaths?:(string|number)[][];handler:(db:HandlerDatabase,args:ArgumentValues<A>,ctx:HandlerContext)=>R|Promise<R>}):CommandDefinition<ArgumentValues<A>,Awaited<R>>;
export interface CommandOptions {signal?:{readonly aborted:boolean;addEventListener(type:'abort',listener:()=>void,options:{once:true}):void;removeEventListener(type:'abort',listener:()=>void):void}}
export interface BoundCommand<A,R> {execute(args:A,options?:CommandOptions):Promise<R>;executeWithReceipt(args:A,options?:CommandOptions):Promise<{key:string;result:R}>}
export interface ApplicationChange {table:string;record_id:string;op:'create'|'update'|'delete';changes:Record<string,unknown>;removed_fields:string[];timestamp_ms:string;field_timestamps:Record<string,string>}
export interface ApplicationPage {checkpoint:string;complete:boolean;changes:ApplicationChange[]}
export interface ApplicationPlan {mappings:Record<string,string>;writes:({op:'put';table:string;id:string;fields:Record<string,unknown>}|{op:'remove';table:string;id:string}|{op:'entity';change:ApplicationChange})[]}
export type ApplicationOutcome={status:'applied'|'corrected';result:ApplicationJson}|{status:'rejected';reason:string};
export interface ApplicationPending {key:string;outcome:ApplicationOutcome;kind:'command'|'entity';created:Record<string,string>}
export interface ApplicationTransport {push(envelopes:Record<string,ApplicationJson>[]):ApplicationJson[]|Promise<ApplicationJson[]>;pull?(since:string|null,scope:string):ApplicationPage|Promise<ApplicationPage>}
export interface ApplicationOpenOptions<S extends ApplicationSchema=ApplicationSchema> extends Omit<OpenOptions,'runtime'|'schema'|'engine'|'context'|'clock'|'transport'|'reconcile'> {
 schema:S;runtime?:Runtime;engine?:'memory'|OpenOptions['engine'];filename?:string;
 context?:()=>ApplicationJson;clock?:()=>Date|number|bigint|string;
 transport?:ApplicationTransport;reconcile?:(pending:ApplicationPending)=>ApplicationPlan|Promise<ApplicationPlan>;
 entities?:{scope:string;tables:string[]};pullIntervalMs?:number;
}
export class ApplicationDatabase<S extends ApplicationSchema=ApplicationSchema> implements HandlerDatabase {
 readonly raw:import('./index.js').Database;
 query<K extends keyof S['tables']&string>(table:K):ApplicationQuery<RowOf<S['tables'][K]>>;
 table<K extends keyof S['tables']&string>(table:K):ApplicationTable<RowOf<S['tables'][K]>>;
 related(table:string,id:string,relation:string):ApplicationQuery;
 command<A,R>(definition:CommandDefinition<A,R>):BoundCommand<A,R>;
 queueStatus():Promise<Status>;syncProgress():Promise<Progress>;
 configureEntitiesDurable(scope:string,tables:string[]):Promise<void>;resolveEntityId(table:string,id:string):Promise<string>;
 configureEntities(scope:string,tables:string[]):Promise<void>;reconfigureEntities(scope:string,tables:string[]):Promise<void>;
 observeOutcome(key:string,callback:(outcome:ApplicationJson)=>void|Promise<void>):Unsubscribe;
 observeStatus(callback:(status:Status)=>void|Promise<void>):Unsubscribe;
 observeProgress(callback:(progress:Progress)=>void|Promise<void>):Unsubscribe;
 preparePush(now?:Date|string|number|bigint,jitter?:number):Promise<{envelopes:ApplicationJson[];next_due_ms:string|null;has_more:boolean}>;
 receive(outcomes:ApplicationJson[]):Promise<{key:string;ok:boolean;error?:{kind:string}}[]>;
 pendingReconciliations():Promise<ApplicationPending[]>;reconcile(key:string,plan:ApplicationPlan):Promise<null>;
 setConflictPolicy(policy:ConflictPolicy):Promise<null>;reconcileResolving(key:string,plan:ApplicationPlan):Promise<Resolution>;
 rollbackResolving(key:string):Promise<Resolution>;discard(key:string):Promise<Resolution>;
 rollback(key:string):Promise<null>;cancel(key:string):Promise<null>;forget(key:string):Promise<null>;
 sync():Promise<void>;startSync(transport?:ApplicationTransport,reconcile?:ApplicationOpenOptions['reconcile']|null,options?:{pullIntervalMs?:number}):void;stopSync():void;
 reset(mode?:'preserve_recovery'|'discard_recovery'):Promise<void>;close():Promise<void>;
}
export function openApplicationDatabase<S extends ApplicationSchema>(options:ApplicationOpenOptions<S>&{createRuntime?:()=>Runtime|Promise<Runtime>}):Promise<ApplicationDatabase<S>>;
export function encodeJson(value:ApplicationJson):import('./index.js').JsonValue;
export function decodeJson(value:import('./index.js').JsonValue):ApplicationJson;
