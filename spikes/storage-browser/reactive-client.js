// API-0005 / TC-0029 R13: host-driven owned delivery; no callback enters WASM.
const failure = message => Object.assign(new Error(message), { kind: 'BackendFailure' });
export class ReactiveClient {
  #send; #handle = null; #generation = 0; #callbacks = new Map(); #draining = false; #opening = false;
  callbackErrors = [];
  constructor(send) { this.#send = send; }
  async #request(op, data = {}, handle = this.#handle) {
    const response = await this.#send({ op, handle, ...data });
    if (response.error) throw Object.assign(new Error(response.error.message || response.error.kind), response.error);
    return response.value;
  }
  #live() { if (!this.#handle) throw failure('reactive client is closed'); return this.#handle; }
  async open(name = 'reactive-client.sqlite') {
    if (this.#opening) throw failure('concurrent replacement rejected');
    this.#opening = true;
    const generation = ++this.#generation, previous = this.#handle;
    this.#handle = null; this.#callbacks.clear();
    try {
      if (previous) await this.#request('reactive_close', {}, previous);
      const opened = await this.#request('reactive_open', { name }, null);
      if (generation !== this.#generation) { await this.#request('reactive_close', {}, opened.handle); throw failure('open canceled'); }
      this.#handle = opened.handle;
      return this;
    } finally { this.#opening = false; }
  }
  async close() {
    const handle = this.#handle;
    ++this.#generation; this.#handle = null; this.#callbacks.clear();
    if (handle) await this.#request('reactive_close', {}, handle);
  }
  async observe(mode, callback, columns = []) {
    if (typeof callback !== 'function') throw failure('callback required');
    const handle = this.#live(), generation = this.#generation;
    const token = await this.#request('reactive_observe', { mode, columns }, handle);
    if (generation !== this.#generation || handle !== this.#handle) throw failure('registration owner replaced');
    this.#callbacks.set(token, callback); return token;
  }
  async unsubscribe(token) {
    const handle = this.#live();
    const present = this.#callbacks.delete(token);
    if (!present) return false;
    return this.#request('reactive_unsubscribe', { token }, handle);
  }
  fetch() { return this.#request('reactive_fetch', {}, this.#live()); }
  insert(fields, table = 'orders') { return this.#request('reactive_insert', { table, row: { id: '', fields } }, this.#live()); }
  replace(row, table = 'orders') { return this.#request('reactive_replace', { table, row }, this.#live()); }
  remove(id, table = 'orders') { return this.#request('reactive_remove', { table, id }, this.#live()); }
  batch(mutations) { return this.#request('reactive_batch', { mutations }, this.#live()); }
  stats() { return this.#request('reactive_stats', {}, this.#live()); }
  refresh() { return this.#request('reactive_refresh', {}, this.#live()); }
  failNextQuery() { return this.#request('reactive_fail_next_query', {}, this.#live()); }
  failNextProject() { return this.#request('reactive_fail_next_project', {}, this.#live()); }
  async drain(now = Math.floor(performance.now())) {
    if (this.#draining) throw failure('recursive or concurrent drain rejected');
    if (!Number.isSafeInteger(now) || now < 0) throw failure('safe monotonic millisecond clock required');
    const handle = this.#live(), generation = this.#generation;
    this.#draining = true; let delivered = 0;
    try {
      const events = await this.#request('reactive_drain', { now: String(now) }, handle);
      for (const event of events) {
        if (generation !== this.#generation || handle !== this.#handle) break;
        const callback = this.#callbacks.get(event.subscription);
        if (!callback) continue;
        try { await callback(structuredClone(event)); }
        catch (error) { this.callbackErrors.push(error); }
        delivered++;
      }
      return delivered;
    } finally { this.#draining = false; }
  }
}
