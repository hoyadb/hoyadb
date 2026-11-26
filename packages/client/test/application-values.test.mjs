// API-0012 / TC-0046 APP01: pure application value translation. These tests exercise the
// translators only; real-core behavior is covered by the application workflow tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  table, column, relation, defineApplicationSchema, applicationSchema, isApplicationSchema,
  hasApplicationTables, resolveColumn, encodeValue, encodeFields, decodeRow, encodeJson,
  decodeJson, milliseconds, readonly
} from '../src/application-values.js';

const kind = expected => error => { assert.equal(error.kind, expected); return true; };
const MAX_SAFE = Number.MAX_SAFE_INTEGER;

const build = () => defineApplicationSchema({version: 1, tables: {
  customers: table({columns: {name: column.string(), country: column.string({optional: true})},
    relations: {orders: relation.hasMany('orders', {foreignKey: 'customer_id'})}}),
  orders: table({columns: {
      total: column.integer(), big: column.integer({bigint: true, optional: true}),
      ratio: column.float({optional: true}), paid: column.boolean({optional: true}),
      placed_at: column.timestamp(), raw_time: column.timestamp({bigint: true, optional: true}),
      note: column.string({optional: true}), ['__proto__']: column.string({optional: true})},
    relations: {customer: relation.toOne('customers', {foreignKey: 'customer_id'})}}),
  tags: table({columns: {label: column.string()}}),
  order_tags: table({columns: {}, relations: {
    order: relation.toOne('orders', {foreignKey: 'order_id'}),
    tag: relation.toOne('tags', {foreignKey: 'tag_id', optional: true})}})
}});
// Built per test, so a failing build is reported by each test rather than at import.
let built;
const fixture = () => (built ??= build());

test('APP01 builders produce exactly the existing wire DTO', () => {
  const schema = fixture();
  assert.deepEqual(JSON.parse(JSON.stringify(schema.tables.customers)), {
    columns: {name: {kind: 'String', optional: false}, country: {kind: 'String', optional: true}},
    relations: {orders: {kind: 'HasMany', table: 'orders', foreign_key: 'customer_id'}}});
  assert.equal(schema.version, '1');
  assert.deepEqual(schema.tables.order_tags.relations.tag,
    {kind: 'ToOne', table: 'tags', foreign_key: 'tag_id', optional: true});
  assert.deepEqual(relation.manyToMany('tags', {through: 'order_tags', sourceRelation: 'order', targetRelation: 'tag'}),
    {kind: 'ManyToMany', table: 'tags', through: 'order_tags', source_relation: 'order', target_relation: 'tag'});
  assert.deepEqual(schema.tables.tags.relations, {});
  // Host policy is metadata, never an extra wire key.
  assert.deepEqual(Object.keys(schema.tables.orders.columns.big), ['kind', 'optional']);
});

test('APP01 schemas are immutable and branded only by explicit construction', () => {
  const schema = fixture();
  assert.throws(() => { schema.tables.orders.columns.total.kind = 'String'; }, TypeError);
  assert.throws(() => { schema.version = '2'; }, TypeError);
  assert.equal(isApplicationSchema(schema), true);
  const wire = JSON.parse(JSON.stringify(schema));
  assert.equal(isApplicationSchema(wire), false, 'an equal raw DTO is not branded');
  assert.equal(isApplicationSchema({version: 1, tables: {}}), false, 'numeric version is not a brand');
  const converted = applicationSchema(wire);
  assert.equal(isApplicationSchema(converted), true);
  assert.deepEqual(resolveColumn(converted, 'orders', 'big'), {kind: 'Integer', optional: true, bigint: false});
  assert.equal(hasApplicationTables({version: 1, tables: {a: table({columns: {}})}}), true);
  assert.equal(hasApplicationTables(wire), false);
  assert.equal(hasApplicationTables({get tables() { throw new Error('accessor'); }}), false);
});

