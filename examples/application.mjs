// A complete Todo app with the HoyaDB JavaScript binding. The documentation build runs this exact
// file in Node and in a browser worker; the named regions below are the snippets on the
// getting-started and JavaScript pages. A tiny dependency-free check helper keeps it portable.
// docs:start schema
import {openDatabase, defineSchema, defineCommand, table, column} from '@hoyadb/client';
import {createFakeTransport} from '@hoyadb/testing';

const schema = defineSchema({
  version: 1,
  tables: {
    todos: table({
      columns: {
        title: column.string(),
        done: column.boolean(),
        completedAt: column.timestamp({optional: true}),
        completedBy: column.string({optional: true})
      }
    })
  }
});
// docs:end schema

function check(condition, message) {
  if (!condition) throw new Error(`check failed: ${message}`);
}
// Waits for an event delivered on a later host turn, with a bound.
async function until(predicate, message) {
  for (let turn = 0; turn < 2000; turn++) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 1));
  }
  throw new Error(`timed out waiting for ${message}`);
}

// docs:start backend
// Your server owns the tasks and re-runs each command with its own logic. In a real app this is
// an HTTP client; here the test backend from @hoyadb/testing plays the server.
const serverTask = (id, title) => ({
  table: 'todos', record_id: id, op: 'create',
  changes: {title, done: false}, removed_fields: [],
  timestamp_ms: '1000', field_timestamps: {title: '1000', done: '1000'}
});
const backend = createFakeTransport({
  pull: async since => since === null
    ? {checkpoint: 'v1', complete: true, changes: [serverTask('task-milk', 'Buy milk'), serverTask('task-docs', 'Write docs')]}
    : {checkpoint: since, complete: true, changes: []},
  handlers: {
    'complete-task-v1': envelope => ({status: 'applied', result: {taskId: envelope.args.taskId}}),
    // add-task-v1 creates exactly one task; the server stores it and answers with its new ID.
    'add-task-v1': envelope => ({status: 'applied', result: {id: 'server-new-task'}})
  },
  entityHandlers: {
    todos: () => ({status: 'applied', result: null})
  }
});
// docs:end backend

// docs:start open
const db = await openDatabase({
  schema,
  engine: 'memory',
  context: () => ({user: 'ada'}),
  entities: {scope: 'todo-app', tables: ['todos']},
  transport: backend,
  // Called for each confirmed command. `created` lists the local IDs the command created. Only
  // add-task-v1 creates a row, and its server result names the new ID, so map that one row.
  // Rows created in entity tables keep their IDs; HoyaDB maps those automatically.
  reconcile: ({kind, created, outcome}) => {
    const localIds = Object.keys(created);
    if (kind === 'command' && localIds.length === 1 && outcome.result?.id) {
      return {mappings: {[localIds[0]]: outcome.result.id}, writes: []};
    }
    return {mappings: {}, writes: []};
  }
});
await db.sync(); // pull the server's tasks
// docs:end open

