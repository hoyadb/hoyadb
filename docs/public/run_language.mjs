// Run public package consumers without changing the workspace or installing anything.
import {mkdtemp,mkdir,symlink,copyFile,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
const name=process.argv[2];
if(!['typescript','react-native'].includes(name))throw Error('expected typescript or react-native');
const root=await mkdtemp(join(tmpdir(),'hoyadb-language-'));
function run(command,args){
 const result=spawnSync(command,args,{cwd:root,encoding:'utf8'});
 process.stdout.write(result.stdout??'');process.stderr.write(result.stderr??'');
 if(result.error)throw result.error;
 if(result.status!==0)throw Error(command+' failed: '+result.status);
}
try{
 await mkdir(join(root,'node_modules/@hoyadb'),{recursive:true});
 for(const pkg of ['client','testing'])await symlink(resolve('tools/javascript/dist',pkg),join(root,'node_modules/@hoyadb',pkg),'dir');
 await writeFile(join(root,'package.json'),'{"type":"module"}');
 if(name==='typescript'){
  await copyFile('examples/typescript.ts',join(root,'typescript.ts'));
  await copyFile('tests/packaging/javascript/application-row-types.ts',join(root,'application-row-types.ts'));
  run('tsc',['--strict','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--lib','ES2022,DOM','--outDir','out','typescript.ts','application-row-types.ts']);
  run(process.execPath,['out/typescript.js']);
 }else{
  await copyFile('examples/react-native.mjs',join(root,'react-native.mjs'));
  await writeFile(join(root,'run.mjs'),`import {openDatabase} from '@hoyadb/client';
import {schema,backend,runTodo} from './react-native.mjs';
const db=await openDatabase({schema,engine:'memory',context:()=>({user:'ada'}),entities:{scope:'react-native-todos',tables:['todos']},transport:backend,reconcile:()=>({mappings:{},writes:[]})});
await runTodo(db); console.log('react-native shared tour: ok (WASM; native bootstrap not compiled or executed)');`);
  run(process.execPath,['run.mjs']);
 }
}finally{await rm(root,{recursive:true,force:true});}
