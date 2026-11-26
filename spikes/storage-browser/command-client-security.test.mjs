// API-0010 / TC-0035 S01/S10: host factory policy mocks, not cipher/OPFS proof.
import assert from 'node:assert/strict';
import test from 'node:test';
import { CommandClient } from './command-client.js';

const reply = value => ({ value: JSON.stringify({ ok: true, value }) });

test('S01 protected open rejects missing or malformed key before any RPC', async () => {
  for (const key of [undefined, null, new Uint8Array(31), new Uint8Array(33),
    new Uint16Array(32), new Array(32).fill(7), 'synthetic-key-secret', 32]) {
    let calls = 0;
    const client = new CommandClient(async () => { calls++; return reply('unexpected-owner'); });
    const failure = await client.open('owned.sqlite', { encryptionKey: key }).then(() => null, error => error);
    assert.equal(calls, 0);
    assert.equal(failure?.kind, 'InvalidRequest');
    assert.equal(client.owner, null);
  }
});

test('S10 supplied raw key produces exactly one secure open without client retention', async () => {
  const key = Uint8Array.from({ length: 32 }, (_, index) => index);
  const original = new Uint8Array(key);
  const keyHex = Array.from(key, byte => byte.toString(16).padStart(2, '0')).join('');
  const frames = [];
  const client = new CommandClient(async ({ input }) => {
    const frame = JSON.parse(input); frames.push(frame);
    return reply(frame.op === 'close' ? null : 'secure-owner');
  });
  assert.equal(await client.open('owned.sqlite', { encryptionKey: key, version: '2',
    authored: true, options: { capacity: '7' } }), client);
  assert.equal(frames.length, 1);
  const frame = frames[0];
  assert.equal(frame.op, 'open_secure'); assert.equal(frame.owner, null);
  assert.match(frame.args.namespace, /^[0-9a-f]{32}$/);
  assert.deepEqual(frame.args, { path: 'owned.sqlite', namespace: frame.args.namespace,
    options: { capacity: '7', history: '100', initial_delay_ms: '1000', multiplier: '2', max_delay_ms: '60000' },
    version: '2', authored: true, key_hex: keyHex });
  assert.deepEqual(key, original);
  assert.equal(Object.hasOwn(client, 'encryptionKey'), false);
  assert.equal(Object.hasOwn(client.options, 'encryptionKey'), false);
  assert.equal(JSON.stringify(client).includes(keyHex), false);
  await client.close();
});

test('S10 secure RPC failure never retries as an unprotected open', async () => {
  const frames = [];
  const client = new CommandClient(async ({ input }) => {
    frames.push(JSON.parse(input));
    return { value: JSON.stringify({ ok: false, error: { kind: 'BackendFailure' } }) };
  });
  await assert.rejects(client.open('owned.sqlite', { encryptionKey: new Uint8Array(32) }),
    error => error.kind === 'BackendFailure');
  assert.deepEqual(frames.map(frame => frame.op), ['open_secure']);
  assert.equal(client.owner, null);
  assert.equal(client.generation, 0);
});

// API-0010 S10: a pending opening owns its eventual handle until acknowledged disposal.
const deferred = () => { let resolve; const promise=new Promise(yes=>{resolve=yes;}); return {promise,resolve}; };
const tick = () => new Promise(resolve=>setImmediate(resolve));
function lifecycleFixture({holdClose=false,failOpen=false,failClose=false}={}) {
  const opened=deferred(),closed=deferred(),frames=[],live=new Set();let count=0,closeCalls=0;
  const client=new CommandClient(async({input})=>{
    const frame=JSON.parse(input);frames.push(frame);
    if(['open','open_secure'].includes(frame.op)) {
      const owner=String(++count);live.add(owner);
      if(count===1) {
        await opened.promise;
        if(failOpen){live.delete(owner);return {value:JSON.stringify({ok:false,error:{kind:'BackendFailure'}})};}
      }
      return reply(owner);
    }
    if(frame.op==='close') {
      closeCalls++;
      if(failClose&&closeCalls===1)return {value:JSON.stringify({ok:false,error:{kind:'BackendFailure'}})};
      if(holdClose)await closed.promise;
      live.delete(frame.owner);
    }
    return reply(null);
  });
  return {client,frames,live,opened,closed,closeCalls:()=>closeCalls,
    release:()=>{opened.resolve();closed.resolve();}};
}
const factory=(client,secure,path='phase-four-lifecycle.sqlite',key=9)=>secure?
  client.open(path,{encryptionKey:new Uint8Array(32).fill(key)}):client.openUnprotected(path);

