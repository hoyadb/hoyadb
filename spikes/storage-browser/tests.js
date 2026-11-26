import { runReactiveChecks, runReviewLargeChecks, runReviewFailureChecks, runReviewSharedChecks } from './reactive-tests.js';
import { bridgePrimitives, primitives, typedExpected } from './typed-tests.js';
// TC-0023 W01–W08. Synthetic data only; reload checks read before insertion.
const output = document.querySelector('#results');
export const results = [];
window.hoyaPersistence = { results, checkedErrors: [], done: false, needsReload: false };
const report = (name, detail) => {
  results.push({ name, passed: true, detail });
  output.textContent = JSON.stringify(window.hoyaPersistence, null, 2);
};
function equal(actual, expected) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`Expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}
const field = (name, kind, value) => ({ name, kind, value });
const row = (id, fields = []) => ({ id, fields });
const original = row('é\0🌿', [field('', 'String', ''), field('max', 'Integer', '9223372036854775807'), field('min', 'Integer', '-9223372036854775808'), field('no', 'Boolean', false), field('text\0', 'String', "保留\0🌿 '); DROP TABLE x;--"), field('yes', 'Boolean', true)]);
const replaced = row('replace', [field('yes', 'Boolean', true)]);
const keeper = row('keeper', [field('k', 'String', 'unchanged')]);
const table = "\0🌿'; DROP TABLE x;--";
const key = 'hoyadb-spec23-run';
let saved = sessionStorage.getItem(key);
const namespace = saved || `hoyadb-spec23-${crypto.randomUUID()}`;
window.hoyaPersistence.namespace = namespace;
let worker, handle, next = 0;
const pending = new Map();
function start() {
  worker = new Worker(`./worker.js?run=${crypto.randomUUID()}`, { type: 'module' });
  worker.onmessage = ({ data }) => {
    const callbacks = pending.get(data.request);
    if (!callbacks) return;
    pending.delete(data.request);
    clearTimeout(callbacks.timer);
    callbacks.resolve(data);
  };
  worker.onerror = (event) => {
    for (const { reject, timer } of pending.values()) {
      clearTimeout(timer); reject(new Error(`Worker failed: ${event.message}`));
    }
    pending.clear();
  };
}
function request(message) {
  const request = ++next;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending.delete(request); reject(new Error('Worker request timed out')); }, 20000);
    pending.set(request, { resolve, reject, timer });
    worker.postMessage({ ...message, request });
  });
}
async function ok(message) {
  const response = await request({ handle, ...message });
  if (response.error) throw new Error(JSON.stringify(response.error));
  return response.value;
}
async function error(message, expected) {
  const response = await request({ handle, ...message });
  equal(response.error, expected);
  equal(Object.keys(response.error), ['kind']);
  for (const key of ['table', 'id', 'message', 'cause', 'extra']) equal(Object.hasOwn(response.error, key), false);
  window.hoyaPersistence.checkedErrors.push(response.error);
}
const backend = async message => {
  const response = await request({ handle, ...message });
  equal(response.error?.kind, 'BackendFailure');
  window.hoyaPersistence.checkedErrors.push(response.error);
  return response.error;
};
async function open(name = 'fixture.sqlite') {
  const opened = await ok({ op: 'open', name }); handle = opened.handle;
}
async function restart() {
  worker.terminate(); start();
  await ok({ op: 'init', namespace }); await open();
}
async function snapshot() {
  const rows = [];
  for (const [tableName, id] of [[table, original.id], [table, 'replace'], [table, 'removed'], [table, 'prefix'], ['', 'keeper'], ['', 'later'], ['', 'empty']]) {
    rows.push([tableName, id, await ok({ op: 'get', table: tableName, id })]);
  }
  return rows;
}
const expected = [[table, original.id, original], [table, 'replace', replaced], [table, 'removed', null], [table, 'prefix', null], ['', 'keeper', keeper], ['', 'later', null], ['', 'empty', row('empty')]];
async function run() {
  start();
  report('runtime', { userAgent: navigator.userAgent, origin: location.origin, secure: isSecureContext, crossOriginIsolated });
  // These are dispatcher simulations, not VFS denial/quota/commit-failure proof.
  await backend({ op: 'simulate_failure', stage: 'init' });
  await backend({ op: 'get', table, id: original.id });
  report('W05 simulated unavailable capability', 'BackendFailure; no adapter exposed');
  const capabilities = await ok({ op: 'init', namespace });
  report('W05 actual capabilities and initialization', capabilities);
  await backend({ op: 'simulate_failure', stage: 'open' });
  await backend({ op: 'get', table, id: original.id });
  await open();
  if (saved) {
    equal(await snapshot(), expected);
    report('W02/W03 page reload before insertion', await snapshot());
    equal(await ok({ op: 'get', table: '', id: primitives.id }), primitives);
    report('P11 primitive page reload before insertion', primitives);
    equal(await ok({ op: 'typed_fixture_read' }), typedExpected);
    report('P11 typed batch page reload before insertion', typedExpected);
    await ok({ op: 'dispose' });
    equal(await ok({ op: 'model_fixture_read', advance: false }), { rowIds: ['local:2'], nextId: null });
    report('D16 managed rows and schema page reload before insertion', { rowIds: ['local:2'], nextId: null });
    equal(await ok({ op: 'model_fixture_read', advance: true }), { rowIds: ['local:2'], nextId: 'local:3' });
    report('D16 persisted ID counter continuity', 'local:3 allocated only after exact restart reads');
    await open();
    // TC-0024 / DEF-0001: reject release while an adapter owns SQLite files.
    await backend({ op: 'shutdown' }); equal(await snapshot(), expected);
    report('L01 live adapter prevents shutdown', await snapshot());
    await ok({ op: 'dispose' });
    const root = await navigator.storage.getDirectory();
    let blocked = false;
    try { await root.removeEntry(namespace, { recursive: true }); }
    catch (error) { equal(error.name, 'NoModificationAllowedError'); blocked = true; }
    equal(blocked, true);
    report('L02 pool owns handles after adapter disposal', 'Actual NoModificationAllowedError while worker is still alive');
    await ok({ op: 'shutdown' });
    await backend({ op: 'get', table, id: original.id });
    await backend({ op: 'init', namespace });
    // Success here must precede worker termination: no termination-release race.
    await root.removeEntry(namespace, { recursive: true });
    report('L02 acknowledged shutdown permits cleanup before termination', namespace);
    worker.terminate(); sessionStorage.removeItem(key);
    report('cleanup', namespace);
    window.hoyaPersistence.done = true;
    output.textContent = JSON.stringify(window.hoyaPersistence, null, 2);
    return;
  }
  await ok({ op: 'create_table', table });
  await ok({ op: 'create_table', table: '' });
  for (const [t, r] of [[table, original], [table, row('replace', [field('omitted', 'String', 'old')])], [table, row('removed')], ['', keeper], ['', row('empty')]]) {
    await ok({ op: 'insert', table: t, row: r });
  }
  await ok({ op: 'batch', mutations: [{ op: 'replace', table, row: replaced }, { op: 'remove', table, id: 'removed' }] });
  equal(await snapshot(), expected);
  await error({ op: 'batch', mutations: [{ op: 'insert', table, row: row('prefix') }, { op: 'replace', table, row: row(original.id) }, { op: 'remove', table: '', id: 'keeper' }, { op: 'insert', table, row: original }, { op: 'insert', table: '', row: row('later') }] }, { kind: 'RowAlreadyExists' });
  equal(await snapshot(), expected);
  report('W03 immediate failed batch rollback', await snapshot());
  await restart(); equal(await snapshot(), expected);
  report('W01/W03 worker termination and recreation', await snapshot());
  const snapshotCopy = await ok({ op: 'get', table, id: original.id }); snapshotCopy.fields.length = 0;
  equal(await ok({ op: 'get', table, id: original.id }), original);
  for (const bad of [
    { op: 'unknown' }, { op: 'get', handle: handle + 1, table, id: original.id },
    { op: 'insert', table, row: row('bad', [field('x', 'Integer', '9223372036854775808')]) },
    { op: 'insert', table, row: row('bad', [field('x', 'Integer', '-9223372036854775809')]) },
    { op: 'insert', table, row: row('bad', [field('x', 'Integer', 42)]) },
    { op: 'insert', table, row: row('bad', [field('x', 'Boolean', 'false')]) },
    { op: 'insert', table, row: row('bad', [field('x', 'String', 1)]) },
    { op: 'insert', table, row: row('bad', [field('x', 'String', 'a'), field('x', 'String', 'b')]) },
    { op: 'batch', mutations: [{ op: 'insert', table, row: row('bad-prefix') }, { op: 'unknown' }] },
    { op: 'insert', table: 1, row: row('bad') }, { op: 'insert', table, row: null },
  ]) await backend(bad);
  equal(await ok({ op: 'get', table, id: 'bad' }), null);
  equal(await ok({ op: 'get', table, id: 'bad-prefix' }), null);
  equal(await snapshot(), expected);
  const old = handle; await ok({ op: 'dispose' });
  await backend({ op: 'get', handle: old, table, id: original.id });
  await open(); await restart(); equal(await snapshot(), expected);
  equal(await ok({ op: 'get', table, id: 'bad' }), null);
  equal(await ok({ op: 'get', table, id: 'bad-prefix' }), null);
  report('W06/W08 malformed input, stale handles, disposal and recreation', await snapshot());
  await backend({ op: 'simulate_failure', stage: 'write' }); equal(await snapshot(), expected);
  report('W07 simulated write failure', 'Synthetic BackendFailure before invocation; no actual VFS failure claimed');
  await ok({ op: 'dispose' });
  for (const name of ['', ':memory:', 'file::memory:', 'file:x?mode=memory']) await backend({ op: 'open', name });
  for (const kind of ['unrelated', 'version']) {
    const name = `${kind}.sqlite`;
    const before = await ok({ op: 'fixture', name, kind });
    await backend({ op: 'open', name });
    equal(await ok({ op: 'inspect_fixture', name, kind }), before);
    report(`W07 preserves incompatible ${kind} fixture`, before);
  }
  const mapping = await ok({ op: 'conformance' });
  report('W04 shared assertions', mapping);
  equal(await ok({ op: 'typed_conformance' }), ['P01', 'P02', 'P03', 'P04', 'P05', 'P06', 'P07', 'P08', 'P09', 'P10']);
  report('P01–P10 shared typed Rust assertions', 'All ten shared cases executed in the real SQLite worker');
  report('P11 physical fixtures and actual SQL rollback', await ok({ op: 'typed_physical' }));
  report('P12 browser engine index costs', await ok({ op: 'typed_index_evidence' }));
  equal(await ok({ op: 'model_conformance' }), ['S01','S02','S03','S04','S05','D01','D02','D03','D04','D05','D06','D07','D08','D09','D10','D11','D12','D13','D14','TC0028']);
  report('D01–D14 shared public model and schema assertions', 'All shared Phase 2 Rust cases executed in the real worker');
  equal(await ok({ op: 'model_physical' }), ['D15 initialization rollback','D15 adoption rollback','D15 public row and counter rollback','D15 migration rollback','D15 reset rollback']);
  report('D15 actual SQL model metadata abort and recovery', 'Initialization, adoption, insertion, migration and reset preserve original state');
  await runReactiveChecks({ request, report });
  await runReviewLargeChecks({ request, report });
  await runReviewSharedChecks({ request, report });
  await runReviewFailureChecks({ request, report });
  await ok({ op: 'model_fixture_write' });
  await open(); equal(await snapshot(), expected);
  await bridgePrimitives({ ok, backend, equal, report });
  await ok({ op: 'typed_fixture_write' });
  equal(await ok({ op: 'typed_fixture_read' }), typedExpected);
  await restart();
  equal(await ok({ op: 'typed_fixture_read' }), typedExpected);
  equal(await ok({ op: 'get', table: '', id: primitives.id }), primitives);
  report('P11 typed batch and new primitive worker restart', typedExpected);
  await ok({ op: 'dispose' });
  equal(await ok({ op: 'model_fixture_read', advance: false }), { rowIds: ['local:2'], nextId: null });
  report('D16 managed schema and relations after worker restart', { rowIds: ['local:2'], nextId: null });
  await open();
  await ok({ op: 'insert', table, row: row('lifecycle') });
  await ok({ op: 'batch', mutations: [{ op: 'remove', table, id: 'lifecycle' }] });
  equal(await snapshot(), expected);
  await ok({ op: 'dispose' }); worker.terminate();
  sessionStorage.setItem(key, namespace);
  window.hoyaPersistence.needsReload = true;
  report('W08 lifecycle CRUD and batch', 'Disposed and terminated; ready for actual page reload');
}
run().catch(error => {
  worker?.terminate(); results.push({ name: 'failure', passed: false, detail: String(error) });
  window.hoyaPersistence.done = true;
  output.textContent = JSON.stringify(window.hoyaPersistence, null, 2);
});
