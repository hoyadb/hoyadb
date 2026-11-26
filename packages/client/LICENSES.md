# Licensing

First-party HoyaDB code is licensed under MIT; see LICENSE. Package publication
and registry ownership remain separate release prerequisites.

The included WASM embeds the reviewed Rust/SQLite-family implementation. Its exact
Cargo.lock is included under src/wasm. SQLCipher, SQLite, sqlcipher-src and
LibTomCrypt notices are included under licenses/sqlcipher. Resolved Rust dependency notices
and exact version/source/license metadata are included under licenses/crates;
this set includes build/cfg-only packages and is not a claim that each executes
on wasm32. Complete reviewed source and
license evidence is maintained in docs/implementation/phase-six-dependencies.md
and the Phase 7 dependency packet in the source repository. This notice is not a
claim that third-party code uses this project's license.