try {
  // docs:start query
  const open = await db.query('todos')
    .where('done', '=', false)
    .orderBy('title', 'asc')
    .fetch();
  // open: [{id: 'task-milk', title: 'Buy milk', done: false}, {id: 'task-docs', title: 'Write docs', done: false}]
  // docs:end query
  check(open.map(task => `${task.id}:${task.title}:${task.done}`).join() === 'task-milk:Buy milk:false,task-docs:Write docs:false',
    'query returns ordered plain rows');
  check(!('completedAt' in open[0]), 'missing optional fields stay missing');

  // docs:start observe
  let openTitles = [];
  const stopWatching = db.query('todos')
    .where('done', '=', false)
    .observe(rows => { openTitles = rows.map(task => task.title).sort(); });
  await stopWatching.ready;
  // The callback first receives the current rows on a later turn, then again after each change.
  // docs:end observe
  await until(() => openTitles.join() === 'Buy milk,Write docs', 'initial snapshot');

  // docs:start command-definition
  const completeTaskV1 = defineCommand('complete-task-v1', {
    args: {taskId: 'string'},
    async handler(db, args, ctx) {
      // This `db` is the handler's tracked view: reads see the handler's own writes, and
      // everything commits atomically when the handler returns.
      const task = db.table('todos').findById(args.taskId);
      if (!(await task.fetch())) throw new Error('no such task');
      return task.update({done: true, completedAt: ctx.now(), completedBy: ctx.identity.user});
    }
  });
  // docs:end command-definition

  // docs:start execute
  const completeTask = db.command(completeTaskV1);
  const completed = await completeTask.execute({taskId: 'task-milk'});
  // completed: {id: 'task-milk', title: 'Buy milk', done: true, completedAt: Date, completedBy: 'ada'}
  // docs:end execute
  check(completed.done === true && completed.completedBy === 'ada', 'the handler result is the updated task');
  check(completed.completedAt instanceof Date, 'timestamps come back as Date objects');
  await until(() => openTitles.join() === 'Write docs', 'the observer sees the optimistic change');
  let rejected = false;
  try { await completeTask.execute({taskId: 'task-missing'}); } catch { rejected = true; }
  check(rejected, 'a throwing handler rejects execute');
  check((await db.query('todos').where('done', '=', true).fetch()).length === 1, 'a thrown handler changes nothing');
  const queued = (await db.queueStatus()).commands.map(entry => `${entry.kind}:${entry.state}`);
  check(queued.join() === 'command:queued', `only the command waits to sync, got ${queued}`);

  // docs:start sync
  await db.sync();
  const states = (await db.queueStatus()).commands.map(entry => entry.state);
  // states: ['applied'] — the server re-ran complete-task-v1 and confirmed it
  // docs:end sync
  check(states.join() === 'applied', `the command synced, got ${states}`);
  const sent = backend.pushes.flat();
  check(sent.length === 1 && sent[0].command === 'complete-task-v1' && sent[0].args.taskId === 'task-milk',
    'the backend received the command with its arguments');

  // docs:start entity
  // Outside a command, updating an entity table records a per-field patch that syncs by itself.
  await db.table('todos').findById('task-docs').update({title: 'Write the getting-started page'});
  await db.sync();
  // docs:end entity
  const renamed = await db.table('todos').findById('task-docs').fetch();
  check(renamed.title === 'Write the getting-started page', 'the patch is applied locally');
  const last = (await db.queueStatus()).commands.at(-1);
  check(last.kind === 'entity' && last.state === 'applied', 'the patch was confirmed by the backend');
  check(backend.pushes.flat().some(envelope => envelope.table === 'todos' && envelope.recordId === 'task-docs'),
    'the backend received the entity patch');
  await until(() => openTitles.join() === 'Write the getting-started page', 'the observer sees the patch');

  // docs:start entity-insert
  // Inserting into an entity table outside a command creates the row with its final ID at once.
  const call = await db.table('todos').insert({title: 'Call the bank', done: false});
  await db.sync();
  // docs:end entity-insert
  check(call.id.startsWith('entity:'), `entity rows get a final ID, got ${call.id}`);
  check(backend.pushes.flat().some(envelope => envelope.table === 'todos' && envelope.recordId === call.id),
    'the backend received the new entity row');
  check((await db.queueStatus()).commands.at(-1).state === 'applied', 'the entity insert is confirmed');
  await until(() => openTitles.includes('Call the bank'), 'the observer sees the inserted task');

  // docs:start create-command
  const addTaskV1 = defineCommand('add-task-v1', {
    args: {title: 'string'},
    handler: (db, args) => db.table('todos').insert({title: args.title, done: false})
  });
  const addTask = db.command(addTaskV1);
  // docs:end create-command

  // docs:start create-execute
  const draft = await addTask.execute({title: 'Plan the launch'});
  // draft.id is a temporary local ID such as 'local:1' until the server confirms the task.
  await db.sync();
  const [saved] = await db.query('todos').where('title', '=', 'Plan the launch').fetch();
  // saved.id is now 'server-new-task': reconcile mapped the local ID to the server's ID.
  // docs:end create-execute
  check(draft.id.startsWith('local:'), `the optimistic ID is local, got ${draft.id}`);
  check(saved && saved.id === 'server-new-task', `the synced task has the server ID, got ${saved?.id}`);
  check(!(await db.table('todos').findById(draft.id).fetch()), 'the local ID no longer exists');
  check((await db.queueStatus()).commands.at(-1).state === 'applied', 'add-task-v1 is confirmed');
  await until(() => openTitles.includes('Plan the launch'), 'the observer sees the created task');

  // docs:start cleanup
  await stopWatching();
  // docs:end cleanup
} finally {
  // docs:start close
  await db.close();
  // docs:end close
}
console.log('application tour: ok');
