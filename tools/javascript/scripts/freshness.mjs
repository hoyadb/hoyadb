// Packaging guard: a prebuilt artifact is only packaged with sources it can have been built from.
// Modification times cannot prove provenance; they only refuse the certain mismatch of a source
// changed after the artifact was produced. A missing artifact or source rejects.
import {stat} from 'node:fs/promises';
export async function staleSources(sources,artifacts){
 let oldest=Infinity;
 for(const artifact of artifacts)oldest=Math.min(oldest,(await stat(artifact)).mtimeMs);
 const stale=[];
 for(const source of sources)if((await stat(source)).mtimeMs>oldest)stale.push(source.href);
 return stale;
}
