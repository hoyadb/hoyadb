// API-0010 / TC-0035 S13: harness diagnostics use static kind and local labels.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';
import { runReactiveChecks, runReviewLargeChecks, runReviewSharedChecks } from './reactive-tests.js';

const cases = [
  ['command-page.js', 'hoyaCommands', ['runCommandChecks', 'runCommandPagingChecks', 'restartChecks']],
  ['entity-page.js', 'hoyaEntities', ['runEntityChecks', 'entityRestart']],
  ['reactive-page.js', 'hoyaReactive', ['runReactiveChecks', 'runReviewLargeChecks', 'runReviewFailureChecks', 'runReviewSharedChecks']],
];
async function runPage(file, stateName, exports, browserEvent = false) {
  const window = {};
  class Worker {
    postMessage(message) {
      queueMicrotask(() => {
        if (browserEvent && message.op === 'init') this.onerror({ message: 'synthetic-foreign-path-key-canary' });
        else this.onmessage({ data: { request: message.request,
          ...(message.op === 'init' ? { error: { kind: 'InvalidQuery' } } : { value: null }) } });
      });
    }
    terminate() {}
  }
  const context = vm.createContext({ window, Worker, URL, setTimeout, clearTimeout,
    crypto: { randomUUID: () => 'fixture' }, performance: { now: () => 0 },
    document: { querySelector: () => ({ textContent: '' }) },
    location: new URL('http://127.0.0.1/owned.html'),
    navigator: { userAgent: 'node-vm-fixture', storage: { getDirectory: async () => ({ removeEntry: async () => {} }) } },
    sessionStorage: { getItem: () => null, setItem() {}, removeItem() {} } });
  const checks = new vm.SyntheticModule(exports, function () {
    for (const name of exports) this.setExport(name, async () => false);
  }, { context });
  const source = await readFile(new URL(`./${file}`, import.meta.url), 'utf8');
  const module = new vm.SourceTextModule(source, { context });
  await module.link(async name => {
    if (name.endsWith('harness-error.js')) return new vm.SourceTextModule(
      await readFile(new URL('./harness-error.js', import.meta.url), 'utf8'), { context });
    return checks;
  });
  await module.evaluate();
  return window[stateName];
}
for (const [file, name, exports] of cases) {
  test(`S13 ${file} describes sanitized RPC failure using local operation and static kind`, async () => {
    const state = await runPage(file, name, exports);
    const failure = state.results.find(row => row.passed === false);
    assert.equal(failure.detail, `Error: ${file.replace('.js', '')}/init: InvalidQuery`);
    assert.equal(state.done, true);
    assert.equal(state.cleaned, true);
  });
  test(`S13 ${file} never copies a raw browser worker-event message`, async () => {
    const state = await runPage(file, name, exports, true);
    const failure = state.results.find(row => row.passed === false);
    assert.equal(failure.detail, `Error: ${file.replace('.js', '')}/worker event: BackendFailure`);
    assert.equal(JSON.stringify(state).includes('synthetic-foreign-path-key-canary'), false);
  });
}
for (const [run, label] of [
  [runReactiveChecks, 'reactive checks/reactive_conformance'],
  [runReviewLargeChecks, 'Q01/review_large_sets'],
  [runReviewSharedChecks, 'Q01 large observations/navigation/reverse dependencies'],
]) {
  test(`S13 ${label} preserves a sanitized failure's kind and harness-local case`, async () => {
    await assert.rejects(run({ request: async () => ({ error: { kind: 'InvalidQuery' } }), report() {} }),
      error => error.kind === 'InvalidQuery' && error.message === `${label}: InvalidQuery`);
  });
}
