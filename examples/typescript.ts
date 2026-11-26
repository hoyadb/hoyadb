// A complete, strictly typechecked Todo app. Run with docs/public/run_language.mjs typescript.
// docs:start schema
import {openDatabase, defineSchema, table, column, defineCommand} from '@hoyadb/client';
import {createFakeTransport} from '@hoyadb/testing';

const schema = defineSchema({version: 1, tables: {
  todos: table({columns: {title: column.string(), done: column.boolean()}})
}});
// docs:end schema
function check(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}
async function until(test: () => boolean): Promise<void> {
  for (let turn = 0; turn < 2000; turn++) {
    if (test()) return;
    await new Promise(resolve => setTimeout(resolve, 1));
  }
  throw new Error('observer did not receive the committed change');
}
// docs:start backend
const backend = createFakeTransport({
  handlers: {'complete-task-v1': () => ({status: 'applied', result: null})},
  entityHandlers: {todos: () => ({status: 'applied', result: null})},
  pull: async since => ({checkpoint: since ?? 'v1', complete: true, changes: []})
});
// docs:end backend
// docs:start open
const db = await openDatabase({
  schema, engine: 'memory', context: () => ({user: 'ada'}),
  entities: {scope: 'typescript-todos', tables: ['todos']},
  transport: backend, reconcile: () => ({mappings: {}, writes: []})
});
// docs:end open
try {
  // docs:start insert
  const task = await db.table('todos').insert({title: 'Learn the typed API', done: false});
  await db.sync(); // Entity-created IDs are final and get identity mappings automatically.
  // docs:end insert
  check(task.id.startsWith('entity:'), 'an entity insert has a final ID');
  // docs:start query
  const rows = await db.query('todos').where('done', '=', false).orderBy('title', 'asc').fetch();
  const title: string = rows[0].title;
  // docs:end query
  check(title === 'Learn the typed API', 'the query returns typed plain rows');
  // docs:start observe
  let remaining: number | undefined;
  const unsubscribe = db.query('todos').where('done', '=', false)
    .observe(rows => { remaining = rows.length; });
  await unsubscribe.ready;
  // docs:end observe
  await until(() => remaining === 1);
  // docs:start command
  const completeTask = db.command(defineCommand('complete-task-v1', {
    args: {taskId: 'string'},
    handler: async (tracked, args) => {
      return tracked.table('todos').findById(args.taskId).update({done: true});
    }
  }));
  await completeTask.execute({taskId: task.id});
  // docs:end command
  await until(() => remaining === 0);
  check((await db.table('todos').findById(task.id).fetch())?.done === true, 'handler committed');
  // docs:start sync
  await db.sync();
  const status = await db.queueStatus();
  // docs:end sync
  check(status.commands.length === 2 && status.commands.every(entry => entry.state === 'applied'), 'entity and command confirmed');
  check(backend.pushes.flat().some(envelope => envelope.command === 'complete-task-v1'), 'server handler received command');
  // docs:start cleanup
  await unsubscribe();
  // docs:end cleanup
} finally {
  // docs:start close
  await db.close();
  // docs:end close
}
console.log('typescript tour: ok');
