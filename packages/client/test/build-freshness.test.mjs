// tools/javascript build: a prebuilt WASM older than a source it claims must not be packaged.
import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,utimes,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {join} from 'node:path';
import {staleSources} from '../../../tools/javascript/scripts/freshness.mjs';

test('R12 sources modified after the prebuilt artifact are reported, older and equal ones are not',async()=>{
 const directory=await mkdtemp(join(tmpdir(),'hoyadb-freshness-'));try{
  const file=async(name,seconds)=>{const path=join(directory,name);await writeFile(path,name);await utimes(path,seconds,seconds);return pathToFileURL(path);};
  const artifact=await file('artifact.wasm',2000),older=await file('older.rs',1000),equal=await file('equal.rs',2000),newer=await file('newer.rs',3000);
  assert.deepEqual(await staleSources([older,equal],[artifact]),[]);
  assert.deepEqual(await staleSources([older,newer,equal],[artifact]),[newer.href]);
  const early=await file('early.js',500);
  assert.deepEqual(await staleSources([older],[artifact,early]),[older.href],'every artifact must be at least as new');
  await assert.rejects(staleSources([older],[pathToFileURL(join(directory,'missing.wasm'))]));
 }finally{await rm(directory,{recursive:true,force:true});}
});
