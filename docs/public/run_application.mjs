// SPEC-0042: execute the exact published tutorial against the locally built packages.
import {mkdtemp,mkdir,symlink,copyFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
const root=await mkdtemp(join(tmpdir(),'hoyadb-public-app-'));
try{
 await mkdir(join(root,'node_modules/@hoyadb'),{recursive:true});
 for(const name of ['client','testing'])await symlink(resolve('tools/javascript/dist',name),join(root,'node_modules/@hoyadb',name),'dir');
 await copyFile('examples/application.mjs',join(root,'application.mjs'));
 const result=spawnSync(process.execPath,['application.mjs'],{cwd:root,encoding:'utf8'});process.stdout.write(result.stdout??'');process.stderr.write(result.stderr??'');if(result.status!==0)throw Error('application example failed: '+result.status);
}finally{await rm(root,{recursive:true,force:true});}
