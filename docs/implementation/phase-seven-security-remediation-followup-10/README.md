# Security remediation follow-up 10: SR-22d robustness correction

This is root's narrow correction GO under the existing SR-22b missing-identity
case and the SR-22d malformed-subset control.

The change is limited to `native_cache.py`, `test_native_cache.py` and
`README.md`. Unchanged: `kotlin_guard.py`, the R1 pin and the other 13
sources.

The sealed follow-up 9 packet (`files-sha256.json` `a704df53…`, 22 files) is
not modified. This is a separate correction cut. The predecessor is the
follow-up 9 successor, snapshotted before the edit and equal to it (see
[successor-manifest.json](successor-manifest.json)).

What was not done: no OS reads, no JVM, Gradle or network, no host reader,
collector or enrollment, no SR-32, nothing staged.

## Defect (root finding, reproduced)

**Probe.** [logs/defect_probe.py.txt](logs/defect_probe.py.txt) is pure: it
builds a v2 record whose first image has no `install_path` and calls
`N.check_host`. Before the fix, in both modes:

```
RAISED KeyError 'install_path'
```

([logs/defect-probe.txt](logs/defect-probe.txt), [-O](logs/defect-probe-O.txt))

**Cause.** `_image_errors` correctly returned a rejection. `_record_errors`
then still called `_dual_errors`, which built `{i['install_path'] for i in
images}` before the image identities were known to be valid.

## TDD

1. **Regression first.**
   `test_sr22d_missing_image_identity_rejects_instead_of_raising` removes each
   of `install_path`, `image_uuid`, `unslid_load_address` and `text_size` from
   image 0 (a v1 image) and image 3 (the dual image). For each case it calls
   both `N.check_host` and `kotlin_guard` through `check()`.
   - Any exception is caught and turned into `self.fail`, so the bug shows up
     as an assertion failure rather than an error.
   - It requires a `system cache record: images` rejection.
2. **RED** ([logs/red-test_native_cache.txt](logs/red-test_native_cache.txt),
   `-O` the same): exactly 2 failures, the `install_path` cases at index 0 and
   index 3, each `AssertionError: raised KeyError instead of rejecting`.
   The other three keys already rejected without raising.
3. **Fix** (smallest validation-ordering change, [correction.diff](correction.diff)):
   - `_record_errors` keeps the `_image_errors` result.
   - For v2, `_dual_errors` runs only when the image identities are valid.
     Otherwise it fails closed with
     `dual_resident_images: not validated because images are invalid`.
   - `_dual_errors` then builds the canonical set from the already-validated
     images.
4. **After the fix** the probe returns, in both modes:

   ```
   ['system cache record: images: explicit list of exact identities required',
    'system cache record: dual_resident_images: not validated because images are invalid']
   ```

5. **GREEN** ([final-green/green-run-identity.json](final-green/green-run-identity.json)):
   all six suites pass, normal and `-O`, **124 tests per mode**, all OK.
   - Python 3.14.8, interpreter sha256 `5f923e62…`.
   - Sources were unchanged during the run and equal the successor.

   | Suite | Tests |
   | --- | --- |
   | `test_kotlin_guard` | 28 |
   | `test_symbol_guard` | 9 |
   | `test_embedded_scan` | 11 |
   | `test_init_script_guard` | 9 |
   | `test_java_properties` | 25 |
   | `test_native_cache` | 42 |
6. **AST audit** ([logs/ast-audit.json](logs/ast-audit.json)) of `native_cache.py`:
   0 asserts; imports only `dataclasses` and `re`; no `open`, `exec` or `eval`.

**README fix.** The old sentence "report every reviewed path as absent as a
file" contradicted version 2. It now states the rule per version:
- version 1: every reviewed path and alias is `absent`;
- version 2: each explicit `dual_resident_images` path is `regular`, and every
  other reviewed path and every alias is `absent`.

## Successor sources

| File | SHA256 |
| --- | --- |
| `native_cache.py` | `adb46c03…` |
| `test_native_cache.py` | `92919813…` |
| `README.md` | `08357ad5…` |

## Inherited issue found, reported and not fixed

This is outside this correction's scope. If an image's `install_path` is not
hashable (for example a list), `_image_errors` raises `TypeError` at
`len(set(paths))`. The same happens in version 1 and at HEAD (`native_cache.py`
before SR-22d), so it predates follow-up 9. It needs its own case.