test('APP01 invalid declarations reject with InvalidSchema', () => {
  const schema = fixture();
  for (const version of [0, 1.5, 2 ** 32, '01', '', -1, MAX_SAFE + 1])
    assert.throws(() => defineApplicationSchema({version, tables: {}}), kind('InvalidSchema'), String(version));
  for (const bad of ['', 'id', 'a.b', '__hoya_x'])
    assert.throws(() => table({columns: {[bad]: column.string()}}), kind('InvalidSchema'), bad);
  assert.throws(() => column.string({bigint: true}), kind('InvalidSchema'));
  assert.throws(() => column.integer({unknown: true}), kind('InvalidSchema'));
  assert.throws(() => column.integer({optional: 'yes'}), kind('InvalidSchema'));
  assert.throws(() => relation.toOne('t', {foreign_key: 'x'}), kind('InvalidSchema'));
  assert.throws(() => table({columns: {}, extra: {}}), kind('InvalidSchema'));
  assert.throws(() => defineApplicationSchema({version: 1, tables: {a: table({columns: {x: column.string()},
    relations: {r: relation.toOne('a', {foreignKey: 'x'})}})}}), kind('InvalidSchema'), 'foreign key shadows a column');
});

test('APP01 resolveColumn covers columns, id, foreign keys and one to-one hop', () => {
  const schema = fixture();
  assert.deepEqual(resolveColumn(schema, 'orders', 'total'), {kind: 'Integer', optional: false, bigint: false});
  assert.deepEqual(resolveColumn(schema, 'orders', 'id'), {kind: 'String', optional: false, bigint: false});
  assert.deepEqual(resolveColumn(schema, 'orders', 'customer_id'), {kind: 'Reference', optional: false, bigint: false});
  assert.deepEqual(resolveColumn(schema, 'orders', 'customer.country'), {kind: 'String', optional: true, bigint: false});
  assert.deepEqual(resolveColumn(schema, 'order_tags', 'tag_id'), {kind: 'Reference', optional: true, bigint: false});
  for (const field of ['missing', 'customer.missing', 'customer.orders.total', 'nothing.name'])
    assert.throws(() => resolveColumn(schema, 'orders', field), kind('InvalidQuery'), field);
  assert.throws(() => resolveColumn(schema, 'customers', 'orders.total'), kind('InvalidQuery'), 'has-many is not a dotted path');
  assert.throws(() => resolveColumn(schema, 'nope', 'id'), kind('TableNotFound'));
  assert.throws(() => resolveColumn(JSON.parse(JSON.stringify(schema)), 'orders', 'id'), kind('InvalidSchema'));
});

test('APP01 integer columns keep a fixed Number or bigint policy at 2^53', () => {
  const schema = fixture();
  const plain = resolveColumn(schema, 'orders', 'total'), wide = resolveColumn(schema, 'orders', 'big');
  assert.deepEqual(encodeValue(plain, MAX_SAFE), {kind: 'integer', value: '9007199254740991'});
  assert.deepEqual(encodeValue(plain, '-9007199254740991'), {kind: 'integer', value: '-9007199254740991'});
  for (const value of [MAX_SAFE + 1, '9007199254740992', 1n, -0, 1.5, NaN, '01', undefined])
    assert.throws(() => encodeValue(plain, value), kind('InvalidRow'), String(value));
  assert.deepEqual(encodeValue(wide, 9223372036854775807n), {kind: 'integer', value: '9223372036854775807'});
  assert.deepEqual(encodeValue(wide, '-9223372036854775808'), {kind: 'integer', value: '-9223372036854775808'});
  for (const value of [1, 9223372036854775808n])
    assert.throws(() => encodeValue(wide, value), kind('InvalidRow'), String(value));
  const row = decodeRow(schema, 'orders', {id: 'o1', fields: {total: {kind: 'integer', value: '9007199254740991'},
    big: {kind: 'integer', value: '1'}, placed_at: {kind: 'timestamp', value: '0'}, customer_id: {kind: 'reference', value: 'c1'}}});
  assert.equal(row.total, MAX_SAFE);
  assert.equal(row.big, 1n, 'bigint columns always decode as bigint');
  assert.throws(() => decodeRow(schema, 'orders', {id: 'o1', fields: {total: {kind: 'integer', value: '9007199254740992'}}}),
    kind('InvalidPayload'), 'an unsafe value never silently rounds on read');
});

