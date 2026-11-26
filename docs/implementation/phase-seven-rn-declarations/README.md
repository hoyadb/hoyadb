# Packed React Native declaration check

This is a read-only typecheck of the packed `@hoyadb/react-native`,
`@hoyadb/client` and `@hoyadb/testing` declarations, run against the packages
installed in the owned RN consumer (final RN tgz `1ac8d113…`).

## Verifier

It uses the established JavaScript verifier, as documented in
`bindings/javascript/README.md` (TypeScript 7.0.2) and invoked bare as `tsc` by
`bindings/javascript/scripts/packed-consumer.mjs`:

- `tsc` on PATH resolves to the global `typescript` 7.0.2 package
  (package.json `3722b302…`) under nvm Node 22.18.0;
- that package executes the native `@typescript/typescript-darwin-x64` 7.0.2
  binary (`lib/tsc`, `76e4987d…`);
- [check.py](check.py.txt) asserts both hashes before running.

Nothing was installed, upgraded or configured.

## Method

[check.py](check.py.txt) works as follows:

1. Copies only each package's `package.json` and `.d.ts` files into a fresh OS
   temporary directory outside the repository.
2. Writes an explicit `tsconfig.json`: strict, `noEmit`, `noEmitOnError`,
   ES2022/NodeNext, `types: []`, no `extends`, no plugins, `skipLibCheck`
   false.
3. Typechecks the public-entry probe [types.ts](types.ts). The probe covers the
   retained `store`, the ephemeral default, store-open `options`, the
   protected profile and the testing factory. It also has three
   `@ts-expect-error` negative cases: `storeOptions` is rejected, a missing
   name/key is rejected, and a numeric store is rejected.
4. Removes the temporary directory; every result records that removal.

## Results

| Variant | lib | Exit | Output |
| ------- | --- | ---- | ------ |
| [established JS set](result-es2022-dom.json) | ES2022, DOM | 0 | none |
| [RN-portable](result-es2022.json) | ES2022 | 1 | `@hoyadb/client/index.d.ts(101,117): error TS2304: Cannot find name 'AbortSignal'.` |
| [pre-fix RN declaration](result-es2022-dom-pre-fix.json) | ES2022, DOM | 1 | the old `storeOptions` API fails 3 probe expectations, so the probe detects the regression |

- The RN declarations and the probe produce no errors in either variant.
- The only RN-portable error comes from the shared client host declaration
  (`execute(..., options?: {signal?: AbortSignal})`). That file is owned by the
  JavaScript lane, which has sealed it.
- React Native 0.86.3's bundled `types/` declare no `AbortSignal`, and no
  `@types` package for it exists in the approved graph.
- Consequence: an RN TypeScript app that uses neither the `dom` lib nor its own
  `AbortSignal` declaration would see this error. It is reported to the root
  and the JavaScript owner, not patched here, and no types were faked.

The first ES2022-only run, made before the script took a lib argument, used
the identical configuration and gave the identical single error. I removed its
record file as a duplicate; the retained ES2022 result above came from the
re-run.
