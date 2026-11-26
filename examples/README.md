# Runnable tutorials

These are the checked-in sources included by the public documentation. Keep one
copy of each tutorial and run it against the actual library. The
[documentation verifier](../docs/public/README.md) records language-specific
execution and source-only limits.

```sh
cargo run --manifest-path examples/Cargo.toml --bin quickstart --offline --locked
```

Build the WASM bridge and npm artifacts as described in
[the JavaScript build guide](../tools/javascript/README.md), then run:

```sh
node examples/javascript.mjs
node docs/public/run_application.mjs
node docs/public/run_language.mjs typescript
```

Swift, C, Kotlin and React Native sources have distinct toolchain requirements;
see [public documentation verification](../docs/public/README.md). A shared
JavaScript example does not establish execution in a native mobile application.
