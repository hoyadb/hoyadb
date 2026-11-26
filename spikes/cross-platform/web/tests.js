// SPEC-0017 B01–B05. Synthetic fixtures; no network except local module loading.
import init, { WebStore } from './pkg/hoyadb_browser_spike.js';
const min = -(1n << 63n), max = (1n << 63n) - 1n;
const output = document.querySelector('#results');
const results = [];
const actualRows = [];
const stringify = value => JSON.stringify(value, (_, v) => typeof v === 'bigint' ? `${v}n` : v, 2);
function assert(ok, message) { if (!ok) throw new Error(message); }
function insert(store, id, status, total, paid) {
  if (typeof total !== 'bigint' || total < min || total > max)
    throw new RangeError('total must be an i64 BigInt');
  store.insert(id, status, total.toString(), paid);
}
function rows(store, status) {
  const snapshot = store.query(status);
  try {
    return Array.from({ length: snapshot.len() }, (_, index) => {
      const row = snapshot.row(index);
      try { return { id: row.id, status: row.status, total: row.total_cents, paid: row.paid }; }
      finally { row.free(); }
    }).sort((a,b) => a.id.localeCompare(b.id));
  } finally { snapshot.free(); }
}
function equal(actual, expected) {
  assert(stringify(actual) === stringify(expected), `rows differ: ${stringify(actual)}`);
}
function drain(store, callback) {
  const events = store.drain();
  try {
    const copied = Array.from({ length: events.len() }, (_, i) => [events.token(i), events.count(i)]);
    for (const [token, count] of copied) callback(token, count);
  } finally { events.free(); }
}
function rejects(action) {
  let rejected = false;
  try { action(); } catch { rejected = true; }
  assert(rejected, 'expected rejection');
}
function run(name, fn) {
  try { fn(); results.push({ name, passed: true }); console.log(`PASS ${name}`); }
  catch (error) { results.push({ name, passed: false, error: String(error) }); console.error(`FAIL ${name}`, String(error)); }
}
await init();
run('B01 typed insert/query', () => {
  const s = new WebStore();
  try {
    insert(s, '注文🌿', 'pending', 1299n, true);
    insert(s, 'complete', 'complete', -77n, false);
    equal(rows(s, 'pending'), [{ id:'注文🌿', status:'pending', total:1299n, paid:true }]);
    actualRows.push(...rows(s, 'pending'));
    console.log('B01 actual rows', stringify(rows(s, 'pending')));
  } finally { s.free(); }
});
run('B02 empty/nonmatching', () => {
  const a = new WebStore(), b = new WebStore();
  try {
    equal(rows(a, 'pending'), []);
    insert(b, 'done', 'complete', 5n, false);
    equal(rows(b, 'pending'), []);
  } finally { a.free(); b.free(); }
});
run('B03 isolation/disposal', () => {
  const a = new WebStore(), b = new WebStore();
  try {
    insert(a, 'a', 'pending', 3n, false);
    equal(rows(b, 'pending'), []);
    assert(rows(a, 'pending').length === 1, 'first store lost row');
  } finally { a.free(); b.free(); }
});
run('B04 integer boundaries/rejections', () => {
  const s = new WebStore();
  try {
    insert(s, 'a-min', 'pending', min, false);
    insert(s, 'b-max', 'pending', max, true);
    const expected = [{ id:'a-min', status:'pending', total:min, paid:false },
      { id:'b-max', status:'pending', total:max, paid:true }];
    equal(rows(s, 'pending'), expected);
    for (const invalid of [min - 1n, max + 1n, 9007199254740992, '1']) {
      rejects(() => insert(s, 'invalid', 'pending', invalid, false));
      equal(rows(s, 'pending'), expected);
    }
    // Bypass the JS wrapper too: Rust must validate before mutating.
    for (const invalid of [(min - 1n).toString(), (max + 1n).toString(), 'NaN']) {
      rejects(() => s.insert('invalid', 'pending', invalid, false));
      equal(rows(s, 'pending'), expected);
    }
  } finally { s.free(); }
});
run('B05 subscription drain/read/unsubscribe', () => {
  const s = new WebStore();
  try {
    const token = s.observe('pending');
    let initial = 0;
    drain(s, (t, count) => { assert(t === token && count === 0, 'bad initial event'); initial++; });
    assert(initial === 1, 'missing initial event');
    const start = performance.now();
    insert(s, 'live', 'pending', 42n, true);
    let delivered = 0;
    drain(s, (t, count) => {
      window.hoyaLatencyMs = performance.now() - start;
      assert(t === token && count === 1, 'bad membership event');
      equal(rows(s, 'pending'), [{ id:'live', status:'pending', total:42n, paid:true }]);
      delivered++;
    });
    assert(delivered === 1, 'missing membership event');
    console.log('B05 write-to-callback latency ms', window.hoyaLatencyMs);
    insert(s, 'irrelevant', 'complete', 0n, false);
    drain(s, () => { throw new Error('irrelevant write emitted'); });
    assert(s.unobserve(token), 'unsubscribe failed');
    insert(s, 'later', 'pending', 0n, false);
    drain(s, () => { throw new Error('disposed subscription emitted'); });
  } finally { s.free(); }
});
window.hoyaResults = results;
// Keep one sentinel alive until navigation to observe this memory-only path on reload.
window.hoyaReloadProbe = new WebStore();
window.hoyaReloadRowsBefore = rows(window.hoyaReloadProbe, 'reload');
insert(window.hoyaReloadProbe, 'reload-sentinel', 'reload', 1n, false);
window.addEventListener('pagehide', () => window.hoyaReloadProbe.free(), { once:true });
output.textContent = stringify({ results, actualRows, latencyMs:window.hoyaLatencyMs,
  reloadRowsBefore:window.hoyaReloadRowsBefore, runtime:navigator.userAgent });
console.log('Runtime', navigator.userAgent);
