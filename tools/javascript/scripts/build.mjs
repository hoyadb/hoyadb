// API-0011 local artifacts: no registry access, compiler download or spike imports.
import {mkdir,cp,copyFile,readFile,writeFile,rm,readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {staleSources} from './freshness.mjs';
const here=new URL('../',import.meta.url),repo=new URL('../../',here),output=new URL('dist/',here);
const guidance=['AI.md','skills/hoyadb-integration/SKILL.md'];
async function files(directory){const result=[];for(const entry of await readdir(directory,{withFileTypes:true})){const path=new URL(entry.name+(entry.isDirectory()?'/':''),directory);if(entry.isDirectory())result.push(...await files(path));else result.push(path);}return result;}
// Refuse to package a prebuilt WASM older than the Rust sources or lock this build records beside it.
// Checked before the previous output is removed, so a refused build leaves dist untouched.
const generated=['hoyadb_web.js','hoyadb_web_bg.wasm','hoyadb_web.d.ts','hoyadb_web_bg.wasm.d.ts'].map(file=>new URL('crates/hoyadb-web/pkg/'+file,repo));
const rust=[new URL('crates/hoyadb-web/Cargo.lock',repo)];
for(const directory of ['crates/hoyadb-web/src/','crates/hoyadb-bindings/src/','crates/hoyadb-core/src/','crates/hoyadb-storage-memory/src/','crates/hoyadb-storage-sqlite/src/'])rust.push(...await files(new URL(directory,repo)));
const stale=await staleSources(rust,generated);
if(stale.length){console.error('crates/hoyadb-web/pkg is older than '+stale.length+' Rust source file(s); rebuild it with crates/hoyadb-web/build.sh before packaging. First: '+fileURLToPath(stale[0]).slice(fileURLToPath(repo).length));process.exit(1);}
await rm(output,{recursive:true,force:true});await mkdir(output,{recursive:true});
for(const name of ['client','testing']){
 const source=new URL('packages/'+name+'/',repo),target=new URL(name+'/',output);await mkdir(target,{recursive:true});
 for(const path of ['src','index.d.ts','LICENSES.md'])await cp(new URL(path,source),new URL(path,target),{recursive:true});
 await copyFile(new URL('LICENSE',repo),new URL('LICENSE',target));
 if(name==='client')for(const file of ['web.d.ts','node.d.ts','application.d.ts'])await copyFile(new URL(file,source),new URL(file,target));
 for(const path of guidance){const destination=new URL(path,target);await mkdir(new URL('./',destination),{recursive:true});await copyFile(new URL(path,repo),destination);}
 const manifest=JSON.parse(await readFile(new URL('package.json',source),'utf8'));manifest.files.push('licenses','SOURCE-MANIFEST.json',...guidance);
 await writeFile(new URL('package.json',target),JSON.stringify(manifest,null,2)+'\n');
}
const wasm=new URL('client/src/wasm/',output);await mkdir(wasm,{recursive:true});
for(const file of ['hoyadb_web.js','hoyadb_web_bg.wasm','hoyadb_web.d.ts','hoyadb_web_bg.wasm.d.ts'])await copyFile(new URL('crates/hoyadb-web/pkg/'+file,repo),new URL(file,wasm));
await copyFile(new URL('crates/hoyadb-web/Cargo.lock',repo),new URL('Cargo.lock',wasm));
await cp(new URL('licenses/sqlcipher/',repo),new URL('client/licenses/sqlcipher/',output),{recursive:true});
await cp(new URL('licenses/javascript/',repo),new URL('client/licenses/crates/',output),{recursive:true});
for(const name of ['client','testing'])await writeFile(new URL(name+'/README.md',output),'AI integration: read [AI.md](AI.md) and the [integration skill](skills/hoyadb-integration/SKILL.md) bundled with this package. Repository-relative links below require matching source.\n\n'+await readFile(new URL('README.md',here),'utf8'));
const source=guidance.map(path=>new URL(path,repo));for(const name of ['client','testing'])for(const path of await files(new URL('packages/'+name+'/src/',repo)))source.push(path);
for(const directory of ['crates/hoyadb-web/src/','crates/hoyadb-bindings/src/','crates/hoyadb-core/src/','crates/hoyadb-storage-memory/src/','crates/hoyadb-storage-sqlite/src/'])for(const path of await files(new URL(directory,repo)))source.push(path);
const manifest=[];for(const path of source.sort((a,b)=>a.href.localeCompare(b.href))){const relative=fileURLToPath(path).slice(fileURLToPath(repo).length);manifest.push({path:relative,sha256:createHash('sha256').update(await readFile(path)).digest('hex')});}
const version=JSON.parse(await readFile(new URL('packages/client/package.json',repo),'utf8')).version;
const artifacts=[];for(const file of ['hoyadb_web.js','hoyadb_web_bg.wasm','Cargo.lock']){const bytes=await readFile(new URL(file,wasm));artifacts.push({path:'src/wasm/'+file,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')});}
for(const name of ['client','testing'])await writeFile(new URL(name+'/SOURCE-MANIFEST.json',output),JSON.stringify({abi:'1',version,note:'Pack-time source snapshot. Match the separately retained pre-build manifest before treating it as artifact build provenance.',testControls:false,sources:manifest,artifacts},null,2)+'\n');
console.log('Built local client/testing artifacts from checked-in JS and prebuilt general WASM');
