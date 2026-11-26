#ifndef HOYA_SPIKE_H
#define HOYA_SPIKE_H
#include <stdint.h>
#include <stddef.h>

/* SPEC-0018: experimental, synchronous ABI; not a production API.
 * Validated host: serial Swift calls, possibly resumed on another OS thread.
 * Required strings must point to readable NUL-terminated memory for the call;
 * UTF-8 is validated and paid must be 0 or 1.
 * Callback byte spans are borrowed only until the callback returns; copy them
 * during the callback if they are needed afterward.
 * Context remains caller-owned; callbacks must not unwind across this ABI.
 * Status: 0 success, 1 invalid handle, 2 invalid input, 3 panic/poison,
 * 4 ordinary SQLite failure (memory/events/scope unchanged).
 * A panic status is not rollback; discard that store. */
typedef void (*hoya_row_callback)(void *context,
    const uint8_t *id, size_t id_len, const uint8_t *status, size_t status_len,
    int64_t total_cents, uint8_t paid);

uint64_t hoya_open(void); /* zero on failure; handles must not be reused */
uint64_t hoya_open_persistent(const char *path); /* zero on invalid path/storage */
/* Persistent mode commits SQLite before successful insert/scope publication.
 * Paths must satisfy string lifetime rules; no encryption/crash guarantee. */
int32_t hoya_close(uint64_t handle);
int32_t hoya_insert(uint64_t handle, const char *id, const char *status,
    int64_t total_cents, uint8_t paid);
int32_t hoya_query(uint64_t handle, const char *status,
    hoya_row_callback visitor, void *context);

/* Explicit drain: no callback or context is retained between calls.
 * Counts describe membership when queued; query returns current rows. */
typedef void (*hoya_event_callback)(void *context, uint64_t subscription, size_t count);
uint64_t hoya_observe(uint64_t handle, const char *status);
int32_t hoya_unobserve(uint64_t handle, uint64_t subscription);
int32_t hoya_drain(uint64_t handle, hoya_event_callback visitor, void *context);
uint64_t hoya_begin_scope(uint64_t handle); /* zero if busy/invalid/exhausted */
int32_t hoya_scope_insert(uint64_t handle, uint64_t scope, const char *id,
    const char *status, int64_t total_cents, uint8_t paid);
int32_t hoya_scope_query(uint64_t handle, uint64_t scope, const char *status,
    hoya_row_callback visitor, void *context);
int32_t hoya_finish_scope(uint64_t handle, uint64_t scope, uint8_t commit);
#endif
