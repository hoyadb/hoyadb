import test from 'node:test';
import assert from 'node:assert/strict';
import * as web from '../src/web-entry.js';
import * as node from '../src/node-entry.js';
test('APP02 explicit application builders are present on browser and Node entry points',()=>{
 for(const api of [web,node])for(const name of ['table','column','relation','defineCommand','applicationSchema','openDatabase'])assert.ok(api[name],name);
});
test('APP04 command definitions reject hostile inputs without invoking accessors',()=>{
 let calls=0;const handler=()=>null;
 assert.throws(()=>web.defineCommand('test-v1',{get args(){calls++;return {};},handler}),{kind:'InvalidCommand'});assert.equal(calls,0);
 assert.throws(()=>web.defineCommand('test-v1',{args:new Map(),handler}),{kind:'InvalidCommand'});
 assert.throws(()=>web.defineCommand('test-v1',{args:{title:{type:'string',unexpected:true}},handler}),{kind:'InvalidCommand'});
});
test('APP02 a failed source bootstrap does not poison later Node runtime attempts',async()=>{
 // Source checkout has no packaged src/wasm; build.mjs owns that location only in dist.
 const first=node.createNodeRuntime();await assert.rejects(first);const second=node.createNodeRuntime();assert.notEqual(first,second);await assert.rejects(second);
});