test('APP01 float, boolean, string and reference values', () => {
  const schema = fixture();
  const ratio = resolveColumn(schema, 'orders', 'ratio');
  assert.deepEqual(encodeValue(ratio, 0.1), {kind: 'float', value: '0.1'});
  assert.deepEqual(encodeValue(ratio, 2), {kind: 'float', value: '2'});
  for (const value of [-0, NaN, Infinity, '1.5', 1n])
    assert.throws(() => encodeValue(ratio, value), kind('InvalidRow'), String(value));
  assert.throws(() => encodeValue(resolveColumn(schema, 'orders', 'paid'), 1), kind('InvalidRow'));
  assert.throws(() => encodeValue(resolveColumn(schema, 'orders', 'customer_id'), ''), kind('InvalidRow'));
  assert.throws(() => encodeValue(resolveColumn(schema, 'customers', 'name'), '\ud800'), kind('InvalidPayload'));
  assert.deepEqual(encodeValue(resolveColumn(schema, 'orders', 'customer_id'), 'c1'), {kind: 'reference', value: 'c1'});
});

test('APP01 timestamps use a fresh Date or an explicit bigint policy', () => {
  const schema = fixture();
  const placed = resolveColumn(schema, 'orders', 'placed_at'), raw = resolveColumn(schema, 'orders', 'raw_time');
  const when = new Date('2026-09-15T12:00:00Z');
  assert.deepEqual(encodeValue(placed, when), {kind: 'timestamp', value: String(when.getTime())});
  assert.deepEqual(encodeValue(placed, -1), {kind: 'timestamp', value: '-1'});
  for(const value of ['8640000000000001','-8640000000000001','9223372036854775807'])
    assert.throws(()=>encodeValue(placed,value),kind('InvalidRow'),'Date-policy writes must remain decodable');
  assert.deepEqual(encodeValue(raw, -9223372036854775808n), {kind: 'timestamp', value: '-9223372036854775808'});
  assert.throws(() => encodeValue(placed, new Date(NaN)), kind('InvalidRow'));
  const wire = {id: 'o1', fields: {placed_at: {kind: 'timestamp', value: '8640000000000000'},
    raw_time: {kind: 'timestamp', value: '9223372036854775807'}}};
  const first = decodeRow(schema, 'orders', wire), second = decodeRow(schema, 'orders', wire);
  assert.equal(first.placed_at.getTime(), 8640000000000000);
  assert.notEqual(first.placed_at, second.placed_at, 'each decode returns a fresh Date');
  assert.equal(first.raw_time, 9223372036854775807n);
  assert.throws(() => decodeRow(schema, 'orders', {id: 'o1', fields: {placed_at: {kind: 'timestamp', value: '8640000000000001'}}}),
    kind('InvalidPayload'), 'an unrepresentable Date rejects');
  assert.equal(milliseconds(when), String(when.getTime()));
  assert.equal(milliseconds(12n), '12');
  assert.throws(() => milliseconds(1.5), kind('InvalidRequest'));
});

