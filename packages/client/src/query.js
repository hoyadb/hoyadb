import {copyOwned,string,unsigned} from './values.js';
export class Query {
 constructor(table,source=null,data=null,database=null){this.data=copyOwned(data||{table:string(table),predicates:[],order:null,limit:null,source});this.database=database;}
 where(field,operator,value){return new Query('',null,{...this.data,predicates:[...this.data.predicates,{field:string(field),operator:string(operator),values:[value]}]},this.database);}
 whereIn(field,values){return new Query('',null,{...this.data,predicates:[...this.data.predicates,{field:string(field),operator:'In',values}]},this.database);}
 orderBy(field,direction){return new Query('',null,{...this.data,order:{field:string(field),direction:string(direction)}},this.database);}
 limit(count){return new Query('',null,{...this.data,limit:unsigned(count)},this.database);}
 toJSON(){return copyOwned(this.data);}
 fetch(){return this.database.fetch(this);}
 observe(callback,mode='membership',columns=[]){return this.database.observe(this,callback,mode,columns);}
}
export const query=(table,source=null)=>new Query(table,source);
export const queryWire=value=>value instanceof Query?value.toJSON():copyOwned(value);
