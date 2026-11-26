// HoyaDB JavaScript SDK tour. Runs in Node against the real Rust core compiled to WebAssembly,
// using the retained-memory engine. Run from the repository root after building crates/hoyadb-web/pkg:
//   node examples/javascript.mjs
import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import init, {Facade} from '../crates/hoyadb-web/pkg/hoyadb_web.js';
import {createRuntime, defineSchema, j, openDatabase} from '../packages/client/src/index.js';

// docs:start runtime
// Load the WebAssembly core once and wrap it in a runtime.
await init({module_or_path: await readFile(new URL('../crates/hoyadb-web/pkg/hoyadb_web_bg.wasm', import.meta.url))});
const facade = new Facade();
const runtime = createRuntime({open: (input, key) => facade.open(input, key), call: input => facade.call(input)});
// docs:end runtime

// 2. Declare the schema. A ToOne relation names its foreign-key field; HasMany is its inverse.
const schema = defineSchema({version: '1', tables: {
  projects: {columns: {name: {kind: 'String', optional: false}},
    relations: {tasks: {kind: 'HasMany', table: 'tasks', foreign_key: 'project_id'}}},
  tasks: {columns: {
      title: {kind: 'String', optional: false},
      done: {kind: 'Boolean', optional: false},
      priority: {kind: 'Integer', optional: false},
      notes: {kind: 'String', optional: true}},
    relations: {project: {kind: 'ToOne', table: 'projects', foreign_key: 'project_id', optional: false}}},
  comments: {columns: {body: {kind: 'String', optional: false}}, relations: {}}
}});

// Deterministic host capabilities keep this tour reproducible. Real apps omit idGenerator,
// clock and jitter, and pass a random 32-hex namespace only for tests.
let now = 1_700_000_000_000;
const ids = ['project-1', 'task-1', 'task-2', 'local:task-3', 'comment-1'];
const store = await runtime.createMemoryStore();
const options = {
  runtime, schema,
  engine: {kind: 'memory', store},
  namespace: '0123456789abcdef0123456789abcdef',
  idGenerator: () => ids.shift(),
  clock: () => String(now),
  jitter: () => 0,
  context: () => j.object({user: j.string('ada')})
};
const db = await openDatabase(options);

// Wait for a condition driven by host-turn delivery, with a bound.
const until = async (predicate, label) => {
  for (let turn = 0; turn < 1000; turn++) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 1));
  }
  throw new Error(`timed out waiting for ${label}`);
};

try {
  // 3. CRUD. Values are tagged cells; integers and timestamps travel as decimal strings.
  const project = await db.insert('projects', {name: j.string('Launch')});
  await db.insert('tasks', {title: j.string('Write docs'), done: j.boolean(false),
    priority: j.integer(2), project_id: j.reference(project.id)});
  const review = await db.insert('tasks', {title: j.string('Review'), done: j.boolean(false),
    priority: j.integer(1), project_id: j.reference(project.id)});
  await db.replace('tasks', review.id, {title: j.string('Review'), done: j.boolean(true),
    priority: j.integer(1), notes: j.string('looks good'), project_id: j.reference(project.id)});

  // 4. Queries: predicates, ordering, limits and relation navigation all run in the core.
  const open = await db.query('tasks').where('done', 'Eq', j.boolean(false)).fetch();
  assert.deepEqual(open.map(row => row.id), ['task-1']);
  const byPriority = await db.query('tasks').orderBy('priority', 'Asc').limit(1).fetch();
  assert.equal(byPriority[0].fields.title.value, 'Review');
  const projectTasks = await db.related('projects', project.id, 'tasks').fetch();
  assert.equal(projectTasks.length, 2);

  // 5. Reactive queries: the first event is the current snapshot, later events follow writes.
  const seen = [];
  const subscription = await db.query('tasks').where('done', 'Eq', j.boolean(false))
    .observe(event => { if (event.value.kind === 'rows') seen.push(event.value.rows.length); });
  await until(() => seen.at(-1) === 1, 'initial snapshot');

  // 6. Commands: a named, versioned intent whose handler writes through a scope and commits once.
  const addTask = {name: 'add-task-v1',
    arguments: {title: {kind: 'String', optional: false}}, reference_paths: []};
  const command = await db.execute(addTask, j.object({title: j.string('Ship')}), async scope => {
    const task = await scope.insert('tasks', {title: scope.args.value.title, done: j.boolean(false),
      priority: j.integer(3), project_id: j.reference(project.id)});
    return j.object({id: j.string(task.id)});
  });
  assert.equal(command.result.value.id.value, 'local:task-3');
  await until(() => seen.at(-1) === 2, 'optimistic command row');
  let entry = (await db.queueStatus()).commands.find(item => item.key === command.key);
  assert.equal(entry.state, 'queued');

  // docs:start manual-sync
  // 7. Sync, driven by hand here. preparePush returns frozen envelopes; the backend answers with
  // an outcome array, which receiveWire correlates through the same core codec.
  const batch = await db.preparePush(String(now), 0);
  const response = await runtime.encodeJson(j.array([j.object({
    idempotencyKey: j.string(command.key), status: j.string('applied'),
    result: j.object({id: j.string('server-task-3')})})]));
  await db.receiveWire(response);
  // docs:end manual-sync
  assert.equal(batch.envelopes.length, 1);
  const sent = await runtime.decodeJson(batch.envelopes[0]);
  assert.equal(sent.value.command.value, 'add-task-v1');
  assert.equal((await db.queueStatus()).commands.find(item => item.key === command.key).state, 'applied');

  // docs:start reconcile
  // 8. Reconcile: the application maps each locally created ID to its server identity.
  const [pending] = await db.pendingReconciliations();
  await db.reconcile(pending.key, {writes: [], mappings: {'local:task-3': 'server-task-3'}});
  // docs:end reconcile
  assert.equal(pending.key, command.key);
  const shipped = await db.query('tasks').where('id', 'Eq', j.string('server-task-3')).fetch();
  assert.equal(shipped.length, 1);

  // 9. Entity sync: opted-in tables record generated per-field patches instead of commands.
  const comment = await db.insert('comments', {body: j.string('first draft')});
  await db.configureEntities('tour', ['comments']);
  await db.patch('comments', comment.id, {body: j.string('final')});
  entry = (await db.queueStatus()).commands.at(-1);
  assert.equal(entry.kind, 'entity');
  assert.equal(entry.table, 'comments');

  // 10. Errors carry a static kind and never leak payloads.
  await assert.rejects(db.insert('tasks', {title: j.string('No project')}), error => {
    assert.equal(typeof error.kind, 'string');
    return true;
  });

  await db.unsubscribe(subscription);
  // Retain the actual adapter across owner close/reopen.
  await db.close();
  await db.open(options);
  assert.equal((await db.query('tasks').where('id', 'Eq', j.string('server-task-3')).fetch()).length, 1);
} finally {
  await db.close();
  await runtime.dropMemoryStore(store);
  facade.free();
}
console.log('javascript tour: ok');