test('APP01 rows stay flat with exact null and absence', () => {
  const schema = fixture();
  const encoded = encodeFields(schema, 'orders', {total: 3, note: null, customer_id: 'c1', placed_at: 5});
  assert.deepEqual(encoded, {total: {kind: 'integer', value: '3'}, note: {kind: 'null'},
    customer_id: {kind: 'reference', value: 'c1'}, placed_at: {kind: 'timestamp', value: '5'}});
  assert.equal('ratio' in encoded, false, 'a missing optional field stays missing');
  const row = decodeRow(schema, 'orders', {id: 'o1', fields: encoded});
  assert.equal(row.id, 'o1');
  assert.equal(row.note, null);
  assert.equal(Object.prototype.hasOwnProperty.call(row, 'ratio'), false);
  assert.equal(row.customer_id, 'c1', 'references decode as strings');
  for (const fields of [{total: undefined}, {total: null}, {nope: 1}, {id: 'x'}])
    assert.throws(() => encodeFields(schema, 'orders', fields), kind('InvalidRow'), JSON.stringify(fields));
  assert.throws(() => encodeFields(schema, 'orders', {get total() { return 1; }}), kind('InvalidRow'), 'accessor');
  assert.throws(() => encodeFields(schema, 'orders', new (class { total = 1; })()), kind('InvalidRow'), 'class instance');
  assert.throws(() => decodeRow(schema, 'orders', {id: 'o1', fields: {total: {kind: 'string', value: '1'}}}), kind('InvalidPayload'));
});

test('APP01 __proto__ is an ordinary own field and never changes a prototype', () => {
  const schema = fixture();
  const input = JSON.parse('{"__proto__": "own text"}');
  const encoded = encodeFields(schema, 'orders', input);
  assert.deepEqual(Object.keys(encoded), ['__proto__']);
  assert.equal(Object.getPrototypeOf(encoded), Object.prototype);
  const row = decodeRow(schema, 'orders', {id: 'o1', fields: encoded});
  assert.equal(Object.getPrototypeOf(row), Object.prototype);
  assert.equal(Object.getOwnPropertyDescriptor(row, '__proto__').value, 'own text');
  const json = decodeJson(encodeJson(JSON.parse('{"__proto__": {"polluted": true}, "constructor": 1}')));
  assert.equal(Object.getPrototypeOf(json), Object.prototype);
  assert.equal({}.polluted, undefined);
  assert.equal(json.constructor, 1);
});

test('APP01 business JSON follows the fixed Number rules and never guesses tags', () => {
  const schema = fixture();
  assert.deepEqual(encodeJson({n: 2, f: 2.5, b: 9007199254740993n, s: 'x', t: true, z: null, a: [1]}), {kind: 'object', value: {
    n: {kind: 'integer', value: '2'}, f: {kind: 'float', value: '2.5'}, b: {kind: 'integer', value: '9007199254740993'},
    s: {kind: 'string', value: 'x'}, t: {kind: 'boolean', value: true}, z: {kind: 'null'},
    a: {kind: 'array', value: [{kind: 'integer', value: '1'}]}}});
  assert.deepEqual(encodeJson({kind: 'string', value: 'x'}), {kind: 'object', value: {
    kind: {kind: 'string', value: 'string'}, value: {kind: 'string', value: 'x'}}}, 'a tag-shaped object stays ordinary data');
  for (const value of [MAX_SAFE + 1, -0, NaN, Infinity, new Date(), undefined, () => 1, Symbol('x'), new Map(),
    {get a() { return 1; }}, Object.assign([1, 2], {extra: 1}), [, 1]])
    assert.throws(() => encodeJson(value), kind('InvalidPayload'), String(value?.constructor?.name ?? value));
  const cycle = {}; cycle.self = cycle;
  assert.throws(() => encodeJson(cycle), kind('InvalidPayload'));
  let deep = 1; for (let n = 0; n < 70; n++) deep = [deep];
  assert.throws(() => encodeJson(deep), kind('LimitExceeded'));
  assert.deepEqual(decodeJson({kind: 'array', value: [{kind: 'integer', value: '9007199254740991'},
    {kind: 'integer', value: '9007199254740992'}, {kind: 'float', value: '0.5'}]}), [MAX_SAFE, 9007199254740992n, 0.5]);
  for (const tagged of [{kind: 'timestamp', value: '1'}, {kind: 'reference', value: 'x'}, {kind: 'float', value: '1e999'}, {value: 1}])
    assert.throws(() => decodeJson(tagged), kind('InvalidPayload'), JSON.stringify(tagged));
});

