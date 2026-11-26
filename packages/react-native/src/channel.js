// API-0011: native transport only; Rust owns all database semantics.
const invalid = () => { throw Object.assign(new Error('InvalidRequest'), {kind:'InvalidRequest'}); };
// SPEC-0036 BK10: host rejections are not core admission responses.
const rejectionKinds = {
  open: new Set(['InvalidRequest','BackupPolicyFailure','Busy','Closed']),
  call: new Set(['InvalidRequest','Closed']),
  namespace: new Set(['Closed']),
};
async function forward(operation, invoke) {
  try { return await invoke(); }
  catch (error) {
    let code;
    try { code = error?.code; } catch { /* unreadable native details stay private */ }
    const kind = rejectionKinds[operation].has(code) ? code : 'BackendFailure';
    throw Object.assign(new Error(kind), {kind});
  }
}
const bytes = (value, length) => {
  if (!Array.isArray(value) || value.length !== length || value.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)) invalid();
  return Uint8Array.from(value);
};

export function nativeChannel(native) {
  if (!native || typeof native.open !== 'function' || typeof native.call !== 'function' || typeof native.onAdmissionReleased !== 'function') invalid();
  return {
    open(input, key) {
      if (typeof input !== 'string' || !(key instanceof Uint8Array) || ![0,32].includes(key.length)) invalid();
      // Array is an owned separate TurboModule argument, never part of the frame.
      return forward('open', () => native.open(input, Array.from(key)));
    },
    call(input) {
      if (typeof input !== 'string') invalid();
      return forward('call', () => native.call(input));
    },
    subscribeRelease(listener) {
      if (typeof listener !== 'function') invalid();
      let active = true;
      const subscription = native.onAdmissionReleased(() => { if (active) listener(); });
      return () => { if (!active) return; active = false; subscription.remove(); };
    },
  };
}

export async function nativeNamespace(native) {
  return bytes(await forward('namespace', () => native.namespace()), 16);
}
