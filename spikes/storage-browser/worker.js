// SPEC-0023: a test-only serialized dispatcher. No Rust handle crosses messages.
// Fresh URLs prevent an earlier generated module from surviving a test rebuild.
const assetKey = crypto.randomUUID();
let load, initialize, shutdown, BrowserStore, TypedRow, Batch, fixture, conformance, typed_conformance, typed_physical, typed_index_evidence, model_conformance, model_physical, model_fixture_write, model_fixture_read, ReactiveStore, reactive_conformance, review_large_sets, review_conformance, CommandFixture, command_conformance, entity_conformance, security_conformance;
let initialized = false, retired = false, store = null, handle = null, reactive = null, reactiveHandle = null, command = null;
const commandOwners = new Set();
const failure = message => ({ kind: 'BackendFailure', message });
// API-0010: rebuild an allowlisted error, never forward foreign payload fields.
const errorKinds = new Set(['BackendFailure', 'MigrationFailure', 'SchemaConflict', 'InvalidSchema', 'InvalidQuery', 'InvalidRow', 'TableNotFound', 'RowAlreadyExists', 'RowNotFound']);
function publicError(error) {
  let kind;
  try { kind = error?.kind; } catch { /* A foreign getter is not a public error. */ }
  return { kind: errorKinds.has(kind) ? kind : 'BackendFailure' };
}
function require(condition, message) { if (!condition) throw failure(message); }
function text(value) { require(typeof value === 'string', 'string required'); return value; }
function name(value) {
  text(value);
  require(value !== '' && value !== ':memory:' && !value.startsWith('file:') && !value.includes('/') && !value.includes('\0'), 'literal fixture database name required');
  return value;
}
function typedRow(input) {
  require(input && typeof input === 'object' && Array.isArray(input.fields), 'typed row required');
  const result = new TypedRow(text(input.id));
  try {
    for (const field of input.fields) {
      require(field && typeof field === 'object', 'field required'); text(field.name);
      switch (field.kind) {
        case 'String': result.string(field.name, text(field.value)); break;
        case 'Integer':
          require(typeof field.value === 'string' && /^-?(0|[1-9][0-9]*)$/.test(field.value), 'decimal i64 string required');
          result.integer(field.name, field.value); break;
        case 'Float':
          require(typeof field.value === 'number' && Number.isFinite(field.value), 'finite float required');
          result.float(field.name, field.value); break;
        case 'Timestamp':
          require(typeof field.value === 'string' && /^-?(0|[1-9][0-9]*)$/.test(field.value), 'decimal i64 string required');
          result.timestamp(field.name, field.value); break;
        case 'Reference': result.reference(field.name, text(field.value)); break;
        case 'Null': require(field.value === null, 'null required'); result.null(field.name); break;
        case 'Boolean':
          require(typeof field.value === 'boolean', 'boolean required'); result.boolean(field.name, field.value); break;
        default: throw failure('unknown field kind');
      }
    }
    return result;
  } catch (error) { result.free(); throw error; }
}
function batch(input) {
  require(Array.isArray(input), 'mutations required');
  const result = new Batch();
  try {
    for (const mutation of input) {
      require(mutation && typeof mutation === 'object', 'mutation required');
      text(mutation.table);
      if (mutation.op === 'remove') result.remove(mutation.table, text(mutation.id));
      else {
        require(mutation.op === 'insert' || mutation.op === 'replace', 'unknown mutation');
        const row = typedRow(mutation.row);
        try { result[mutation.op](mutation.table, row); } finally { row.free(); }
      }
    }
    return result;
  } catch (error) { result.free(); throw error; }
}
async function dispatch(message) {
  require(message && typeof message === 'object' && Number.isSafeInteger(message.request), 'invalid request');
  const { op } = message;
  if (op === 'simulate_failure') {
    require(['init', 'open', 'write'].includes(message.stage), 'invalid simulation');
    if (message.stage !== 'write') require(!store, 'simulation requires no open adapter');
    throw failure(`Simulated ${message.stage} failure before invocation; no real VFS failure claimed`);
  }
  require(!retired, 'worker has shut down');
  if (op === 'init') {
    require(!initialized, 'worker already initialized');
    require(isSecureContext && typeof WebAssembly === 'object' && typeof BigInt === 'function'
      && typeof navigator.storage?.getDirectory === 'function'
      && typeof FileSystemFileHandle?.prototype.createSyncAccessHandle === 'function', 'unsupported: required OPFS capabilities unavailable');
    // Load inside the serialized request: onmessage is installed before any await.
    ({ default: load, initialize, shutdown, BrowserStore, TypedRow, Batch, fixture, conformance, typed_conformance, typed_physical, typed_index_evidence, model_conformance, model_physical, model_fixture_write, model_fixture_read, ReactiveStore, reactive_conformance, review_large_sets, review_conformance, CommandFixture, command_conformance, entity_conformance, security_conformance } =
      await import(`./pkg/hoyadb_storage_browser_spike.js?run=${assetKey}`));
    await load({ module_or_path: new URL(`./pkg/hoyadb_storage_browser_spike_bg.wasm?run=${assetKey}`, import.meta.url) });
    await initialize(text(message.namespace)); initialized = true;
    return { secure: isSecureContext, dedicatedWorker: true, wasm: true, bigint: true, opfs: true, syncAccessHandle: true, crossOriginIsolated };
  }
  require(initialized, 'worker not initialized');
  if (op === 'shutdown') {
    require(!store && !reactive && commandOwners.size === 0, 'dispose adapter before VFS shutdown');
    if (command) { command.free(); command = null; }
    shutdown(); initialized = false; retired = true; return null;
  }
  if (op === 'security_conformance') {
    require(!store && !reactive && commandOwners.size === 0, 'conformance requires no active adapter');
    require(['typed','model','reactive','review','command','entity'].includes(message.group), 'security group required');
    require(Number.isInteger(message.index) && message.index >= 0, 'security index required');
    require(typeof message.key_hex === 'string' && /^[0-9a-f]{64}$/.test(message.key_hex), 'exact synthetic key required');
    return security_conformance(message.group, message.index, message.key_hex);
  }
  if (op === 'command_conformance') {
    require(!store && !reactive && commandOwners.size === 0, 'conformance requires no active adapter');
    require(Number.isInteger(message.index) && message.index >= 0 && message.index < 14, 'command case index required');
    return command_conformance(message.index);
  }
  if (op === 'entity_conformance') {
    require(!store && !reactive && commandOwners.size === 0, 'entity conformance requires no active adapter');
    require(Number.isInteger(message.index) && message.index >= 0 && message.index < 13, 'entity case index required');
    return entity_conformance(message.index);
  }
  if (op === 'command_rpc') {
    require(!store && !reactive, 'raw/reactive and command fixtures are exclusive');
    command ||= new CommandFixture();
    const input = text(message.input);
    const reply = command.rpc(input);
    const response = JSON.parse(reply);
    // Rust has validated this complete outer tree has no numeric tokens before
    // any successful response. Domain JSON/envelope strings remain opaque.
    if (response.ok) {
      const frame = JSON.parse(input);
      if (frame.op === 'open' || frame.op === 'open_secure') commandOwners.add(response.value);
      else if (frame.op === 'close') commandOwners.delete(frame.owner);
    }
    return reply;
  }
  if (op === 'open') {
    require(!store && !reactive && commandOwners.size === 0, 'only one active connection supported');
    store = new BrowserStore(text(message.name)); handle = crypto.randomUUID();
    return { handle };
  }
  if (op === 'reactive_open') {
    require(!store && !reactive && commandOwners.size === 0, 'only one active connection supported');
    reactive = new ReactiveStore(name(message.name)); reactiveHandle = crypto.randomUUID();
    return { handle: reactiveHandle };
  }
  if (op === 'review_conformance') { require(!store && !reactive && commandOwners.size === 0, 'conformance requires no active adapter'); require(Number.isInteger(message.index) && message.index >= 0 && message.index < 5, 'review case index required'); return review_conformance(message.index); }
  if (op === 'review_large_sets') { require(!store && !reactive && commandOwners.size === 0, 'fixture requires no active adapter'); return review_large_sets(); }
  if (op === 'reactive_conformance') { require(!store && !reactive && commandOwners.size === 0, 'conformance requires no active adapter'); return reactive_conformance(); }
  if (op.startsWith('reactive_')) {
    require(reactive && message.handle === reactiveHandle, 'stale or disposed reactive handle');
    switch (op) {
      case 'reactive_close': reactive.free(); reactive = null; reactiveHandle = null; return null;
      case 'reactive_observe':
        require(['rows','count','columns'].includes(message.mode) && Array.isArray(message.columns) && message.columns.every(v => typeof v === 'string'), 'observation mode and columns required');
        return reactive.observe(message.mode, message.columns);
      case 'reactive_unsubscribe':
        require(typeof message.token === 'string' && /^[1-9][0-9]*$/.test(message.token), 'decimal subscription required');
        return reactive.unsubscribe(message.token);
      case 'reactive_fetch': return reactive.fetch();
      case 'reactive_insert': case 'reactive_replace': {
        text(message.table); const row = typedRow(message.row);
        try { return op === 'reactive_insert' ? reactive.insert(message.table, row) : reactive.replace(message.table, row); } finally { row.free(); }
      }
      case 'reactive_remove': return reactive.remove(text(message.table), text(message.id));
      case 'reactive_batch': {
        const mutations = batch(message.mutations);
        try { reactive.batch(mutations); return null; } finally { mutations.free(); }
      }
      case 'reactive_drain':
        require(typeof message.now === 'string' && /^(0|[1-9][0-9]*)$/.test(message.now), 'decimal monotonic milliseconds required');
        return reactive.drain(message.now);
      case 'reactive_refresh': reactive.refresh(); return null;
      case 'reactive_fail_next_query': reactive.fail_next_query(); return null;
      case 'reactive_fail_next_project': reactive.fail_next_project(); return null;
      case 'reactive_stats': return reactive.stats();
      default: throw failure('unknown reactive operation');
    }
  }
  if (op === 'fixture' || op === 'inspect_fixture') {
    require(!store && !reactive && commandOwners.size === 0, 'fixture requires no active adapter');
    return fixture(name(message.name), text(message.kind), op === 'fixture');
  }
  if (op === 'typed_physical' || op === 'typed_index_evidence') { require(!store && !reactive && commandOwners.size === 0, 'fixture requires no active adapter'); return op === 'typed_physical' ? typed_physical() : typed_index_evidence(); }
  if (op === 'model_conformance' || op === 'model_physical' || op === 'model_fixture_write' || op === 'model_fixture_read') {
    require(!store && !reactive && commandOwners.size === 0, 'model fixture requires no active adapter');
    if (op === 'model_conformance') return model_conformance();
    if (op === 'model_physical') return model_physical();
    if (op === 'model_fixture_write') { model_fixture_write(); return null; }
    require(typeof message.advance === 'boolean', 'fixture advance flag required'); return model_fixture_read(message.advance);
  }
  if (op === 'typed_conformance') { require(!store && !reactive && commandOwners.size === 0, 'conformance requires no active adapter'); return typed_conformance(); }
  if (op === 'conformance') { require(!store && !reactive && commandOwners.size === 0, 'conformance requires no active adapter'); return conformance(); }
  require(store && message.handle === handle, 'stale or disposed handle');
  switch (op) {
    case 'typed_fixture_write': store.typed_fixture_write(); return null;
    case 'typed_fixture_read': return store.typed_fixture_read();
    case 'dispose': store.free(); store = null; handle = null; return null;
    case 'create_table': store.create_table(text(message.table)); return null;
    case 'get': return store.get(text(message.table), text(message.id));
    case 'remove': return store.remove(text(message.table), text(message.id));
    case 'insert': case 'replace': {
      text(message.table); const row = typedRow(message.row);
      try { store[op](message.table, row); return null; } finally { row.free(); }
    }
    case 'batch': {
      const mutations = batch(message.mutations);
      try { store.batch(mutations); return null; } finally { mutations.free(); }
    }
    default: throw failure('unknown operation');
  }
}
let serial = Promise.resolve();
self.onmessage = ({ data }) => {
  serial = serial.then(async () => {
    try { self.postMessage({ request: data?.request, value: await dispatch(data) }); }
    catch (error) {
      self.postMessage({ request: data?.request, error: publicError(error) });
    }
  });
};
