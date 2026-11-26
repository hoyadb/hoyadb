// TC-0036 evidence only: hashes source/ordinary/instrumented artifacts before/after.
import {readFile,readdir,writeFile} from 'node:fs/promises';import {createHash} from 'node:crypto';import {fileURLToPath} from 'node:url';import {execFileSync} from 'node:child_process';
const repo=new URL('../../../',import.meta.url),files=[];
async function visit(relative){for(const entry of await readdir(new URL(relative,repo),{withFileTypes:true})){if(['node_modules','dist','target','pkg','pkg-test','__pycache__'].includes(entry.name))continue;const path=relative+entry.name;if(entry.isDirectory())await visit(path+'/');else if(entry.isFile()&&!entry.name.endsWith('.pyc'))files.push(path);}}
for(const directory of ['tools/javascript/','packages/client/','packages/testing/','tests/packaging/javascript/','crates/hoyadb-web/src/','crates/hoyadb-bindings/src/','crates/hoyadb-core/src/','crates/hoyadb-storage-memory/src/','crates/hoyadb-storage-sqlite/src/'])await visit(directory);
files.push('Cargo.toml','Cargo.lock','crates/hoyadb-web/Cargo.toml','crates/hoyadb-web/Cargo.lock','crates/hoyadb-bindings/Cargo.toml','crates/hoyadb-core/Cargo.toml','crates/hoyadb-storage-memory/Cargo.toml','crates/hoyadb-storage-sqlite/Cargo.toml');
const hash=async path=>{const bytes=await readFile(new URL(path,repo));return {path,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};};
const sources=await Promise.all([...new Set(files)].sort().map(hash)),artifacts=[];
for(const profile of ['pkg','pkg-test'])for(const file of ['hoyadb_web.js','hoyadb_web_bg.wasm','hoyadb_web.d.ts','hoyadb_web_bg.wasm.d.ts'])artifacts.push(await hash('crates/hoyadb-web/'+profile+'/'+file));
const output={profile:'release',abi:'1',node:process.version,platform:process.platform,arch:process.arch,head:execFileSync('git',['rev-parse','HEAD'],{cwd:fileURLToPath(repo),encoding:'utf8'}).trim(),sources,artifacts};
const destination=process.argv[2];if(!destination)throw Error('explicit evidence output path required');await writeFile(destination,JSON.stringify(output,null,2)+'\n');console.log('Frozen '+sources.length+' sources and '+artifacts.length+' ordinary/instrumented artifact files');