test('APP04 readonly captured values throw on any nested mutation, also in sloppy mode', () => {
  const schema = fixture();
  const identity = readonly({user: {name: 'ada', roles: ['admin'], id: 7, wide: 9007199254740993n}});
  assert.equal(identity.user.id, 7);
  assert.equal(identity.user.wide, 9007199254740993n);
  const sloppy = new Function('target', 'target.user.name = "eve"');
  assert.throws(() => sloppy(identity), TypeError);
  assert.throws(() => { delete identity.user.roles; }, TypeError);
  assert.throws(() => identity.user.roles.push('x'), TypeError);
  assert.throws(() => Object.defineProperty(identity, 'extra', {value: 1}), TypeError);
  assert.throws(() => Object.setPrototypeOf(identity.user, null), TypeError);
  assert.equal(identity.user.name, 'ada');
  const cycle = {}; cycle.self = cycle;
  assert.throws(() => readonly(cycle), kind('InvalidPayload'));
  assert.throws(() => readonly({n: NaN}), kind('InvalidPayload'));
});

test('APP01 foreign keys mirror the core schema invariants (e6 regression)', () => {
  const users = table({columns: {name: column.string()}});
  const groups = table({columns: {name: column.string()}});
  const define = notes => defineApplicationSchema({version: 1, tables: {users, groups, notes: table(notes)}});
  let explicit, shared;
  assert.doesNotThrow(() => { explicit = define({columns: {author_id: column.reference({optional: true})},
    relations: {author: relation.toOne('users', {foreignKey: 'author_id', optional: true})}}); },
    'an explicit Reference column matching the to-one declaration is valid');
  assert.deepEqual(resolveColumn(explicit, 'notes', 'author_id'), {kind: 'Reference', optional: true, bigint: false});
  assert.deepEqual(resolveColumn(explicit, 'notes', 'author.name'), {kind: 'String', optional: false, bigint: false});
  assert.doesNotThrow(() => { shared = define({columns: {}, relations: {
    author: relation.toOne('users', {foreignKey: 'person_id'}),
    editor: relation.toOne('users', {foreignKey: 'person_id'})}}); },
    'to-one aliases may share a foreign key with the same target and optionality');
  assert.deepEqual(resolveColumn(shared, 'notes', 'person_id'), {kind: 'Reference', optional: false, bigint: false});
  assert.deepEqual(resolveColumn(shared, 'notes', 'editor.name'), {kind: 'String', optional: false, bigint: false});
  for (const [label, notes] of [
    ['explicit column of the wrong kind', {columns: {author_id: column.string()},
      relations: {author: relation.toOne('users', {foreignKey: 'author_id'})}}],
    ['explicit column with different optionality', {columns: {author_id: column.reference()},
      relations: {author: relation.toOne('users', {foreignKey: 'author_id', optional: true})}}],
    ['shared alias with different optionality', {columns: {}, relations: {
      author: relation.toOne('users', {foreignKey: 'person_id'}),
      editor: relation.toOne('users', {foreignKey: 'person_id', optional: true})}}],
    ['shared alias with a different target', {columns: {}, relations: {
      author: relation.toOne('users', {foreignKey: 'person_id'}),
      group: relation.toOne('groups', {foreignKey: 'person_id'})}}],
    ['relation name shadowing a column', {columns: {author: column.string()},
      relations: {author: relation.toOne('users', {foreignKey: 'author_id'})}}],
    ['relation name shadowing an inferred foreign key', {columns: {}, relations: {
      author: relation.toOne('users', {foreignKey: 'editor'}),
      editor: relation.toOne('users', {foreignKey: 'person_id'})}}]])
    assert.throws(() => define(notes), kind('InvalidSchema'), label);
});
