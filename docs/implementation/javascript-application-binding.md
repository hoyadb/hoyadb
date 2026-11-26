# JavaScript application binding checkpoint — 2026-10-28

Implements API-0012, SPEC-0042 and TC-0046 after the operator requested that the
original JavaScript workflow actually work and that the documentation teach it.
Claude assisted with value helpers, the plain fake, the tutorial and reviews.

The application layer translates values and orchestrates host work over the same
core. It adds no storage or query implementation. `/host` remains compatible.
The only production Rust addition is a read-only pending reconciliation context
projection. It exposes persisted creation identities for restart-safe mapping,
with no args, undo, frozen bytes or outcome payload in that projection.

Verified on the working source snapshot (existing user edits preserved):

- `cargo test --workspace --offline --locked`: 698 passed, 0 failed, 76 groups.
- `cargo clippy --workspace --all-targets --offline --locked -- -D warnings`:
  passed (isolated task target directory).
- `npm test --prefix bindings/javascript`: 168 passed, 0 failed.
- `node --test bindings/react-native/test/*.test.mjs`: 10 passed, 0 failed;
  these test the existing JavaScript channel, not native application execution.
- `node bindings/javascript/scripts/packed-consumer.mjs`: local tarballs install
  offline; strict TypeScript consumers, the new Node app and existing real-WASM
  consumers pass. The plain fake is checked as an application transport.
- `python3 docs/public/verify.py`: all 19 commands pass, including 12 executable
  examples, a rebuilt ordinary WASM artifact, example lint and tooling checks.
- `node bindings/javascript/scripts/application-browser.mjs`: the complete app
  passes in an actual dedicated browser worker; a task-owned encrypted OPFS
  store writes, closes and reopens with the caller's key.
- The HTML build checks all local links and fragments, including Rustdoc
  redirection pages. The recorded Chrome check covers offline search, keyboard
  focus and all 30 configured pages at 320 px, plus wider home-page layouts.

Every new behavior has assertion-first tests; review findings received their own
regressions. Logs, setup failures, restored mutations and preservation manifests
are retained locally under `target/binding-application/`. Generated verification
reports bind their claims to hashes of the tested bytes.

`cargo fmt --all --check` still flags five pre-existing files. Each is unchanged
from the pre-task hash manifest; none contains this task's Rust additions.
Formatting those files is outside this change. The two overlapping Rust source
files preserve the user's overlay; only the new projection is staged.

Limits: packages remain unreleased and local. Node's default runtime is memory
only; browser persistence requires an application-owned key. Native application
execution, Swift/RN parity work, JVM/Android execution, R9/R10/R11 and the planned
Discarded policy are not completed by this change. A capped backoff for the
application scheduler's 10 ms Busy retry remains a non-blocking release review
recommendation; command completions already wake the scheduler immediately.
