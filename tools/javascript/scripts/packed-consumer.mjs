// Independent local npm artifact install; no registry/network or checkout imports.
import {mkdtemp,readFile,writeFile,copyFile,mkdir,rm,cp} from 'node:fs/promises';
import {tmpdir} from 'node:os';import {join} from 'node:path';import {fileURLToPath} from 'node:url';import {spawnSync,spawn} from 'node:child_process';
const base=new URL('../',import.meta.url),root=await mkdtemp(join(tmpdir(),'hoyadb-sdk-consumer-'));
const run=(command,args,cwd=root)=>{const result=spawnSync(command,args,{cwd,encoding:'utf8'});process.stdout.write(result.stdout||'');process.stderr.write(result.stderr||'');if(result.status!==0)throw Error(command+' failed '+result.status);return result.stdout;};
try{
 const artifacts=new URL('../../target/packages/javascript/tarballs/',base);await mkdir(artifacts,{recursive:true});
 const archives=[];for(const name of ['client','testing']){const output=run('npm',['pack','--json','--ignore-scripts','--pack-destination',fileURLToPath(artifacts)],fileURLToPath(new URL('dist/'+name,base)));archives.push(join(fileURLToPath(artifacts),JSON.parse(output)[0].filename));}
 await writeFile(join(root,'package.json'),JSON.stringify({private:true,type:'module'}));run('npm',['install','--offline','--ignore-scripts','--no-audit','--no-fund',...archives]);
 for(const file of ['schemas.mjs'])await copyFile(new URL('../../packages/client/test/'+file,base),join(root,file));
 const source=await readFile(new URL('../../tests/packaging/javascript/conformance.mjs',base),'utf8');await writeFile(join(root,'conformance.mjs'),source.replace("'../../../packages/client/src/index.js'","'@hoyadb/client/host'").replace("'../../../packages/testing/src/index.js'","'@hoyadb/testing'").replace("'../../../packages/client/test/schemas.mjs'","'./schemas.mjs'"));
 const memory=await readFile(new URL('../../tests/packaging/javascript/memory.mjs',base),'utf8');await writeFile(join(root,'memory.mjs'),memory.replace("'../../../crates/hoyadb-web/pkg/hoyadb_web.js'","'@hoyadb/client/wasm'").replace("'../../../packages/client/src/index.js'","'@hoyadb/client/host'").replace("new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url)","new URL(import.meta.resolve('@hoyadb/client/wasm/hoyadb_web_bg.wasm'))"));
 const walkthrough=await readFile(new URL('../../tests/packaging/javascript/walkthrough.mjs',base),'utf8');await writeFile(join(root,'walkthrough.mjs'),walkthrough.replace("'../../../packages/client/src/index.js'","'@hoyadb/client/host'").replace("'../../../packages/testing/src/index.js'","'@hoyadb/testing'"));
 const offline=await readFile(new URL('../../tests/packaging/javascript/offline-memory.mjs',base),'utf8');await writeFile(join(root,'offline-memory.mjs'),offline.replace("'../../../crates/hoyadb-web/pkg/hoyadb_web.js'","'@hoyadb/client/wasm'").replace("'../../../packages/client/src/index.js'","'@hoyadb/client/host'").replace("'../../../packages/testing/src/index.js'","'@hoyadb/testing'").replace("new URL('../../../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm',import.meta.url)","new URL(import.meta.resolve('@hoyadb/client/wasm/hoyadb_web_bg.wasm'))"));
 await copyFile(new URL('../../tests/packaging/javascript/types.ts',base),join(root,'types.ts'));
 await copyFile(new URL('../../tests/packaging/javascript/application.mjs',base),join(root,'application.mjs'));
 await copyFile(new URL('../../tests/packaging/javascript/application-types.ts',base),join(root,'application-types.ts'));
 await copyFile(new URL('../../tests/packaging/javascript/application-row-types.ts',base),join(root,'application-row-types.ts'));
 run('tsc',['--noEmit','--strict','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--lib','ES2022','application-types.ts','application-row-types.ts']);
 run('node',['application.mjs']);
 run('tsc',['--noEmit','--strict','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--lib','ES2022,DOM,ESNext.Disposable','types.ts']);
 // DEF-0026: the portable host/testing declarations also typecheck without DOM or ambient host types.
 await copyFile(new URL('../../tests/packaging/javascript/portable.ts',base),join(root,'portable.ts'));
 await writeFile(join(root,'portable.tsconfig.json'),JSON.stringify({compilerOptions:{strict:true,noEmit:true,target:'ES2022',module:'NodeNext',moduleResolution:'NodeNext',lib:['ES2022'],types:[],skipLibCheck:false},files:['portable.ts']}));
 run('tsc',['-p','portable.tsconfig.json']);
 run('node',['memory.mjs']);run('node',['offline-memory.mjs']);console.log('PASS independent packed npm consumer: types + actual WASM/core memory');
 if(process.argv.slice(2).includes('--serve')){
  const imports='<script type="importmap">'+JSON.stringify({imports:{'@hoyadb/client':'./node_modules/@hoyadb/client/src/web-entry.js','@hoyadb/client/host':'./node_modules/@hoyadb/client/src/index.js','@hoyadb/testing':'./node_modules/@hoyadb/testing/src/index.js'}})+'</script>';
  for(const file of ['browser.mjs','browser.html','control-client.mjs','control-worker.mjs','control.mjs','control.html','worker-retry.mjs','worker-retry.html','consumer_server.py']){
   let text=await readFile(new URL('../../tests/packaging/javascript/'+file,base),'utf8');
   text=text.replaceAll("'../../../tools/javascript/dist/client/src/web-entry.js'","'@hoyadb/client'").replaceAll("'../../../packages/client/src/index.js'","'@hoyadb/client/host'").replaceAll("'../../../packages/client/test/schemas.mjs'","'./schemas.mjs'").replaceAll("'../../../packages/client/src/worker-host.js'","'./node_modules/@hoyadb/client/src/worker-host.js'").replaceAll("'../../../tools/javascript/dist/client/src/worker.js'","'./node_modules/@hoyadb/client/src/worker.js'").replaceAll("'../../../crates/hoyadb-web/pkg-test/hoyadb_web.js'","'./test-wasm/hoyadb_web.js'").replaceAll("'../../../crates/hoyadb-web/pkg/hoyadb_web.js'","'./node_modules/@hoyadb/client/src/wasm/hoyadb_web.js'");
   if(file.endsWith('.html'))text=text.replace(/<script type="importmap">[\s\S]*?<\/script>/g,'').replace('<script type="module"',imports+'<script type="module"');
   await writeFile(join(root,file),text);
  }
  await cp(new URL('../../crates/hoyadb-web/pkg-test/',base),join(root,'test-wasm'),{recursive:true});
  const server=spawn('python3',[join(root,'consumer_server.py'),'--port','8878'],{cwd:root,stdio:['ignore','pipe','pipe']});server.stdout.pipe(process.stdout);server.stderr.pipe(process.stderr);
  const stop=()=>server.kill('SIGTERM');process.once('SIGTERM',stop);process.once('SIGINT',stop);
  console.log('PACKED_BROWSER_SERVER http://127.0.0.1:8878/browser.html parent='+process.pid+' child='+server.pid+' root='+root);
  await new Promise((resolve,reject)=>{server.once('error',reject);server.once('exit',(code,signal)=>code===0||signal==='SIGTERM'?resolve():reject(Error('fixture server failed '+code)));});
  process.removeListener('SIGTERM',stop);process.removeListener('SIGINT',stop);
 }

}finally{await rm(root,{recursive:true,force:true});console.log('Removed exact task-owned temporary consumer '+root);}
