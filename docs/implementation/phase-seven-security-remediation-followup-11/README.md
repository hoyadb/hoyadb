# Security remediation follow-up 11: DEF-0031 cache and capture input-type validation

This implements `docs/design/defects/DEF-0031-cache-input-type-validation.md`
(commit `04b8d99`), cases UP01–UP05.

**Scope:**
- `native_cache.py`, `kotlin_guard.py` and their two test files;
- one README sentence documenting the entry and name API contract.

Unchanged: the R1 pin and the other 11 sources.

What was not done:
- no OS, JVM, Gradle, helper, network, JAR or native payload;
- no enrollment, host reader or collector;
- no SR-32;
- nothing staged or committed.

All execution holds remain.

**Predecessor.** The follow-up 10 successor, which is Git HEAD `04b8d99`
(the `cf2fa3b` sources). All 16 sources were snapshotted before the edit
(`target/phase-seven-security-remediation/code-snapshots/pre-followup-11`) and
equal both. [successor-manifest.json](successor-manifest.json) binds
predecessor, RED, GREEN and successor.

## TDD

1. **Tests first.** These were written before any implementation change:
   - `test_native_cache.py`: class `InputTypes`, UP01–UP05;
   - `test_kotlin_guard.py`: `test_malformed_native_entries_reject_without_raising_or_echo`,
     the no-cache UP05 subset.

   Escaping exceptions are caught **only in the tests** and turned into
   `self.fail('raised <Type> instead of rejecting')`. A synthetic sentinel
   (`malformed-value-sentinel-91c4`) sits inside list and dict values to
   prove that nothing is echoed.
2. **RED** ([red/run-identity.json](red/run-identity.json)), normal and `-O`.
   The implementation hashes recorded at RED equal the predecessor
   (`native_cache` and `kotlin_guard` unchanged). There were no import,
   fixture or schema setup errors.

   | Suite | Tests | Subtest failures |
   | --- | --- | --- |
   | `test_native_cache` | 47 | 96 |
   | `test_kotlin_guard` | 29 | 6 |

   What the failures were:
   - 42 escaping exceptions converted to assertions (`raised TypeError` ×32,
     `raised AttributeError` ×10), from unhashable paths and names, a `None`
     arch concatenated into a filename, and non-object entries;
   - 2 cases where an empty `arch` manufactured a `cache_files` filename error
     (`True is not false`) in v1 and v2;
   - malformed names that were echoed or misclassified, for example
     `native library with missing digest outside reviewed system cache: ['malformed-value-sentinel-91c4']`,
     and names landing in `discovered_images`.

   UP03's controls already passed.
3. **Implementation.** Field validation only: no broad catch, and no new
   `try` blocks. The AST audit shows `kotlin_guard` keeps exactly its 3
   pre-existing `try` blocks and `native_cache` has none
   ([ast-audit.json](ast-audit.json)).
   - **`native_cache._record_errors`:** `arch` is validated as a non-empty
     string first, and `_file_errors`, which builds the filename, runs only
     when it is valid.
   - **`native_cache._image_errors`:** the duplicate check builds `set(paths)`
     only when every path is a string. Non-string paths already report
     `images: invalid identity`.
   - **`kotlin_guard.check_observed_jvms`:** before any lookup, a non-object
     entry reports `incomplete capture: native library entry is not an
     object`. A missing, empty or non-string name reports `incomplete
     capture: native library name missing or not a non-empty string`. Both
     are role-specific, `continue` without binning, and include no value.
4. **GREEN** ([green/run-identity.json](green/run-identity.json)): all six
   suites pass, normal and `-O`, **130 tests per mode**, all OK.
   - Python 3.14.8, interpreter sha256 `5f923e62…`.
   - The test files are byte-identical to RED. Sources were unchanged during
     the run and equal the successor.

   | Suite | Tests |
   | --- | --- |
   | `test_kotlin_guard` | 29 |
   | `test_symbol_guard` | 9 |
   | `test_embedded_scan` | 11 |
   | `test_init_script_guard` | 9 |
   | `test_java_properties` | 25 |
   | `test_native_cache` | 47 |

## Case coverage

| Case | Coverage |
| --- | --- |
| UP01 | v1 and v2 `install_path` `[]` / `{}`: `check_host` and the full guard return `system cache record: images…`, not clean |
| UP02 | v1 and v2 `install_path` `None`, `True`, 0, 1.5, list, dict: `images: invalid identity`, no echo, no coercion |
| UP03 | Valid v1 and v2 stay clean; duplicate string paths still give `images: duplicate install_path`; `''`, relative, wildcard and traversal paths still reject |
| UP04 | v1 and v2 `arch` `''`, `None`, `True`, 0, 1.5, list, dict: `system cache record: arch` from both `check_host` and the full guard; **no `cache_files` error is manufactured**, including for empty `arch` with every other field valid; no echo |
| UP05 | Full matrix (see below) |

**UP05 matrix:**
- with and without cache evidence;
- non-object entries (`None`, string, list, int, bool);
- names that are missing, `''`, `None`, `True`, 0, 1.5, list or dict, each
  × digest present or absent × `base_address` present or absent.

For every case:
- the role-specific identity is present;
- the sentinel is absent from `repr(result)`;
- `discovered_images` is `[]`;
- the valid payload, baseline and cache bins are exactly the valid
  libraries'.

The existing valid payload, baseline, cache and dual-resident classifications,
and discovery of unknown *valid string* paths, are unchanged.

## Files

| File | SHA256 |
| --- | --- |
| `native_cache.py` | `f9dc2834…` |
| `kotlin_guard.py` | `050fdf51…` |
| `test_native_cache.py` | `e759ad59…` |
| `test_kotlin_guard.py` | `fb7db3aa…` |
| `README.md` | `e592f15f…` |

The full diff against the snapshot is [changes.diff](changes.diff). The
runner is [bound_run.py.txt](bound_run.py.txt).
