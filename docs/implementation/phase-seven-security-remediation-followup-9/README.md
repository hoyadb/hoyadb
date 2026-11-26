# Security remediation follow-up 9: SR-22d dual-resident schema successor

This implements the committed SPEC-0034 / TC-0038 SR-22d case (`1a7d2c5`).

The change is limited to `native_cache.py`, `test_native_cache.py` and
`README.md` under `bindings/react-native/tools/security/`. These are
unchanged: `kotlin_guard.py`, capture classification, the R1 pin, and the
other 13 sources.

What was not done:
- no enrollment, host reader, controller, collector or OS reads;
- no JVM, Gradle, `jcmd`, helper, JAR, native payload or network;
- no SR-32, and no baseline approved;
- nothing staged.

The predecessor is the follow-up 8 successor (16 sources). It was snapshotted
before the edit and equals the follow-up 8 manifest and Git HEAD.
[successor-manifest.json](successor-manifest.json) binds the two. All
predecessor packets are frozen.

## TDD record (logs in [logs/](logs/), normal and `-O`)

1. **Setup error, not RED**
   ([logs/setup-error-not-red-attempt1.txt](logs/setup-error-not-red-attempt1.txt)).
   - My first test draft referenced `N.SCHEMA_V2`. That produced 12
     `AttributeError` errors plus 2 assertion failures.
   - I preserved and relabelled it. The tests were then changed to use the
     literal schema string, so the RED would come from the guard's result
     through the existing API.
2. **Genuine RED** ([logs/red-test_native_cache.txt](logs/red-test_native_cache.txt)):
   49 subtest failures across 41 tests, **all**
   `AssertionError: False is not true`, with **0 errors**.
   - **Positive capability.** A valid version-2 dual-resident evidence set is
     rejected (`ok=False`):
     - classify once;
     - reviewed alias to canonical image;
     - dual image as mandatory;
     - an empty dual set behaving like version 1.
   - **Negatives.** Each one lacks its field-specific identity: today's
     rejection is only the generic schema/record-fields or "host record
     incomplete" message. That is exactly what the case requires the
     identity assertions to catch.
   - The existing version-1, mandatory-image and capture tests are
     characterization controls and were not relabelled. The new version-1
     characterization case and the unknown/candidate-schema case already
     passed.
3. **Implementation** ([changes.diff](changes.diff); `native_cache.py` +47/−7):
   - **Record.** `SCHEMA_V2`, `RECORD_KEYS_V2` (adds `dual_resident_images`),
     `HOST_KEYS_V2` (adds `dual_resident_files`) and `DUAL_KEYS`.
     - The key set is chosen by the exact schema string, so version 1 is
       unchanged: a version-1 record carrying the field rejects as
       `record fields: … unexpected ['dual_resident_images']`, and unknown
       or candidate schemas still reject.
     - A version-2 record without the field reports
       `dual_resident_images: required for version 2`.
     - `_dual_errors` rejects anything that is not a dict keyed by canonical
       `images` paths with values exactly `{bytes: positive non-bool int,
       sha256: 64 lowercase hex}`.
   - **Host.**
     - A version-1 host carrying `dual_resident_files` reports
       `<side> host: dual_resident_files present for a version 1 record`.
     - A version-2 host missing it reports
       `<side> host: dual_resident_files missing for a version 2 record`.
     - Version-2 values are compared type-strictly with the record,
       separately for pre and post.
     - In `image_path_status`, dual paths must be `regular`; anything else
       reports `<side> host: dual-resident image_path_status not regular:
       <path>`. Every other path and alias keeps the existing `absent` rule
       and message.
   - **Capture.** `lookup`, `check_capture` and `kotlin_guard` are unchanged:
     a digest-bearing claim still rejects even when it matches the disk hash.
4. **Field-specific violation identities** that the tests assert:
   - `system cache record: dual_resident_images` for every subset case: absent,
     non-dict, unregistered, alias, wildcard, prefix; bytes `True`, 0, −1,
     float or string; sha uppercase, short or missing; an extra value field;
   - `pre-capture host: dual_resident_files` and
     `post-capture host: dual_resident_files` for:
     - a version-1 host carrying the field, or a version-2 host missing it;
     - a sha, bytes or type change, a missing, extra or incomplete entry, or
       a non-dict;
     - pre and post that are both wrong in the same way (each reported);
   - `<side>-capture host: dual-resident image_path_status` for symlink,
     absent, other or `regular_file`.

   **Preserved behaviour:**
   - a non-dual path that is `regular`, or a dual alias that is `regular`,
     still reports `reviewed cache path not absent as a file`;
   - digest-bearing claim, different or unaligned slide, missing mandatory
     libSystem and `top_address` all still reject;
   - a version-2 record without `cache_files` still rejects.
5. **GREEN** ([final-green/green-run-identity.json](final-green/green-run-identity.json)):
   all six suites pass, normal and `-O`, **123 tests per mode**, all OK.
   - Python 3.14.8, interpreter sha256 `5f923e62…`.
   - Sources were identical before and after the run and equal the successor.

   | Suite | Tests |
   | --- | --- |
   | `test_kotlin_guard` | 28 |
   | `test_symbol_guard` | 9 |
   | `test_embedded_scan` | 11 |
   | `test_init_script_guard` | 9 |
   | `test_java_properties` | 25 |
   | `test_native_cache` | 41 |
6. **AST audit** ([logs/ast-audit.json](logs/ast-audit.json)) of `native_cache.py`:
   0 `assert` nodes; imports only `dataclasses` and `re`; no `open`, `exec`,
   `eval` or `__import__` calls.

## Successor sources

| File | SHA256 |
| --- | --- |
| `native_cache.py` | `cec043d7…` |
| `test_native_cache.py` | `74573a65…` |
| `README.md` | `8b7f4a02…` |

`kotlin_guard.py` and the R1 init-script pin are unchanged (verified in the
manifest).

## Limits (unchanged)

- Disk hashes bind the on-disk copies only. They are not the mapped cache
  bytes; the complete cache-file manifest stays the cache integrity anchor.
- The Mach-O UUID agreement between disk and cache is review-time evidence for
  any concrete entry and is not checked at runtime. No concrete entry is
  approved here.
- The trusted-host inference, partial native lifetime capture and missing
  effective properties remain as before.
