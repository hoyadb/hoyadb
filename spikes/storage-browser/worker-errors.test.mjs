// API-0010 / TC-0035 S10/S13: actual worker dispatcher, no engine substitute claimed.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

const source = await readFile(new URL('./worker.js', import.meta.url), 'utf8');
const canary = 'secret-key-path-identity-table-record-東京';
async function workerWithFailure(error) {
  let next;
  const replies = [];
  const surface = { postMessage: value => { replies.push(structuredClone(value)); next?.(); } };
  class FileSystemFileHandle { createSyncAccessHandle() {} }
  const context = vm.createContext({ self: surface, crypto: { randomUUID: () => 'fixture' },
    isSecureContext: true, crossOriginIsolated: false, navigator: { storage: { getDirectory() {} } },
    FileSystemFileHandle, WebAssembly: {}, BigInt, URL });
  const exports = { default: async () => {}, initialize: async () => {},
    BrowserStore: class { constructor() { throw error; } }, shutdown() {},
    security_conformance: (group, index, key) => ({group, index, encrypted:key.length === 64}),
    CommandFixture: class {
      rpc(input) {
        const frame = JSON.parse(input);
        return JSON.stringify(frame.op === 'open_secure' && frame.args.reject ?
          { ok: false, error: { kind: 'BackendFailure' } } :
          { ok: true, value: frame.op === 'open_secure' ? 'secure-owner' : null });
      }
      free() {}
    } };
  const bridge = new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [name, value] of Object.entries(exports)) this.setExport(name, value);
  }, { context });
  await bridge.link(() => {}); await bridge.evaluate();
  const worker = new vm.SourceTextModule(source, { context,
    initializeImportMeta: meta => { meta.url = new URL('./worker.js', import.meta.url).href; },
    importModuleDynamically: () => bridge });
  await worker.link(() => {}); await worker.evaluate();
  let request = 0;
  async function send(message) {
    const response = new Promise(resolve => { next = resolve; });
    surface.onmessage({ data: { ...message, request: ++request } });
    await response; return replies.at(-1);
  }
  assert.equal((await send({ op: 'init', namespace: 'owned' })).error, undefined);
  return send;
}

test('S13 worker fallback never serializes raw foreign errors or fixture context', async () => {
  const send = await workerWithFailure(new Error(canary));
  const response = await send({ op: 'open', name: 'owned.sqlite' });
  assert.deepEqual(response.error, { kind: 'BackendFailure' });
  assert.equal(JSON.stringify(response).includes(canary), false);
  assert.deepEqual((await send({ op: 'simulate_failure', stage: 'open' })).error, { kind: 'BackendFailure' });
});

test('S10 worker preserves only allowlisted typed kinds and strips every extra field', async () => {
  for (const kind of ['BackendFailure', 'MigrationFailure', 'SchemaConflict', 'InvalidSchema',
    'InvalidQuery', 'InvalidRow', 'TableNotFound', 'RowAlreadyExists', 'RowNotFound']) {
    const send = await workerWithFailure({ kind, message: canary, table: canary, id: canary,
      cause: { identity: canary }, extra: canary });
    const response = await send({ op: 'open', name: 'owned.sqlite' });
    assert.deepEqual(response.error, { kind });
    assert.equal(JSON.stringify(response).includes(canary), false);
  }
});

test('S13 foreign kind strings cannot turn arbitrary data into a public error kind', async () => {
  const send = await workerWithFailure({ kind: canary, message: canary });
  assert.deepEqual((await send({ op: 'open', name: 'owned.sqlite' })).error, { kind: 'BackendFailure' });
});

// Successful/failed secure opens share API-0008 ownership gates; these are RPC stubs.
const rpc = (send, op, owner = null, args = {}) => send({ op: 'command_rpc',
  input: JSON.stringify({ op, owner, args }) });
test('S10 successful secure RPC owner blocks shutdown until its explicit close', async () => {
  const send = await workerWithFailure();
  const opened = JSON.parse((await rpc(send, 'open_secure')).value);
  assert.deepEqual(opened, { ok: true, value: 'secure-owner' });
  assert.deepEqual((await send({ op: 'shutdown' })).error, { kind: 'BackendFailure' });
  assert.equal(JSON.parse((await rpc(send, 'close', opened.value)).value).ok, true);
  assert.equal((await send({ op: 'shutdown' })).value, null);
});
test('S10 failed secure RPC open never registers an owner or blocks shutdown', async () => {
  const send = await workerWithFailure();
  assert.equal(JSON.parse((await rpc(send, 'open_secure', null, { reject: true })).value).ok, false);
  assert.equal((await send({ op: 'shutdown' })).value, null);
});

// Routing only: the real keyed shared cases execute later in OPFS.
test('S06 keyed shared test control routes only with an exact key and no active owners', async () => {
  const send = await workerWithFailure();
  const args = {op:'security_conformance',group:'typed',index:0,key_hex:'03'.repeat(32)};
  assert.deepEqual((await send(args)).value,{group:'typed',index:0,encrypted:true});
  assert.deepEqual((await send({...args,key_hex:'bad'})).error,{kind:'BackendFailure'});
  const owner=JSON.parse((await rpc(send,'open_secure')).value).value;
  assert.deepEqual((await send(args)).error,{kind:'BackendFailure'});
  await rpc(send,'close',owner);
});
