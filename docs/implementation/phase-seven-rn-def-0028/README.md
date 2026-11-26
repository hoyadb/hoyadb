# DEF-0028: optimized repository input validation — implementation evidence

This records the implementation evidence for DEF-0028
(`docs/design/defects/DEF-0028-optimized-repository-input-validation.md`). The
corrected preparation was approved at 24ba5d4, and the location amendment at
a759cd6 chose Option B. Nothing is staged or committed, and the compiler hold
is unchanged.

## History, stated honestly

The experimental `target/` assembler script was corrected before this formal
defect existed. That correction is not RED-first.

The exact script bytes behind the 34/633 incident were not preserved. The
separate replay below uses the frozen pre-addendum v1 script (SHA256
`f76c2a5c…`) and demonstrates the same mechanism with its own synthetic counts.
It is not the 34/633 incident.

## Permanent tool and regression

The tool lives outside the shipped RN package (`package.json` `"files"` is
unchanged):

- `bindings/react-native/tools/android/prepare_repo.py`: `assemble` and
  `verify`, each with `--inputs PLAN --out DIR --record FILE`. Every check is an
  explicit raise. There is no network or subprocess, and importing the module
  has no side effects. Collection is separate from validation. Provenance
  conflicts and byte conflicts are distinct rejections, and a missing input is
  a local rejection. Output goes to a `.partial-*` directory, is rechecked, then
  atomically renamed, and only after that is the success record written.
- `bindings/react-native/tools/android/test_prepare_repo.py`: stdlib unittest
  with synthetic fake bytes in fresh temporary roots. The actual tool runs as a
  subprocess with `-I` and with `-I -O`.
- `bindings/react-native/tools/android/testdata/prepare-repo-v1.py.txt`: the
  frozen v1 copy, SHA256 `f76c2a5c…`, checked explicitly.

Commands:

```text
python3 -I bindings/react-native/tools/android/test_prepare_repo.py
python3 -I -O bindings/react-native/tools/android/test_prepare_repo.py
```

## TDD chronology ([tdd/](tdd/), source hashes recorded per step)

1. **red-1 (harness error, not counted as RED).** The suite against the
   compiling stub had 14 errors: `assembled()` passed the fixture name where the
   interpreter mode belonged, which raised `KeyError`. Only the test helper was
   fixed.
2. **red-2 (meaningful RED, both modes).** The stub run had 0 errors and 31
   assertion failures, all `REJECTED not-implemented`. The static checks and the
   v1 replay ran independently.
3. **green-1 (one test-design failure).** R01 compared plans written into two
   separately located input fixtures, whose absolute roots made the plan SHA256
   differ. The test was changed to one read-only input plan with separate fresh
   output roots per mode. The tool was unchanged.
4. **green (both modes).** 16/16 OK under `-I` (optimize 0) and `-I -O`
   (optimize 1), with tool `0dbe70ee…` and tests `bed30179…`.

The final tool's coverage:

| Case | Coverage |
| ---- | -------- |
| R01 | Identical repository bytes and byte-identical success records in both modes; verification passes |
| R02 | A corrupted binary, and separately corrupted metadata, are rejected (`integrity`) with no record and no published repository |
| R02b | A missing binary, and separately missing metadata, are rejected locally (`missing-input`); a static check confirms no network or subprocess import |
| R03 | The same path with a different provenance identity is rejected (`provenance-conflict`); different bytes for the same destination are rejected (`byte-conflict`) |
| R04 | Verification rejects a missing record, a stale record, a tampered record, an unexpected file, a missing file, a mutated byte, a missing repository and a partial copy |
| Extra | Absolute, escaping and backslash output paths, escaping source paths and source symlinks leaving their root (`unsafe-path`); no assert statements; import has no side effects |
| v1 mechanism replay | Normal counts are binaries 1, metadata 1. `-O` counts are binaries 1, metadata 0. A corrupted binary makes the normal run fail with `sha256 mismatch`, while the `-O` run exits 0 and accepts it |

## Real 633/241 assembly ([integration/](integration/))

The trusted controller `target/phase-seven-rn/android/make-plan.py`
([copy](controller/make-plan.py.txt), SHA256 `c656e93f…`) derives the plan only
from the frozen approval records:

- the ADR-0035 index with verified local bytes;
- 8a87389;
- compiler bundle `364a4201` and index `2182049f`, both checked;
- root's provenance-checked metadata.

When an approval record has no size, the size comes from the verified local
bytes; the approved hash still decides integrity. The plan has 874 entries
(241 binaries, 633 metadata), with plan SHA256 `9ff3ff84…`.

Results:

- The normal and `-O` assemblies are byte-identical repositories. Their success
  records are byte-identical (`bc826ac7…`), with inventory SHA256 `71d65770…`.
- That repository is byte-identical to the earlier valid explicit-check
  repository (874 files).
- `verify` passes in both modes.
- One `-O` attempt failed only on shell quoting (`"-I -O"` passed as a single
  argument) and never ran the tool; it is preserved.

## Controller ordering ([controller/](controller/))

This is integration evidence only; the permanent suite does not prove the
untracked controller's ordering. The controller
`target/phase-seven-rn/android/build.py` ([copy](controller/build.py.txt))
loads the permanent tool by path and calls `verify_repository` before any
subprocess.

| Run | Result | Steps run |
| --- | ------ | --------- |
| Valid repository, `verify-only` | verified | 0 |
| `repo-INVALID-O-run` | `missing-file` | 0 |
| A copy with one mutated byte | `integrity` | 0 |
| A missing record | `missing-record` | 0 |

The three rejection runs used `included-graph` mode, so Gradle would have run
had verification passed.

## Presentation

Raw logs that fail whitespace checks are stored as exact-byte JSON wrappers
(`*.txt.json`, which record the original bytes and SHA256).
