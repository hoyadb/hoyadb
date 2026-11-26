# Licensing

First-party HoyaDB code is licensed under MIT; see LICENSE. Package publication
and registry ownership remain separate release prerequisites.

The WASM package includes its exact Cargo.lock under `src/wasm`. SQLCipher,
SQLite, sqlcipher-src and LibTomCrypt notices are under `licenses/sqlcipher`.
Resolved Rust dependency notices and exact version/source/license metadata are
under `licenses/crates`; that set includes build/cfg-only packages and does not
claim that every package executes on wasm32. Third-party terms remain their own;
this project's license does not replace them.
