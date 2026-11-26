# Phase 5 native entity sync verification

The reference Swift host exercises the reviewed API-0009 entity and pull operations
through the existing C ABI and shared Rust `CommandDatabase<SqliteStorage>` fixture.
Ordinary batches and patches capture trusted context once; command-scope writes
retain command-only ownership. Pull responses stay opaque until the production
Rust parser receives bounded UTF-8 chunks. Successful reset invalidates old host
generations; failed reset preserves subscriptions and pending work.

Final verification passed **24 named runtime groups** on the same freshly built
native library: five original ABI cases, three scope cases, two independent
persistence processes, four reactive cases, six Phase 4 command cases and four
Phase 5 entity cases. All five Swift consumers compiled with warnings as errors;
strict Swift formatting passed. The full command/entity executable took
116.654 seconds. This run used Rust 1.98.1 debug and unoptimized Swift 6.3.2 on
x86_64 macOS 26.7.1, targeting macOS 26.5.

- E-N01 verifies same-value patches, omitted optional fields, stable entity IDs,
  command/diff mixed HTTP arrays, cancellation undo and typed correction clocks:
  optimistic 1000 becomes authoritative 25; pull 24 preserves it and 26 updates it.
- E-N02 verifies two actual URLSession pages, readiness/progress, and a
  360,264-byte UTF-8 page uploaded in three chunks without publication before apply.
- E-N03 verifies suspended-handler Busy retry, fresh-ticket StalePull refetch,
  protected optimistic-delete conflicts, reset callback invalidation and late
  push/pull replies from an old generation.
- E-N04 verifies independent-process recovery of partial bootstrap, frozen diff
  bytes and retry deadline after a named post-commit SIGKILL barrier, then resumes
  the actual HTTP continuation. This demonstrates process termination, not power loss.

[Final commands and output](phase-five-native-output.txt),
[structured results](../../spikes/cross-platform/native/ENTITY_EVIDENCE.json) and
[pre-runtime source/binary hashes](../../spikes/cross-platform/native/ENTITY_SOURCE_SHA256.json)
record the observed results. The manifest captures 77 source files and six built
artifacts; every captured hash matched after runtime. Exact build and runtime
commands are included in the results. The HTTP fixture was the task-owned
`http://127.0.0.1:8775` server; request and resource deadlines remained five seconds.
Successful harnesses removed only their own temporary stores.

The compiling initial Swift stubs produced four behavioral failures before the
host implementation; the typed-correction test also failed against the prior
linked parser before the production correction branch landed. See
[RED narrative and reproduction](../../spikes/cross-platform/native/ENTITY_RED.md)
and local `target/phase-five-swift-{red,correction-red}.{txt,swift}` evidence.
A later real regression showed an optimistic delete letting an incoming update
advance the checkpoint silently. The unchanged native test first failed with
`E-N03 optimistic tombstone checkpointed away authoritative update`
([observed RED](../../target/phase-five-swift-protected-red.txt)); after the shared
core conflict guard was corrected, final E-N03 passed. Its RED output/source
hashes are retained in the structured final results.

This is a desktop reference boundary, not a packaged Swift SDK, mobile validation,
general concurrent-FFI proof or production backend. It adds no runtime dependency.


## Verification wording clarification (2026-10-24)

The preserved raw command record in phase-five-native-output.txt line32 invokes
strict swift-format only for commands.swift. All five consumers compiled with
warnings as errors and ran; the earlier broad formatting wording should be read
as applying only to that modified command consumer. Phase 6 rechecked the four
unchanged historical Swift fixtures and observed existing style diagnostics;
their runtime/compilation results remain separate from formatting. Raw historical
output is preserved, and no runtime result is relabeled by this clarification.
