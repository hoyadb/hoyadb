// API-0012 / TC-0046 APP07: the plain-value fake backend used with the application binding.
// It correlates outcomes itself and validates every value; it implements no database logic.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createFakeTransport} from '../src/index.js';
import {j} from '../../client/src/index.js';

const kind = expected => error => { assert.equal(error.kind, expected); return true; };
const command = (key, args = {n: 1}) => ({command: 'add-v1', idempotencyKey: key, schemaVersion: 1,
  args, context: {identity: null, capturedAt: '2026-09-15T12:00:00.000Z'}});
const entity = writeId => ({table: 'notes', recordId: 'same', writeId, schemaVersion: 1,
  changes: {body: writeId}, context: {identity: null, capturedAt: '2026-09-15T12:00:00.000Z'}});

test('APP07 plain fake correlates commands and keeps wide integers exact', async () => {
  let received;
  const input = [command('k1', {min: -9223372036854775808n, ratio: 2.5, safe: 7})];
  const fake = createFakeTransport({handlers: {'add-v1': async envelope => {
    received = envelope; await Promise.resolve();
    return {status: 'applied', result: {id: 'server-1', max: 9223372036854775807n}};
  }}});
  assert.deepEqual(await fake.push(input),
    [{idempotencyKey: 'k1', status: 'applied', result: {id: 'server-1', max: 9223372036854775807n}}]);
  assert.deepEqual(received, input[0]);
  assert.notEqual(received, input[0], 'the handler receives its own copy');
  received.args.min = 0n;
  assert.deepEqual(fake.pushes, [input], 'recorded pushes are independent validated copies');
  assert.equal(input[0].args.min, -9223372036854775808n);
});

test('APP07 entity writes keep their exact tuples beside rejected commands', async () => {
  const fake = createFakeTransport({
    handlers: {'add-v1': () => ({status: 'rejected', reason: 'out of stock'})},
    entityHandlers: {notes: envelope => envelope.writeId === 'first'
      ? {status: 'acknowledged'} : {status: 'corrected', result: envelope.changes}}});
  assert.deepEqual(await fake.push([command('k1'), entity('first'), entity('second')]), [
    {idempotencyKey: 'k1', status: 'rejected', reason: 'out of stock'},
    {table: 'notes', recordId: 'same', writeId: 'first', status: 'acknowledged'},
    {table: 'notes', recordId: 'same', writeId: 'second', status: 'corrected', result: {body: 'second'}}]);
});

test('APP07 handlers are explicit own entries; missing handlers never invent outcomes', async () => {
  let calls = 0;
  const fake = createFakeTransport({handlers: {'add-v1': () => { calls++; return {status: 'applied', result: null}; }}});
  await assert.rejects(fake.push([{...command('k1'), command: 'constructor'}]), kind('InvalidCommand'));
  await assert.rejects(fake.push([{...command('k2'), command: 'toString'}]), kind('InvalidCommand'));
  await assert.rejects(fake.push([entity('w1')]), kind('InvalidCommand'), 'no default entity outcome');
  await assert.rejects(fake.push([command('k3'), {...command('k4'), command: 'missing-v1'}]), kind('InvalidCommand'));
  assert.equal(calls, 0, 'a push with any unknown handler calls none');
  const handlers = {};
  Object.defineProperty(handlers, '__proto__', {value: () => ({status: 'applied', result: 'own'}), enumerable: true});
  const own = createFakeTransport({handlers});
  assert.deepEqual(await own.push([{...command('k5'), command: '__proto__'}]),
    [{idempotencyKey: 'k5', status: 'applied', result: 'own'}]);
});

