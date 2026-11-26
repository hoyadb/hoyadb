#ifndef HOYA_PHASE3_H
#define HOYA_PHASE3_H
#include <stdint.h>
#include <stddef.h>

/* SPEC-0027 / TC-0029 R14: fixed synthetic fixture over the production
 * ReactiveDatabase<MemoryStorage>, not a packaged consumer API.
 * Required strings are readable NUL-terminated UTF-8 for the duration of calls.
 * All callback arrays/byte spans are borrowed only until callback return:
 * copy them if needed later. Context remains caller-owned; nothing is retained.
 * Callbacks must never throw/unwind across C. Calls are serial in this harness.
 * A callback may query/write/unsubscribe/close, but recursive drains reject.
 * Status: 0 success, 1 stale handle/token, 2 invalid input, 3 panic/poison,
 * 4 core/storage failure, 5 recursive drain. Panic is not rollback/recovery.
 * Events execute synchronously on the drain caller thread, with no Rust
 * registry lock or owner borrow held. Callback writes wait for the next drain.
 * Handles and subscriptions are opaque and must not be reused across owners. */
typedef struct {
    const uint8_t *id;
    size_t id_len;
    const uint8_t *status;
    size_t status_len;
    const uint8_t *notes;
    size_t notes_len;
    int64_t total_cents;
} hoya_reactive_row;

typedef void (*hoya_reactive_rows_callback)(void *context,
    const hoya_reactive_row *rows, size_t row_count);
/* kind: 0 Rows, 1 Count, 2 Error. Row snapshots contain every fixed-schema
 * application field; managed fields are excluded. Count events have no rows.
 * error_code mirrors StorageError variants: 1 TableNotFound, 2 RowAlreadyExists,
 * 3 RowNotFound, 4 InvalidSchema, 5 InvalidQuery, 6 InvalidRow,
 * 7 BackendFailure, 8 SchemaConflict, 9 MigrationFailure. Zero means no error.
 * count is the row count for Rows or projected cardinality for Count. */
typedef void (*hoya_reactive_event_callback)(void *context,
    uint64_t subscription, uint8_t kind, size_t count,
    const hoya_reactive_row *rows, size_t row_count, uint32_t error_code);

uint64_t hoya_reactive_open(void);
int32_t hoya_reactive_close(uint64_t handle);
int32_t hoya_reactive_insert(uint64_t handle, const char *status,
    const char *notes, int64_t total_cents);
int32_t hoya_reactive_replace(uint64_t handle, const char *id,
    const char *status, const char *notes, int64_t total_cents);
int32_t hoya_reactive_remove(uint64_t handle, const char *id);
int32_t hoya_reactive_query(uint64_t handle, const char *status,
    hoya_reactive_rows_callback visitor, void *context);
/* mode: 0 membership/default, 1 count, 2 observe total_cents only. */
uint64_t hoya_reactive_observe(uint64_t handle, const char *status, uint8_t mode);
int32_t hoya_reactive_unsubscribe(uint64_t handle, uint64_t subscription);
/* now_ms is a monotonic host sample. Count throttle is owned by core. */
int32_t hoya_reactive_drain(uint64_t handle, uint64_t now_ms,
    hoya_reactive_event_callback visitor, void *context);
/* SPEC-0028 / TC-0031 Q05, synthetic test fixture only: fail exactly the next
 * projected read, not a full-row query or write. This is not a real disk fault.
 * Refresh queues observation results; zero acknowledges the attempt even when
 * an Error notification records a failed read. Both helpers check owner liveness. */
int32_t hoya_reactive_fail_next_project(uint64_t handle);
int32_t hoya_reactive_refresh(uint64_t handle);
#endif
