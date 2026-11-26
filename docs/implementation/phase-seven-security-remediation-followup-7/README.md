# Security remediation follow-up 7: SR-22a/b/c macOS system shared-cache identities

This implements the approved SPEC-0034 / TC-0038 SR-22a/b/c contract (`acccf55`,
plus root's strengthening notes in `4ad628b`) as pure guards.

What was not done:
- No Gradle, compiler, JAR, JVM or native payload was executed.
- No capture or controller wiring was added; that is a later task.
- SR-32 was not started.
- Nothing is staged.

The predecessor cut is follow-up 5 (14 sources). It was snapshotted unchanged
before any edit, and [successor-manifest.json](successor-manifest.json) binds
the successor to it.

## Primary-source semantics

Root authorized these reads. All GETs are logged with URL, status, size and
SHA256 in [semantics-study/fetch-log.json](semantics-study/fetch-log.json).
The fetched sources stay local under the ignored
`target/phase-seven-security-remediation/native-baseline-study/sources/`.
This packet carries no source excerpts or translations: only the summary below
and line references.

**JDK native-library capture** (OpenJDK jdk17u, tag `jdk-17.0.1+12`).

*Which source the pinned JDK was built from.*
- The pinned Temurin `release` file names `SOURCE=.:git:571f1238bb46`. That is
  the Adoptium merge commit `571f1238…`, whose parent is the OpenJDK tag commit
  `75240a58…`.
- The four relevant files are byte-identical at the OpenJDK tag and at the
  Temurin commit. The SHA256s are in the fetch log.

*How it enumerates and reports libraries.*
- [os_bsd.cpp L1258–L1302](https://github.com/openjdk/jdk17u/blob/jdk-17.0.1%2B12/src/hotspot/os/bsd/os_bsd.cpp#L1258-L1302):
  `os::get_loaded_modules_info` has a link-map branch (L1259) and an Apple
  branch (L1292–L1299).
- The Apple branch walks dyld's image list **starting at index 1**, so index 0
  (normally the main executable) is skipped. For each image it reports:
  - the dyld image name;
  - the image's Mach-O header address as the base;
  - **0 as the top address**, because no module size is available.
- macOS does not provide `RTLD_DI_LINKMAP`. Consistent with that, the pinned
  `libjvm.dylib` imports `_dyld_image_count`, `_dyld_get_image_name` and
  `_dyld_get_image_header`, and has no `dlinfo` import.
- [jfrPeriodic.cpp L135–L148](https://github.com/openjdk/jdk17u/blob/jdk-17.0.1%2B12/src/hotspot/share/jfr/periodic/jfrPeriodic.cpp#L135-L148):
  the periodic `NativeLibrary` request copies those three values unchanged
  into one event per image.
- [metadata.xml L836–L840](https://github.com/openjdk/jdk17u/blob/jdk-17.0.1%2B12/src/hotspot/share/jfr/metadata/metadata.xml#L836-L840):
  the event is `jdk.NativeLibrary`, period `everyChunk`, with fields `name`,
  `baseAddress` and `topAddress`.
- [os.hpp L648–L655](https://github.com/openjdk/jdk17u/blob/jdk-17.0.1%2B12/src/hotspot/share/runtime/os.hpp#L648-L655)
  declares the callback.

The event carries **no cache UUID and no slide**.

**dyld.**
- The host's `/usr/lib/dyld` reports source version 1387. That version is not
  published; the newest published tag is `dyld-1378`.
- [dyld_cache_format.h L32–L116](https://github.com/apple-oss-distributions/dyld/blob/dyld-1378/include/mach-o/dyld_cache_format.h#L32-L116)
  (header):
  - The computed `sizeof` is 552, which equals the host cache's own
    `mappingOffset`.
  - Every offset my reader uses matches; there are 0 mismatches
    ([dyld-header-offset-check.json](semantics-study/dyld-header-offset-check.json)).
- The image info is at L157, the subcache entry at L581, and the image text
  info at
  [L218–L224](https://github.com/apple-oss-distributions/dyld/blob/dyld-1378/include/mach-o/dyld_cache_format.h#L218-L224).
  The text info's `loadAddress` is the unslid start of `__TEXT`.
- [DyldAPIs.cpp L227–L311](https://github.com/apple-oss-distributions/dyld/blob/dyld-1378/dyld/DyldAPIs.cpp#L227-L311):
  - the image header is the loader's load address and the name is the
    loader's path;
  - `_dyld_get_image_slide` returns the single cache slide for any image
    inside the cache.
- `normalizeImageIndex` (L238–L253): on x86_64 macOS, when dylibs are
  inserted, which image sits at index 0 depends on the main executable's
  deployment target. Only for old targets is index 0 remapped to the main
  executable.
  - The JDK's index-0 skip is only known to drop the main executable when
    `DYLD_INSERT_LIBRARIES` is absent.
  - The pinned `java` launcher carries the
    `com.apple.security.cs.allow-dyld-environment-variables` entitlement
    (`codesign` display, read-only), so `DYLD_*` variables are honored.
  - That makes the SR-22c rejection necessary, not cosmetic.
- [SharedCacheRuntime.cpp L1109–L1110, L1142](https://github.com/apple-oss-distributions/dyld/blob/dyld-1378/dyld/SharedCacheRuntime.cpp#L1109-L1142):
  dyld passes the main cache's `maxSlide` to the kernel, and the kernel
  chooses the slide.

**xnu** (the host runs 12377.161.15; the nearest published tag is
`xnu-12377.121.6`).
- [vm_unix.c L2036–L2048, L2275–L2299](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.121.6/bsd/vm/vm_unix.c#L2275-L2299):
  - the slide is a random value modulo `max_slide`, so it is strictly below
    `max_slide`;
  - it is masked with `~FOURK_PAGE_MASK` on x86_64 (`~SIXTEENK_PAGE_MASK`
    otherwise);
  - one value is applied to every cache file.
- [vm_shared_region.c L1544–L1560](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.121.6/osfmk/vm/vm_shared_region.c#L1544-L1560):
  a shared region accepts only one non-zero slide.

**Conclusion.** The official sources support the required address semantics:
observed base = unslid `__TEXT` address + one common slide per process, the
slide is non-negative, below `max_slide` and 4 KiB-aligned on x86_64, and
`topAddress` is 0. Nothing disproves them.

## API (pure; no host, cache or process access)

`native_cache.SystemCacheEvidence(expected, pre_host, post_host)` is passed as
`kotlin_guard.check_observed_jvms(..., system_cache=...)` (keyword-only; the
existing positional API is unchanged).

**`expected`.** The externally reviewed `hoyadb.system-shared-cache/1` record,
with exact keys:
- `os`: product version, build, build ID, version extra, and the
  `SystemVersion.plist` SHA256;
- `host_integrity`: SIP, authenticated root and sealed-volume observations;
- `arch` and `cache_dir`;
- `cache`: main UUID, the ordered subcache suffixes and UUIDs, and the header
  integers;
- `cache_files`: the **complete** name → bytes/SHA256 manifest. The main file
  and every subcache file are required, every digest must be 64 lowercase hex
  characters, and there is no UUID-only fallback;
- `slide_alignment`: a reviewed power of two with **no default**;
- `capture_source`: only the studied `mach_header`/`zero` semantics are
  accepted;
- `images`: an explicit list of path, image UUID, unslid address and text size;
- `aliases`: explicit, each pointing to a reviewed image.

**Images and aliases are rejected when they:**
- use a prefix, wildcard or relative path;
- are duplicates;
- try to enroll the cache's entire image set;
- are aliases that collide with an install path.

**`pre_host` / `post_host`.** Read independently by the runner before and after
the capture. Each must:
- equal the reviewed identity type-strictly (`1 ≠ True`);
- re-read every reviewed image identity from the cache;
- report every reviewed path and alias as `absent` as a file;
- share one `boot_session_uuid` with the other host record and with every
  captured JVM.

**Classification.** Each observed library is classified exactly once:
- by name and SHA256, as a payload (`native_loads`) or a file-backed baseline
  member (`baseline_loads`);
- or, without a digest, as a reviewed cache image (`system_cache_loads`).

These reject:
- a name that matches more than one class;
- a digest on a reviewed cache path (a real file cannot enter through the
  cache class);
- a missing digest outside the record, with or without cache evidence.

Unknown digest-less images are listed in `discovered_images` and keep the gate
open. The record is never modified, so a capture cannot enroll its own images.

**Per JVM.**
- The capture source must equal the reviewed capture source.
- `top_address` must be the integer 0.
- `base_address` must be an integer.
- base minus unslid gives one common slide that is non-negative, below
  `max_slide` and a multiple of the alignment. A different JVM may use a
  different slide.

**SR-22c.**
- `check_launch` rejects any `DYLD_*` environment variable before launch,
  reporting the name only.
- The effective `environment` map is now **mandatory** in every observation
  (otherwise the capture is incomplete), and any `DYLD_*` name in it rejects.

## TDD record (logs in [logs/](logs/); every run done normal and `-O`)

1. **RED 1:** `ModuleNotFoundError: No module named 'native_cache'`, from the
   new suite alone.
2. **RED 2:** the module exists but `kotlin_guard` is unwired. The run shows
   82 `TypeError: ... unexpected keyword argument 'system_cache'` and 6 DYLD
   launch failures (`True is not false`).
   - Three further errors (`KeyError`/`TypeError`) were fixture defects: the
     host fixture was built from a deliberately broken record. I fixed the
     tests to use an independently valid host record; the implementation was
     not touched for this.
3. **Wiring.** The existing `test_kotlin_guard` then failed 3 tests, because
   its observations lacked the now-mandatory environment. I updated the
   fixture with a secret-bearing environment and added a missing-environment
   regression test.
4. **RED 3** (self-review hardening): 5 failures.
   - `top_address` `False` and `0.0` were accepted;
   - `1`, `False` and `79340.0` compared equal to the reviewed `True`, `0` and
     `79340`.
   Fixed with type-strict comparison and non-empty UUID and suffix checks.
5. **GREEN** ([final-green/green-run-identity.json](final-green/green-run-identity.json)):
   all six suites pass, 102 tests per mode, and the sources were identical
   before and after the run.

   | Suite | Tests |
   | --- | --- |
   | `test_kotlin_guard` | 28 |
   | `test_symbol_guard` | 9 |
   | `test_embedded_scan` | 11 |
   | `test_init_script_guard` | 9 |
   | `test_java_properties` | 25 |
   | `test_native_cache` | 20 |

## Source changes ([changes.diff](changes.diff))

| File | Change |
| --- | --- |
| `native_cache.py`, `test_native_cache.py` | Added |
| `kotlin_guard.py` | Three-way exactly-once classification, `system_cache`, mandatory effective environment, DYLD_* rejection |
| `test_kotlin_guard.py` | Environment in the fixture; missing-environment test; stale comment corrected |
| `README.md` | New API documented; stale wording corrected |

The stale wording was "Parsing ports JDK17 `java.util.Properties.load`". It
now says the parser is `java_properties.py`, written independently from the
public `Properties.load(Reader)` contract, with no legal clean-room
certification claimed.

The other 9 sources and the R1 init-script pin `74aa545a…f9a0` are unchanged.

## Host observations

[semantics-study/observations.json](semantics-study/observations.json) records
the studied host:
- macOS 26.7.1, build 25G241, x86_64h;
- cache UUID `31374756-…` and nine cache-file SHA256s.

These are inputs for a reviewed record, not an approved record.

## Residuals

- **Version gaps.** The dyld and xnu sources come from the nearest
  *published* tags, not the host's builds. The header layout is checked
  directly against the host (`sizeof` equals `mappingOffset`); the slide logic
  is not.
- **Alignment macro.** The definition of `FOURK_PAGE_MASK` was not located in
  the 8 xnu headers fetched. The 4 KiB value rests on the macro name and the
  x86_64 branch, and the guard takes it as a reviewed input.
- **No mapping attestation.** JFR has no mapped-cache UUID. Linking a process
  to the rehashed cache is an inference under the trusted host boundary, not
  attestation.
- **No size evidence from the capture.** Because `topAddress` is 0, sizes come
  only from the reviewed text sizes.
- **Image-list completeness.** dyld documents image iteration as not
  thread-safe, so a concurrent load can be missed in one chunk. Capture
  completeness remains the runner's attested input.
- **First-capture review.** The reviewed image list can only be completed by
  reviewing a first bounded capture's `discovered_images`.
- **OS updates.** Any OS update changes the identity, and the gate then
  rejects.

## Indices

[files-sha256.json](files-sha256.json) indexes this directory. The predecessor
indices were left untouched.
