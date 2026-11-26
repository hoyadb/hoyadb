# Supplied-key storage and safe diagnostics

Implemented surfaces are [API-0010](../design/contracts/API-0010-secure-storage-and-diagnostics.md)
and [ADR-0034](../design/decisions/ADR-0034-protected-engines-and-diagnostics.md).
These packages remain unpublished. Phase 6 evidence and platform limits are in
[the implementation report](../implementation/phase-six-security.md).

## Rust storage

The application supplies its original 32-byte raw key on each open. This helper
uses the actual adapter API; the application owns entropy, protected key storage,
identity and key lifecycle:

```rust
use std::path::Path;
use hoyadb_core::storage::StorageError;
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage};

pub fn open(path: &Path, application_key: [u8; 32]) -> Result<SqliteStorage, StorageError> {
    SqliteStorage::open_with_options(path, SqliteOpenOptions {
        encryption_key: Some(EncryptionKey::from_bytes(application_key)),
    })
}
```

`SqliteStorage::open(path)` and options without a supplied key reject before path
inspection. A protected open verifies SQLCipher4.19.0 and its authenticated v4
AES-256-CBC/HMAC-SHA512 profile before inspecting/initializing the private schema.
Wrong keys, plaintext stores and foreign/corrupt formats reject without conversion
or reset. Keep the original key and a compatible reader when reverting source.
The explicit `open_unprotected` factory serves historical synthetic regression
controls; it supplies no at-rest protection claim.

The generic storage traits and core remain key-free. Adapter-owned raw-key and
key-SQL buffers use zeroize. Foreign runtimes, parser copies and process memory
have their own lifetimes; the API cannot promise erasure of every copy. Keys are
absent from owned diagnostics, error frames and durable command/entity recovery.

## Foreign reference hosts

The experimental worker client uses
`client.open(filename, { encryptionKey: suppliedUint8Array32 })` and never
falls back on failure. Existing synthetic controls use `openUnprotected`.
The native reference host passes its supplied key only after validating the
exact pre-established root and applying/read-checking its backup policy.
These reference hosts retain the current scoped test schema and ABI; a general
consumer SDK is Phase 7 work.

Native Foundation backup exclusion defaults true on the owned directory, covering
its SQLite sidecars. Explicit opt-in writes/read-checks false. Revalidate closed
owner/file replacement before reopening. The resource value is an OS hint;
external copies and arbitrary backup tools remain outside its guarantee. Browser
OPFS reports backup exclusion `unsupported`. Origin isolation and persist()
provide different capabilities. No automatic export, rotation, rekey or migration
operation is provided.

## Diagnostics and explicit data access

`CommandDatabase::diagnostics()` returns exactly schema_version, pending, sending,
awaiting_outcome, live, blocked, ready, pages, changes and tombstones. It reads
owned state without storage I/O or identifier/payload cloning. Host counter
values use decimal strings and flags use Booleans. No destination or automatic
logging/telemetry is configured.

API-0010's enumerated context/recovery/transport/event/error Debug boundaries
emit bounded type/variant text. Explicit application inspection of data/config
and directly exported records is application-owned data access; this is not a
blanket guarantee that arbitrary application logging is safe. Full retry/data
serialization remains complete. Handler context is an immutable owned view;
JavaScript rejects nested and non-strict mutations through await. Its Proxy view
supports ordinary JSON serialization but does not support structuredClone.

Only the caller's explicit transport performs network work. Protect and authorize
backend operations independently; this library never infers membership or caller
identity from local rows. Current evidence covers native macOS, pinned Linux
engine tests and dedicated-worker OPFS. Mobile/new binding breadth, consumer
packaging and benchmark budgets are verified separately in Phase 7.
