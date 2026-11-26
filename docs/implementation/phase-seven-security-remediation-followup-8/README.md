# Security remediation follow-up 8: mandatory system cache images (SR-22a)

**Reviewer gap.** In follow-up 7, a valid `system_cache` record plus a JVM that
reported zero cache images returned `ok=True`, which is a vacuous pass. This
slice implements root's approved contract (`3c3096d`, clarified in `09bf1f5`).
The reviewed record must name a non-empty canonical subset of its images,
including `/usr/lib/libSystem.B.dylib`. Every JVM must report that subset, and
matching happens after reviewed-alias lookup.

What was not done:
- No Gradle, compiler, JAR or JVM execution.
- No capture or controller wiring, no dependency activation, no SR-32.
- Nothing is staged.

The predecessor is the follow-up 7 successor (16 sources). It was snapshotted
unchanged before the edit, and [successor-manifest.json](successor-manifest.json)
binds the two. Follow-up 7 itself is unchanged.

## TDD record

1. **Behavioral RED, with the current record shape and fixture.** No schema
   field was added first. Four tests were added; logs:
   [logs/red-test_native_cache.txt](logs/red-test_native_cache.txt) and the
   `-O` log.

   | Test | Result on the unchanged implementation |
   | --- | --- |
   | `test_sr22a_zero_cache_images_is_incomplete` | fails: `True is not false` |
   | `test_sr22a_every_jvm_must_report_mandatory_images` | fails: `True is not false` |
   | `test_sr22a_non_mandatory_cache_image_alone_is_incomplete` | fails: `True is not false` |
   | `test_sr22a_reviewed_alias_satisfies_canonical_mandatory_image` | passes (positive control) |

   That is 3 failures out of 24 tests in both modes. Each failure is the
   guard returning `ok=True`; none is an import, keyword, fixture or schema
   error.
2. **Implementation** (`native_cache.py`):
   - new record key `mandatory_images` and constant
     `REQUIRED_MANDATORY_IMAGES = ('/usr/lib/libSystem.B.dylib',)`;
   - the record is valid only if the list is non-empty, has no duplicates,
     contains only canonical `images` paths (never aliases) and includes
     libSystem;
   - `check_capture`, which runs for every JVM when cache evidence is valid,
     requires each canonical mandatory path among that JVM's
     alias-resolved cache entries. Otherwise it reports
     `incomplete capture: mandatory system cache image missing: <path>`;
   - unknown or discovered images never count.

   The reviewed fixture gained `mandatory_images=[LIBSYSTEM]`. Four existing
   tests that observed only `libz` then needed `libSystem` at the same slide:
   - the repeated-chunk test;
   - the per-JVM slide test;
   - the Security alias test;
   - the slide-bounds positives.

   The slide negatives now also carry `libSystem` at the same bad slide, so
   only the slide rule can reject them.
3. **Negative controls** (all pass after implementation):
   - the record rejects `mandatory_images` that is absent, empty, `'all'`, a
     non-list, unregistered, an alias entry, duplicated, missing libSystem
     (`[LIBZ]`) or a wildcard;
   - with a valid two-image subset, a JVM missing `libz` rejects;
   - with three JVMs, each JVM in turn lacking libSystem rejects, and only
     that JVM is reported;
   - libSystem seen only under an unreviewed alias does not satisfy the set:
     it is reported as discovered and the gate stays open;
   - a reviewed alias does satisfy it.
4. **GREEN** ([final-green/green-run-identity.json](final-green/green-run-identity.json)):
   all six suites pass, normal and `-O`, 109 tests per mode, and the sources
   were identical before and after the run.

   | Suite | Tests |
   | --- | --- |
   | `test_kotlin_guard` | 28 |
   | `test_symbol_guard` | 9 |
   | `test_embedded_scan` | 11 |
   | `test_init_script_guard` | 9 |
   | `test_java_properties` | 25 |
   | `test_native_cache` | 27 |

## Source changes ([changes.diff](changes.diff))

| File | Change |
| --- | --- |
| `native_cache.py` | Mandatory subset; `SystemCacheEvidence` docstring |
| `test_native_cache.py` | New tests and fixture updates |
| `README.md` | Mandatory subset, trusted snapshots, controller contract |

