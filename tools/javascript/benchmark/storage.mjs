// Test-control cleanup/metrics only, outside ordinary SDK latency clocks.
import{exact,requireValue}from'./protocol.mjs';
export async function cleanupOwned(control,{names,poolBefore,canaryHash},hash){
 requireValue(Array.isArray(names)&&names.length===2&&names.every(name=>typeof name==='string'&&/^phase-seven-sdk-owned-bench-[a-z0-9.-]+\.sqlite$/.test(name)&&!name.includes('..'))&&names[1]===names[0].slice(0,-7)+'.neighbor.sqlite','foreign cleanup identity');
 requireValue(Array.isArray(poolBefore)&&poolBefore.length===2&&poolBefore[0]===24&&Number.isInteger(poolBefore[1])&&poolBefore[1]>=0&&poolBefore[1]<=24&&/^[a-f0-9]{64}$/.test(canaryHash));
 const canaryAfter=await hash(await control.file('export',names[1]));exact(canaryAfter,canaryHash);
 const main=await control.file('export',names[0]);requireValue(main instanceof Uint8Array);const footprint={closed_logical_main_bytes:main.byteLength};
 // Closed exact-name journal/WAL lengths; an absent file is recorded as absent, never as zero.
 for(const [key,suffix] of [['closed_journal','-journal'],['closed_wal','-wal']]){const name=names[0]+suffix;footprint[key]=(await control.file('exists',name))[0]?{status:'present',bytes:(await control.file('export',name)).byteLength}:{status:'absent'};}
 for(const name of names){await control.file('remove',name);exact(Array.from(await control.file('exists',name)),[0]);}
 const after=await control.stats();exact(after,poolBefore);
 return{lifecycle:{owner_closed:true,worker_terminated:true,removed_names:[...names],pool_before:[...poolBefore],pool_after:after,canary_before_sha256:canaryHash,canary_after_sha256:canaryAfter},footprint};
}
// Physical SDK pool bytes, read-only, only while no worker holds the pool. Never removes entries.
export async function poolFootprint(root){
 let directory;try{directory=await root.getDirectoryHandle('hoyadb-v1');}catch(error){if(error?.name==='NotFoundError')return{status:'absent',directory:'hoyadb-v1'};throw error;}
 const files={};const walk=async(handle,prefix)=>{for await(const [name,entry]of handle.entries()){const path=prefix+name;if(entry.kind==='directory')await walk(entry,path+'/');else files[path]=(await entry.getFile()).size;}};
 await walk(directory,'');const sorted=Object.fromEntries(Object.entries(files).sort(([a],[b])=>a<b?-1:a>b?1:0));
 return{status:'observed',directory:'hoyadb-v1',regular_files_bytes:sorted,regular_file_count:Object.keys(sorted).length,total_bytes:Object.values(sorted).reduce((sum,value)=>sum+value,0),tool:'navigator.storage.getDirectory() read-only getFile().size; no worker holding the pool'};
}
