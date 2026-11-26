export * from './index.js';
export {createWebRuntime} from './web.js';
export {table,column,relation,defineCommand,defineSchema,applicationSchema,ApplicationDatabase} from './application.js';
import {openDatabase as openHost} from './index.js';
import {isApplicationSchema,openApplicationDatabase} from './application.js';
import {createWebRuntime} from './web.js';
export const openDatabase=options=>isApplicationSchema(options.schema)?openApplicationDatabase({...options,createRuntime:createWebRuntime}):openHost(options);
