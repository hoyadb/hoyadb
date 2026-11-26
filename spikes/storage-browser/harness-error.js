// API-0010: only this harness's own labels may augment a static foreign kind.
const kinds = new Set(['BackendFailure', 'MigrationFailure', 'SchemaConflict', 'InvalidSchema', 'InvalidQuery', 'InvalidRow', 'TableNotFound', 'RowAlreadyExists', 'RowNotFound']);
export function workerError(error, label) {
  let kind;
  try { kind = error?.kind; } catch { /* Foreign accessors have no diagnostic authority. */ }
  if (!kinds.has(kind)) kind = 'BackendFailure';
  return Object.assign(new Error(`${label}: ${kind}`), { kind });
}
