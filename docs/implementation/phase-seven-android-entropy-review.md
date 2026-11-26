# Claude Opus Android entropy checkpoint

Reviewer: existing Claude Opus5 Paseo agent `e6f3b33e-7787-43bb-904f-646b096c9154`.
Review delivered2026-10-24 before Android provider/engine activation. Scope and
execution qualifications below are part of the verdict. This does not close
Phase 7 or establish Android target support.

## Android entropy adapter — PASS, no blocking concern

Reviewed `bindings/native/android/hoya_entropy.c`, `bindings/native/tests/entropy.c` (the adjacent test; `rg --files bindings/native/android` returns only the `.c`, the driver lives one level up) and `docs/implementation/phase-seven-android-crypto.md`.

### Verified, not assumed

- **Signature is byte-identical to the unit it replaces.** Upstream declares `unsigned long rng_get_bytes(unsigned char *out, unsigned long outlen, void (*callback)(void))` at `tomcrypt_prng.h:218-220` and defines it the same way at `prngs/rng_get_bytes.c:129-130` — no `LTC_EXPORT`/`LTC_CALL` decoration to mismatch. `hoya_entropy.c:33-34` matches exactly, so there is no silent type/ABI divergence. And because this is a symbol replacement, failing to exclude the upstream unit is a duplicate-symbol **link error**, not a silent revert to the clock fallback.
- **The caller is genuinely fail-closed.** `sqlcipher.c:115122-115128` does `bytes = rng_get_bytes(random_buffer, FORTUNA_MAX_SZ, NULL); if (bytes != FORTUNA_MAX_SZ) { … rc = SQLITE_ERROR; goto cleanup; }`. Returning 0 aborts provider init rather than seeding Fortuna weakly, so the doc's claim at line 152-154 holds against the real source.
- **Every failure path zeroizes the whole caller buffer.** Open failure (`:42-45`), partial read, EOF and EIO (`:52` break → `:57-60`), and close failure (`:56-60`) all call `clear_output(out, length)` for the full length, not the unfilled tail. `clear_output` (`:28-31`) stores through a `volatile unsigned char *`, so it is not optimizable away.
- **Descriptor 0 is handled correctly.** The guard is `fd < 0`, not `fd <= 0`, so a `/dev/urandom` opened as fd 0 (stdin closed) is used and closed normally. `entropy.c:19,23,35` deliberately return and assert fd 0, so this is covered, not accidental.
- **Close is never retried** (`:55-56`), which is the right call on Linux/bionic where EINTR may already have released and reused the descriptor.
- **I reran the fault suite**: `./target/phase-seven-native/entropy-faults` → `HOYA_ENTROPY_FAULTS_GREEN cases=9`, exit 0. The RED log is an authentic behavioral failure (`entropy.c:54`, the return-value assertion), not a build error.
- **I closed the one real coverage gap myself.** Every one of the nine cases compiles with `HOYA_ENTROPY_TEST`, so the real `open`/`read`/`close` path had zero executed coverage. I compiled the production path with `clang -std=c11 -Wall -Wextra -Werror -O2` and exercised it (`target/opus-entropy-probe/`): clean compile, two 32-byte reads differ, a 4096-byte read fills completely, `NULL` returns 0. The production path works and is warning-free. This was on macOS `/dev/urandom`, not Android.

### Non-blocking notes

- `HOYA_ENTROPY_TEST` appears nowhere in the deployment recipe (doc lines 96-99 cover the `LIBTOMCRYPT_SOURCES` exclusion but not the macro). Low risk, because defining it in a production build leaves three undefined externals and fails at link. Shortest correction: one sentence in the recipe stating the macro is test-only.
- No `fstat`/`S_ISCHR` check on the opened descriptor. libsodium and BoringSSL do this to defend against `/dev/urandom` being a regular file under a chroot or device-node-less namespace. Outside the Android app-sandbox threat model as scoped, so optional; the correction is a single `fstat` between `:45` and `:46`.
- The `(size_t)received > count` guard at `:52` is defensive against a hostile `read` and has no test. Not worth one.

### Qualification

This covers the adapter source, its faults and its production syscall path only. The Android provider and engine have not been built, no NDK cross-compile or on-device run has happened, and I verified no part of the sysroot/linking claims by execution — those remain open. This is not a final Phase 7 review.
