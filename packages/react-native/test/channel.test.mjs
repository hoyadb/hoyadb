import test from 'node:test';
import assert from 'node:assert/strict';
import {nativeChannel,nativeNamespace} from '../src/channel.js';

test('owned binary key travels separately; exact complete frame/reply stays opaque',async()=>{
 const calls=[];const reply='{"abi":"1","ok":false,"error":{"kind":"WrongKey"}}';
 const native={open:async(input,key)=>{calls.push([input,key]);return reply;},call:async input=>input,onAdmissionReleased:()=>({remove(){}})};
 const channel=nativeChannel(native);const key=Uint8Array.from({length:32},(_,i)=>i===31?255:i);
 const pending=channel.open('original frame',key);key.fill(0);
 assert.equal(await pending,reply);assert.deepEqual(calls[0],['original frame',Array.from({length:32},(_,i)=>i===31?255:i)]);
 assert.equal(await channel.call('opaque call'),'opaque call');
});

test('payload-free release listener is synchronous and removal idempotent/stale guarded',()=>{
 let emit,removals=0,calls=0;
 const channel=nativeChannel({open(){},call(){},onAdmissionReleased:listener=>{emit=listener;return{remove(){removals++;}};}});
 const stop=channel.subscribeRelease(()=>calls++);emit();assert.equal(calls,1);stop();stop();emit();
 assert.equal(calls,1);assert.equal(removals,1);
});

test('platform namespace accepts exactly16 byte integers without weak fallback',async()=>{
 assert.deepEqual(await nativeNamespace({namespace:async()=>Array.from({length:16},(_,i)=>i)}),Uint8Array.from({length:16},(_,i)=>i));
 for(const value of [[],Array(16).fill(0.5),Array(16).fill(256),Array(16).fill(-1)])await assert.rejects(nativeNamespace({namespace:async()=>value}),error=>error.kind==='InvalidRequest');
});
