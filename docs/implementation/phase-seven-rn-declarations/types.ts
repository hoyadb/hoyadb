// Public-entry consumer probe for the packed RN declarations; type-level only, never executed.
import {createNativeRuntime, defineSchema, j, openMemoryDatabase, openNativeDatabase, query} from '@hoyadb/react-native';
import type {Database, Handle, Runtime, Schema} from '@hoyadb/react-native';
import {openMemoryDatabase as openTestingMemory} from '@hoyadb/testing';

const schema: Schema = {version: '1', tables: {notes: {columns: {body: {kind: 'String', optional: false}}, relations: {}}}};
const namespace = 'a8'.repeat(16);

export async function probe(): Promise<void> {
  const runtime: Runtime = createNativeRuntime();
  const store: Handle = await runtime.createMemoryStore();
  const retained: Database = await openMemoryDatabase({schema, store, namespace, options: {initial_delay_ms: '60000', max_delay_ms: '60000'}});
  const ephemeral: Database = await openMemoryDatabase({schema, store: null});
  const encrypted: Database = await openNativeDatabase({
    name: 'notes.sqlite', encryptionKey: new Uint8Array(32), schema, backupExcluded: true,
    clock: () => Date.now(), context: () => j.string('trusted'), options: {capacity: '64'},
  });
  // @ts-expect-error store open options are named `options`, as in the shared OpenOptions
  await openMemoryDatabase({schema, storeOptions: {}});
  // @ts-expect-error the protected profile requires a name and an encryption key
  await openNativeDatabase({schema});
  // @ts-expect-error an explicit store is a Handle, not a number
  await openMemoryDatabase({schema, store: 7});
  const shared: Database = await openTestingMemory({runtime, schema, namespace, store});
  const rows = await retained.query('notes').fetch();
  const body: string | undefined = rows[0]?.id;
  void [ephemeral, encrypted, shared, body, query('notes'), defineSchema];
  await Promise.all([retained.close(), ephemeral.close(), encrypted.close(), shared.close()]);
  await runtime.dropMemoryStore(store);
}
