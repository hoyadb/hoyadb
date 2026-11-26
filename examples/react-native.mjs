// Shared application logic. Executed against the real WASM core; native bootstrap is separate.
import {defineSchema, table, column, defineCommand} from '@hoyadb/client/application';
import {createFakeTransport} from '@hoyadb/testing';
// docs:start schema
export const schema = defineSchema({version: 1, tables: {
  todos: table({columns: {title: column.string(), done: column.boolean()}})
}});
// docs:end schema
// docs:start backend
export const backend = createFakeTransport({
  handlers: {'complete-task-v1': () => ({status: 'applied', result: null})},
  entityHandlers: {todos: () => ({status: 'applied', result: null})},
  pull: async since => ({checkpoint: since ?? 'v1', complete: true, changes: []})
});
// docs:end backend
function check(condition, message) { if (!condition) throw new Error(message); }
async function until(test) {
  for (let turn = 0; turn < 2000; turn++) {
    if (test()) return;
    await new Promise(resolve => setTimeout(resolve, 1));
  }
  throw new Error('observer delivery timed out');
}
export async function runTodo(db) {
  let unsubscribe;
  try {
    // docs:start query
    const task = await db.table('todos').insert({title: 'Read on the train', done: false});
    await db.sync();
    const rows = await db.query('todos').where('done', '=', false).fetch();
    // docs:end query
    check(rows.length === 1 && rows[0].id === task.id, 'local query');
    // docs:start observe
    let openCount;
    unsubscribe = db.query('todos').where('done', '=', false)
      .observe(rows => { openCount = rows.length; });
    await unsubscribe.ready;
    // In a screen, update component state here and dispose on unmount.
    // docs:end observe
    await until(() => openCount === 1);
    // docs:start command
    const complete = db.command(defineCommand('complete-task-v1', {
      args: {taskId: 'string'},
      handler: (tracked, args) => tracked.table('todos').findById(args.taskId).update({done: true})
    }));
    await complete.execute({taskId: task.id});
    // docs:end command
    await until(() => openCount === 0);
    // docs:start sync
    await db.sync();
    const queue = await db.queueStatus();
    // docs:end sync
    check(queue.commands.length === 2 && queue.commands.every(entry => entry.state === 'applied'), 'backend confirmations');
    check((await db.table('todos').findById(task.id).fetch()).done === true, 'committed row');
  } finally {
    // docs:start cleanup
    if (unsubscribe) await unsubscribe();
    await db.close();
    // docs:end cleanup
  }
}
