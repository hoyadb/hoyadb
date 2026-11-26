// Source-checked only: requires the built native TurboModule in a React Native app.
// docs:start open
import {openMemoryDatabase} from '@hoyadb/react-native';
import {schema, backend, runTodo} from './react-native.mjs';

// For a protected persistent store, use openNativeDatabase with a caller-owned key.
// Create once in your application service, then share this owner across screens.
const db = await openMemoryDatabase({
  schema,
  context: () => ({user: 'ada'}),
  entities: {scope: 'react-native-todos', tables: ['todos']},
  transport: backend,
  reconcile: () => ({mappings: {}, writes: []})
});
await runTodo(db); // The complete example also disposes its watcher and closes.
// docs:end open