test('APP07 malformed outcomes and spoofed correlation are rejected', async () => {
  const push = outcome => createFakeTransport({handlers: {'add-v1': () => outcome}}).push([command('k1')]);
  for (const outcome of [null, 'applied', {}, {status: 'done'}, {status: 'applied'}, {status: 'rejected'},
    {status: 'rejected', reason: 1}, {status: 'acknowledged'}, {status: 'applied', result: 1, extra: 1},
    {status: 'applied', result: 1, idempotencyKey: 'spoof'}, {status: 'applied', result: 1, writeId: 'spoof'},
    {get status() { return 'applied'; }, result: 1}, new (class { status = 'applied'; result = 1; })()])
    await assert.rejects(push(outcome), kind('InvalidRequest'), JSON.stringify(outcome) ?? String(outcome));
  for (const result of [undefined, NaN, 2 ** 60, new Date(0), {a: () => 1}])
    await assert.rejects(push({status: 'applied', result}), kind('InvalidPayload'), String(result));
  const entityPush = outcome => createFakeTransport({entityHandlers: {notes: () => outcome}}).push([entity('w1')]);
  await assert.rejects(entityPush({status: 'acknowledged', table: 'other'}), kind('InvalidRequest'));
  await assert.rejects(entityPush({status: 'applied', result: 1, recordId: 'other'}), kind('InvalidRequest'));
});

test('APP07 malformed or duplicated envelopes reject before any handler runs', async () => {
  let calls = 0;
  const fake = createFakeTransport({handlers: {'add-v1': () => { calls++; return {status: 'applied', result: null}; }},
    entityHandlers: {notes: () => { calls++; return {status: 'acknowledged'}; }}});
  for (const input of ['[]', {}, [null], [[]], [{...command('k1'), idempotencyKey: 1}], [{command: 'add-v1'}],
    [{...entity('w1'), table: undefined}], [{...entity('w1'), recordId: 7}], [command('k1'), command('k1')],
    [entity('w1'), entity('w1')], [command('k1', {n: 2 ** 60})], [command('k1', {n: NaN})]])
    await assert.rejects(fake.push(input), kind('InvalidPayload'), JSON.stringify(input) ?? String(input));
  assert.equal(calls, 0);
  assert.deepEqual(fake.pushes, [], 'rejected pushes are not recorded');
  assert.deepEqual(await createFakeTransport().push([]), []);
});

test('APP07 hostile keys stay own data and never pollute prototypes', async () => {
  let received;
  const args = JSON.parse('{"__proto__": {"polluted": true}, "constructor": "own"}');
  const fake = createFakeTransport({handlers: {'add-v1': envelope => {
    received = envelope; return {status: 'applied', result: JSON.parse('{"__proto__": "kept"}')};
  }}});
  const outcomes = await fake.push([command('k1', args)]);
  assert.equal(outcomes.length, 1, 'one correlated outcome');
  assert.ok(received, 'the handler received the envelope');
  const [outcome] = outcomes;
  assert.equal(Object.getPrototypeOf(received.args), Object.prototype);
  assert.deepEqual(Object.keys(received.args), ['__proto__', 'constructor']);
  assert.equal({}.polluted, undefined);
  assert.equal(Object.getOwnPropertyDescriptor(outcome.result, '__proto__').value, 'kept');
});

test('APP07 pull is passed through as a validated copy', async () => {
  const page = {checkpoint: 'c1', complete: true, changes: [{table: 'notes', record_id: 'n1', big: 9223372036854775807n}]};
  const fake = createFakeTransport({pull: async (since, scope) => {
    assert.deepEqual([since, scope], [null, 'app']); return page;
  }});
  const pulled = await fake.pull(null, 'app');
  assert.deepEqual(pulled, page);
  assert.notEqual(pulled, page);
  await assert.rejects(createFakeTransport().pull(null, 'app'), kind('UnsupportedOperation'));
  await assert.rejects(createFakeTransport({pull: () => ({bad: NaN})}).pull(null, 'app'), kind('InvalidPayload'));
});

test('APP07 a supplied runtime keeps the existing tagged wire branch', async () => {
  let encoded;
  const runtime = {
    decodeJson: async body => { assert.equal(body, 'opaque body'); return j.array([j.object({
      command: j.string('add-v1'), idempotencyKey: j.string('k1'), args: j.object({})})]); },
    encodeJson: async value => { encoded = value; return 'canonical response'; }};
  const fake = createFakeTransport({runtime, handlers: {'add-v1': () => ({status: 'applied', result: j.null()})}});
  assert.equal(await fake.push('opaque body'), 'canonical response');
  assert.deepEqual(encoded, j.array([j.object({idempotencyKey: j.string('k1'),
    status: j.string('applied'), result: j.null()})]));
});
