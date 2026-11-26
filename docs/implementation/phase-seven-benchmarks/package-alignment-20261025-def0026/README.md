# Browser package footprint alignment for the DEF-0026 client tarball

This is a static input-alignment record. No browser, worker, SDK runtime, timing
or memory measurement ran. It records the actual footprint of the client
tarball that the
[DEF-0026 declaration fix](../../phase-seven-portable-declarations/README.md)
produced (committed `82cffb8`), using the existing benchmark footprint collector
unchanged. Earlier browser manifests, footprints and correctness cuts remain
immutable historical records for their own tarballs. They are not rewritten or
reinterpreted here.

## Inputs (read-only)

| Tarball | Bytes | SHA256 |
| --- | --- | --- |
| new `hoyadb-client-0.1.0.tgz` (`target/phase-seven-portable-declarations/after/`) | 1481408 | `fb710c70c87cbc8f327d7ee2b97b6eb4365ed25c63c6938c25553b51124c17ad` |
| `hoyadb-testing-0.1.0.tgz` (unchanged) | 9465 | `808802a4b4e0d341a3d0fbc2fce38d535d5ad3ff12e464e18b32a082a8d5d9f1` |
| historical `hoyadb-client-0.1.0.tgz` (benchmark lane prepare copy) | 1481367 | `b1cec02df8393a7348517a8ae447340aad1f02a66e128f2a5ad491b1dd509530` |

No WASM was regenerated, and no shared or historical tarball was modified.

## Method

`package_alignment.py`, whose SHA256 is recorded in `alignment.json`, was run
as `python3 -I package_alignment.py <repo> <new client> <testing> <historical
client> <historical prepare manifest> alignment.json`. It does four things:

1. It reads every regular tar member of the historical and new tarballs and
   records per-file bytes and SHA256.
2. It copies the new client and testing tarballs into a fresh owned OS
   temporary application. It installs them with `npm install --offline
   --ignore-scripts --no-audit --no-fund` and checks that the installed
   dependency graph is exactly the two packages.
3. It runs the existing `benchmarks/browser_run.py` `package_footprint`
   collector. That covers installed per-file bytes and kinds, plus each file's
   gzip size with `gzip.compress(data, compresslevel=9, mtime=0)`.
4. It checks that every installed file is byte-identical to its tar member with
   no extra files. It then compares the installed footprint per file against the
   historical prepare manifest and removes the temporary directory.

Tools: Python 3.14.8, Node v22.18.0, npm 11.17.0. The collector file's SHA256 is
`f7ff2bc5…`.

## Results (`alignment.json`)

- Both versions have 105 tar files, with the same file set. 104 are
  byte-identical. The only changed member is `client/package/index.d.ts`, which
  went from 11579 bytes `0fb04bd3…` to 11725 bytes `4b2d7594…`. This matches
  the DEF-0026
  [comparison](../../phase-seven-portable-declarations/comparison.json).
- The installed graph is exactly the root plus `@hoyadb/client` and
  `@hoyadb/testing`. There are no install-fidelity mismatches, and the owned
  temporary directory was removed.
- Installed client total, measured: 5512775 bytes. Of that, WASM is 5001038,
  JavaScript 69830, notices 368204, other 73703. Installed testing total:
  23283 bytes, unchanged.
- Per-file installed delta against the historical footprint: only
  `@hoyadb/client/index.d.ts` changed, from 11579 bytes (gzip 3251) to 11725
  bytes (gzip 3293). Every other installed file has identical bytes and gzip
  size.

## Related evidence and limits

The DEF-0026 evidence records:

- strict packed-declaration checks: an ES2022-only portable consumer, and DOM
  and RN declaration checks against the packed tarballs;
- the packed consumer's actual WASM/core memory cases passing after the fix;
- `npm test` with 84/84 passing.

It ran no new browser, mobile or RN-device runtime. This record adds none
either. Any future browser benchmark run must prepare its installation from
these exact tarball identities. Accepted latency or resource measurement still
requires an explicit root GO and the full protocol.