The other 13 sources are unchanged, including `kotlin_guard.py` and the R1
init-script pin `74aa545a…f9a0`.

**Trusted snapshots.** No deep-freeze redesign. `SystemCacheEvidence` is a
shallow frozen dataclass whose dicts stay mutable. The inputs are trusted
caller snapshots: the caller binds the approved record identity and prevents
concurrent mutation while a check runs.

**Mandatory controller contract** (root decision i). `system_cache` stays
optional in the platform-neutral pure API. For the macOS restricted workflow,
the controller MUST supply reviewed system-cache evidence, and its integration
tests must reject omission before accepting observations. Neither
caller-attested completeness nor a file-only result substitutes for it. That
wiring is a later controller task.

## Corrected follow-up 7 chronology

Follow-up 7's README called RED1, RED2 and RED3 "RED". The accurate labels:

**Setup, API and fixture errors (not behavioral RED):**
- RED1 `ModuleNotFoundError: No module named 'native_cache'`;
- RED2's 82 `TypeError: ... unexpected keyword argument 'system_cache'`;
- RED2's 3 fixture errors (`KeyError`/`TypeError`).

**Genuine behavioral failures:**
- RED2's **6** DYLD launch assertions (`True is not false`, one per variable);
- RED3's **5** hardening failures: `top_address` `False`/`0.0`, and three
  type-loose host comparisons.

**Core cache matching was green first.** The host identity, complete-manifest,
image identity, slide, boot-session and capture-source checks passed on their
first behavioral run after the setup errors. They never had a behavioral RED.

Follow-up 7's frozen files are not rewritten; this record supersedes their
labelling.

**Retrospective strength check, done now (not a historical claim).** The
frozen follow-up 7 `test_native_cache.py` was run against two compiling stubs
of `native_cache` that keep the constants and the evidence class
([retrospective-stub/](retrospective-stub/), normal and `-O`):
- **Permissive stub** (no host or capture checks; exact lookup): 85 subtest
  failures. Every host, record, slide, boot, capture-source, alias, wildcard,
  UUID-only and type-strict test catches it.
- **Rejecting stub** (never classifies a cache image): 65 failures, including
  every positive test.
- **Gap:** no follow-up 7 test caught a JVM with zero cache images. That is
  the gap closed here.

## FOURK_PAGE_MASK (follow-up 7 residual closed)

The record is [semantics-study/fourk-page-mask-note.json](semantics-study/fourk-page-mask-note.json),
with every GET in [semantics-study/fetch-log.json](semantics-study/fetch-log.json).
The fetched source bodies stay local in ignored `target/`.

- **Definition.** At xnu tag `xnu-12377.121.6`,
  [osfmk/vm/vm_protos.h L186–L188](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.121.6/osfmk/vm/vm_protos.h#L186-L188)
  defines the 4 KiB page constants: size 0x1000, mask 0xFFF, shift 12.
  - The only guards open there are `XNU_KERNEL_PRIVATE` and the include
    guard.
  - The definitions sit outside both arm-specific blocks: L180–L184 (16 KiB
    constants) and the block starting at L190.
- **Use.** In
  [bsd/vm/vm_unix.c L2044–L2045](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.121.6/bsd/vm/vm_unix.c#L2044-L2048),
  the x86_64 branch masks the slide with the inverse of that mask, and L2281
  applies it to a random value modulo `max_slide`.
- **Include path.** `vm_unix.c` reaches the header through
  `vm/vm_protos_internal.h` and `vm/vm_page.h`.
- **Result.** The x86_64 slide is a multiple of 4096 and strictly below
  `max_slide`.
- **Search record.** 18 other headers at the same tag do not define it, and
  the installed SDK has no definition (it is a kernel-private macro).
- **Residual.** This is the nearest published tag, not the host build
  (12377.161.15). That is consistent with `slide_alignment` being a reviewed
  input.

## Residuals (unchanged from follow-up 7 unless noted)

- JFR has no mapped-cache UUID, so the mapping is an inference under the
  trusted host boundary.
- dyld and xnu come from the nearest published tags.
- Image-list completeness is the runner's attested input.
- The reviewed image and mandatory lists come from the first bounded capture's
  review.
- Any OS update makes the gate reject.

## Index

[files-sha256.json](files-sha256.json) indexes this directory. The predecessor
indices are unchanged.