test('S10 duplicate protected or legacy opens are Busy before a second RPC',async()=>{
  for(const secure of [true,false]) {
    const f=lifecycleFixture();const first=factory(f.client,secure).catch(error=>error);
    try {
      for(const duplicateSecure of [true,false])await assert.rejects(factory(f.client,duplicateSecure,'phase-four-duplicate.sqlite'),{kind:'Busy'});
      assert.equal(f.frames.filter(frame=>['open','open_secure'].includes(frame.op)).length,1);
    }finally{f.release();await first;await f.client.close().catch(()=>{});}
  }
});

test('S10 close during pending open never publishes a late handle and waits for disposal acknowledgement',async()=>{
  for(const secure of [true,false]) {
    const f=lifecycleFixture({holdClose:true});
    const opening=factory(f.client,secure).then(value=>({value}),error=>({error}));
    let completed=false;const closing=f.client.close().then(()=>{completed=true;});
    try {
      await tick();assert.equal(completed,false,'close cannot finish while its open reply is pending');
      f.opened.resolve();const outcome=await opening;assert.equal(outcome.error?.kind,'Closed');
      await tick();assert.equal(f.client.owner,null,'cancelled late reply must not publish an owner');
      assert.equal(f.closeCalls(),1);assert.equal(completed,false,'close must wait for disposal acknowledgement');
      assert.equal(f.live.size,1);
      await assert.rejects(factory(f.client,secure,'phase-four-before-cleanup.sqlite'),{kind:'Busy'});
      f.closed.resolve();await closing;assert.equal(f.live.size,0);
    }finally{f.release();await opening;await closing;await f.client.close().catch(()=>{});}
  }
});

test('S10 close waits for a failed opening without inventing an owner close',async()=>{
  const f=lifecycleFixture({failOpen:true});
  const opening=factory(f.client,true).then(()=>null,error=>error);
  let completed=false;const closing=f.client.close().then(()=>{completed=true;});
  try {
    await tick();assert.equal(completed,false);
    f.opened.resolve();assert.equal((await opening)?.kind,'Closed');await closing;
    assert.equal(f.closeCalls(),0);assert.equal(f.client.owner,null);assert.equal(f.live.size,0);
  }finally{f.release();await opening;await closing;await f.client.close().catch(()=>{});}
});

test('S10 failed late-handle disposal retains ownership for one shared retry',async()=>{
  const f=lifecycleFixture({failClose:true,holdClose:true});
  const opening=factory(f.client,true).then(()=>null,error=>error);
  const failedClose=f.client.close().then(()=>null,error=>error);
  try {
    f.opened.resolve();assert.equal((await opening)?.kind,'Closed');
    assert.equal((await failedClose)?.kind,'BackendFailure');assert.equal(f.live.size,1);
    assert.equal(f.client.owner,null);
    await assert.rejects(factory(f.client,false,'phase-four-orphan-replacement.sqlite'),{kind:'Busy'});
    const retry=f.client.close(),shared=f.client.close();await tick();
    assert.equal(f.closeCalls(),2,'concurrent closes share the one retry');
    f.closed.resolve();await Promise.all([retry,shared]);assert.equal(f.live.size,0);
  }finally{f.release();await opening;await failedClose;await f.client.close().catch(()=>{});}
});

test('S10 a fresh open after cancelled-open cleanup uses only fresh configuration',async()=>{
  const f=lifecycleFixture();const opening=factory(f.client,true,'phase-four-old-key.sqlite',7).catch(error=>error);
  const closing=f.client.close();
  try {
    f.opened.resolve();assert.equal((await opening)?.kind,'Closed');await closing;
    await factory(f.client,true,'phase-four-new-key.sqlite',19);
    const opens=f.frames.filter(frame=>frame.op==='open_secure');assert.equal(opens.length,2);
    assert.deepEqual(opens.map(frame=>[frame.args.path,frame.args.key_hex]),[
      ['phase-four-old-key.sqlite','07'.repeat(32)],['phase-four-new-key.sqlite','13'.repeat(32)]]);
    assert.notEqual(opens[0].args.namespace,opens[1].args.namespace);
    assert.equal(f.client.owner,'2');assert.equal(JSON.stringify(f.client).includes('07'.repeat(32)),false);
  }finally{f.release();await opening;await closing;await f.client.close().catch(()=>{});}
});
