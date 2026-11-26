// SPEC-0025 / TC-0025 P01/P11: test-only bridge payload and restart assertions.
const field = (name, kind, value) => ({ name, kind, value });
export const primitives = { id: 'typed-primitives', fields: [
  field('float', 'Float', 1.25), field('max', 'Float', Number.MAX_VALUE),
  field('min', 'Float', -Number.MAX_VALUE), field('subnormal', 'Float', Number.MIN_VALUE),
  field('negativeZero', 'Float', 0), field('null', 'Null', null),
  field('reference', 'Reference', "引用\0'; DROP TABLE x;--"),
  field('timestamp', 'Timestamp', '-9223372036854775808'),
  field('timestampMax', 'Timestamp', '9223372036854775807'),
].sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0) };
export async function bridgePrimitives({ ok, backend, equal, report }) {
  const input = structuredClone(primitives);
  input.fields.find(field => field.name === 'negativeZero').value = -0;
  await ok({ op: 'insert', table: '', row: input });
  equal(await ok({ op: 'get', table: '', id: primitives.id }), primitives);
  equal(Object.is((await ok({ op: 'get', table: '', id: primitives.id })).fields.find(field => field.name === 'negativeZero').value, -0), false);
  for (const [kind, value] of [['Float', NaN], ['Float', Infinity], ['Float', -Infinity], ['Float', '1.25'], ['Timestamp', 1], ['Timestamp', '9223372036854775808'], ['Reference', 1], ['Null', false]]) {
    await backend({ op: 'batch', mutations: [{ op: 'insert', table: '', row: { id: 'typed-bad-prefix', fields: [] } }, { op: 'insert', table: '', row: { id: 'typed-bad', fields: [field('x', kind, value)] } }] });
    equal(await ok({ op: 'get', table: '', id: 'typed-bad-prefix' }), null);
    equal(await ok({ op: 'get', table: '', id: 'typed-bad' }), null);
  }
  report('P01 bridge finite float, i64 timestamp, reference and Null validation', primitives);
}

export const typedExpected = { id: 'typed-owned', fields: [
  field('float', 'Float', Number.MAX_VALUE), field('integer', 'Integer', '-9223372036854775808'),
  field('null', 'Null', null), field('reference', 'Reference', "引用\0'; DROP TABLE x;--"),
  field('timestamp', 'Timestamp', '9223372036854775807'),
] };
