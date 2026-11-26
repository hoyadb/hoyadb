# DEF-0026 portable host cancellation declaration — RED/GREEN evidence

Declaration-only correction for
[DEF-0026](../../design/defects/DEF-0026-portable-host-declaration-dom-signal.md).
The repository HEAD at test time was `abdf25c`. The changed files are uncommitted
in the working tree, and the root owns the commit.

## Runtime usage established from source

`client/src/client.js` `execute` (lines 101–104, 172) is the only consumer of
`options.signal` in the client and testing sources (excluding the generated
WASM). It uses exactly these members:

- `if (options.signal)`: a truthy object.
- `signal.addEventListener('abort', job.cancel, {once:true})`, where `job.cancel`
  takes no arguments.
- `signal.aborted`, which it reads once after registering the listener.
- `signal.removeEventListener('abort', job.cancel)`, called once on completion.

## Change

`bindings/javascript/client/index.d.ts:101` replaces `{signal?:AbortSignal}` with
an inline structural type that has exactly those members:
`{readonly aborted:boolean; addEventListener(type:'abort',listener:()=>void,options:{once:true}):void; removeEventListener(type:'abort',listener:()=>void):void}`.

- No new exported name.
- No global or ambient type.
- No dependency, `@types` package or `skipLibCheck`.
- No change to runtime JS, WASM or Rust, or to cancellation semantics.

Regression fixtures:

- **New `consumer/portable.ts`.** A strict host/testing consumer using ES2022
  only. A host-defined signal is accepted. Five genuine `@ts-expect-error`
  negatives are rejected: missing `aborted`, missing `removeEventListener`, a
  string `aborted`, a non-`abort` event type, and a Boolean signal.
- **`consumer/types.ts`, with `ES2022,DOM,ESNext.Disposable`.** An actual
  `new AbortController().signal` is accepted. An `{aborted:true}`-only shape is
  rejected.
- **`scripts/packed-consumer.mjs`.** It now also runs `tsc -p
  portable.tsconfig.json` with strict, noEmit, `lib:['ES2022']`, `types:[]` and
  `skipLibCheck:false` against the installed packed tarballs.
- **`rn-portable.ts`, evidence only.** It covers the RN public entry (a
  read-only copy of the RN lane's current `bindings/react-native/package.json`
  `d5dcb806…` and `index.d.ts` `8ea711a2…`) over the packed client/testing. It
  uses the same host-defined signal and two negatives.

## Tool provenance

`tsc` on PATH is `/Users/boristacyniak/.nvm/versions/node/v22.18.0/bin/tsc`. It
resolves to the global TypeScript 7.0.2 package (package.json `3722b302…`), which
runs the native darwin-x64 binary `76e4987d…`. These are the same hashes as the
[RN declaration evidence](../phase-seven-rn-declarations/README.md). Node is
v22.18.0. The [driver](check.mjs.txt) asserts both hashes before it runs.
Nothing was installed or updated.

## Method

The [driver](check.mjs.txt) is run as `node check.mjs <before|after>
<work-directory>`. It works as follows:

1. It mirrors the checked-in JS sources, the existing prebuilt
   `bindings/web/pkg`, the `Cargo.lock`, the SQLCipher licenses and the crate
   sources that the build manifest reads. They go into a fresh OS temporary
   directory, at `<owned>/repo`. Every mirrored file is hash-checked against
   the repository.
2. It runs the unchanged `scripts/build.mjs`, `check-build.mjs` and
   `packed-consumer.mjs` there. The packed consumer runs the DOM control, the
   new ES2022-only portable check, and then the actual memory WASM/core cases.
3. It installs the packed tarballs offline, adds the RN declarations, and
   typechecks `rn-portable.ts` and `portable.ts` with explicit tsconfigs (lib
   `ES2022` and `ES2022,DOM`). It also records `--listFilesOnly`, which shows
   that no `lib.dom.d.ts` is loaded in the ES2022 runs.
4. It extracts both tarballs, hashes every file, and then removes its owned
   directory (`temporary_removed: true`).

No shared `dist/` directory or shared tarball directory was touched.

## Results

| Check | Before (`record-before.json`) | After (`record-after.json`) |
| --- | --- | --- |
| build / check-build | 0 / 0 | 0 / 0 |
| packed-consumer: DOM `types.ts` | passes, then the run fails at the portable step | 0 |
| packed-consumer: ES2022 `portable.ts` | exit 1: `node_modules/@hoyadb/client/index.d.ts(101,117): error TS2304: Cannot find name 'AbortSignal'.` (plus TS2578 because the unresolved type admits every negative) | 0 |
| packed-consumer: actual WASM/core memory cases | not reached | `PASS independent packed npm consumer: types + actual WASM/core memory` |
| RN `rn-portable.ts`, ES2022 | 1 (the same TS2304, plus 2 TS2578) | 0 |
| RN `rn-portable.ts`, ES2022+DOM | 1 (TS2739: a host signal is not a DOM AbortSignal) | 0 |
| `portable.ts`, ES2022+DOM | 1 (TS2739) | 0 |

- The before run fails because of the defect itself. The tool, the imports and
  the setup all resolved: there is no TS2307 or TS2305 error, and the build and
  install exited 0.
- Each passing after run includes its `@ts-expect-error` negatives. A negative
  that the type accepted would have failed the run with TS2578.

## Existing runtime suite

`npm test --prefix bindings/javascript` was run in the repository, with the
runtime sources unchanged. Exit 0, with tests 84, pass 84 and fail 0. This
includes:

- H01, the never-settling handler with an actual `AbortController`;
- H02, delayed-finish cancellation;
- the release and G09 cancellation cases.

The output is preserved in a [lossless JSON wrapper](npm-test.log.json), with
the original byte hash and UTF-8 text; the raw `.log` suffix is ignored by Git.
Git status was identical before and after the run. Root independently compared
all105 packed files and confirmed that only `client/package/index.d.ts` differs.

## Package identities and byte proof ([comparison.json](comparison.json))

| Tarball | Before | After |
| --- | --- | --- |
| `hoyadb-client-0.1.0.tgz` | 1481367 bytes, `b1cec02d…` | 1481408 bytes, `fb710c70c87cbc8f327d7ee2b97b6eb4365ed25c63c6938c25553b51124c17ad` |
| `hoyadb-testing-0.1.0.tgz` | 9465 bytes, `808802a4…` | unchanged, `808802a4…` |

- The before client tarball is byte-identical to the historical shared
  `target/phase-seven-web/tarballs` artifact (`b1cec02d…`).
- Of the 105 files in the two packages, 104 are byte-identical. All 100
  non-declaration files are identical, including all JS, the WASM,
  `SOURCE-MANIFEST.json`, `package.json`, the READMEs and the licenses. The
  four other `.d.ts` files are also identical.
- The only changed file is `client/package/index.d.ts`, from 11579 bytes
  `0fb04bd3…` (the HEAD blob) to 11725 bytes `4b2d7594…`.
- The only source delta between the two mirrors is
  `bindings/javascript/client/index.d.ts`.

## Qualifiers

- No new browser, mobile or RN-device runtime was run. Earlier installed-browser
  packets and footprint metrics remain historical cuts for their recorded
  tarballs. The bytes that changed in the new client tarball are
  declaration-only.
- The RN declarations are the RN lane's current, uncommitted files, copied
  read-only. They were not modified.
- The before and after tarballs are kept outside the repository, under the
  git-ignored `target/phase-seven-portable-declarations/{before,after}/`.
