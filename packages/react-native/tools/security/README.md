# Restricted-build security guards (SPEC-0034 / TC-0038)

Pure-Python, offline, repo-owned checks. They read JAR bytes with `zipfile` and
a minimal class-file constant-pool parser; they never load or execute JAR code,
launch Gradle/compilers, use the network or spawn subprocesses. Every check is
explicit (no `assert`) and is tested under `python3 -I` and `python3 -I -O`.

API for the RN Android runner owner (wiring happens after handback):

- `kotlin_guard.secure_launch_args()` -> list of reviewed Gradle arguments.
- `kotlin_guard.check_launch(argv, env, properties)` -> `Result`.
  Rejects Kotlin incremental compilation, a non-pinned Kotlin execution
  strategy and `idea.use.native.compression` from every option source; it
  reports only source and setting names, never values. `properties` maps a
  label to the text or raw bytes of EVERY Gradle properties source the launch
  can read: consumer root, included-build root, the isolated
  `GRADLE_USER_HOME/gradle.properties` and the Gradle installation's
  `gradle.properties`. Parsing uses `java_properties.py`, written
  independently from the public `Properties.load(Reader)` API contract (no JDK
  source was ported; no legal clean-room certification is claimed): logical
  lines, `=`/`:`/whitespace separators, `\uXXXX` and other escapes, last value
  wins. Bytes decode as ISO-8859-1 and non-ASCII files are also checked as
  UTF-8; malformed escapes fail closed. Any `DYLD_*` environment variable
  rejects by name (SR-22c). This pre-launch check is required in addition to
  the authoritative observed-JVM check.
- `kotlin_guard.check_observed_jvms(observations, required_roles,
  native_inventory, native_baseline, *, system_cache=None)` -> gate result for
  the effective compiling JVMs (SR-20/21/22 evidence input). Each observed
  native library is classified exactly once:
  - by name and SHA256, as a reviewed embedded payload (`native_loads`) or a
    file-backed trusted JDK/platform baseline member (`baseline_loads`);
  - or, without a digest, as a reviewed macOS system shared-cache image
    (`system_cache_loads`).
  A name matching more than one class, a digest on a reviewed cache path, or a
  missing digest outside the cache record rejects. Unknown digest-less images
  are listed in `discovered_images` for review and keep the gate open; a
  capture never updates its own allowlist. Capture completeness is the
  caller's attested input and is never inferred. Empty `required_roles` is a
  vacuous certification and rejects. An observation without a non-empty
  effective `system_properties` map, an `input_arguments` list or an
  effective `environment` map is incomplete; any `DYLD_*` name in the
  effective environment rejects. Each `native_libraries_loaded` entry must be
  an object with a non-empty string `name`. Otherwise it rejects as an
  incomplete capture for that role (DEF-0031). The value is never echoed and
  never enters a load bin or `discovered_images`.
- `native_cache.SystemCacheEvidence(expected, pre_host, post_host)` -> the
  `system_cache` input (SR-22a/b). `expected` is the externally reviewed
  `hoyadb.system-shared-cache/1` record:
  - OS build, build ID and version-plist hash; host integrity observations;
    architecture; cache directory;
  - cache header values and main/subcache UUIDs, plus the complete cache-file
    SHA256 manifest (mandatory; no UUID-only fallback);
  - the reviewed slide alignment (no default) and the capture source;
  - an explicit image list of path, image UUID, unslid address and text size
    (no prefix, wildcard or whole-cache enrollment), and explicit reviewed
    aliases;
  - `mandatory_images`: a non-empty list of canonical `images` paths (never
    aliases), including `/usr/lib/libSystem.B.dylib`. Every observed JVM must
    report each of them, matched after reviewed-alias lookup, so a reviewed
    alias satisfies its canonical image. A JVM with zero cache images, or
    missing one mandatory image, is an incomplete capture.

  **Schema versions.** Version 1 (`hoyadb.system-shared-cache/1`) rejects
  every reviewed path that is a real file. Version 2
  (`hoyadb.system-shared-cache/2`, SR-22d) adds one required field,
  `dual_resident_images`:
  - a dict keyed only by canonical `images` paths (never aliases, prefixes or
    wildcards);
  - each value is exactly `{bytes: positive int, sha256: 64 lowercase hex}` of
    the on-disk file;
  - an empty dict grants no exception.

  **Version 2 host records** must also carry `dual_resident_files`. It must
  equal that dict type-strictly, and each of `pre_host` and `post_host` is
  compared with the record on its own. In `image_path_status`, each dual path
  must be `regular` (a non-symlink regular file); every other reviewed path
  and alias must stay `absent`.

  **Cross-version rejections:**
  - a version 1 host carrying `dual_resident_files` rejects;
  - a version 2 host without it rejects;
  - a version 1 record carrying the version 2 field rejects.

  **What the disk hashes mean.** They bind the on-disk copies only, not the
  mapped cache bytes; the complete cache-file manifest remains the cache
  integrity anchor. The on-disk/cache Mach-O UUID agreement is review-time
  evidence, not a runtime check.

  **Capture classification is unchanged:** a digest-bearing observation of a
  reviewed cache path rejects even when its digest equals the disk hash.
  `pre_host` and `post_host` are records the runner reads independently
  before and after the capture. They must equal the reviewed identity and
  re-read every reviewed image identity from the cache. Their path statuses
  depend on the version:
  - version 1: every reviewed path and alias is `absent` as a file;
  - version 2: each explicit `dual_resident_images` path is `regular`, and
    every other reviewed path and every alias is `absent`.

  They must also share the captured JVM's boot-session UUID. Within one
  JVM, every cached image's `base_address` minus its unslid address must give
  one common slide that is non-negative, below `max_slide` and a multiple of
  the alignment; `top_address` must be 0. These follow the studied JDK
  17.0.1+12 `jdk.NativeLibrary` semantics on macOS (Mach-O header address,
  top address 0). JFR does not report the mapped cache UUID, so tying a
  process to the rehashed host cache is an inference under the trusted host
  boundary, not process attestation.
  The inputs are trusted caller snapshots. The frozen evidence dataclass is
  shallow: its dicts stay mutable, so the caller binds the approved record
  identity and prevents concurrent mutation while a check runs.
  **Mandatory controller contract.** `system_cache` stays optional in this
  platform-neutral pure API. For the macOS restricted workflow, however, the
  controller MUST supply reviewed system-cache evidence, and its integration
  tests must show that omitting it rejects before any observation is accepted.
  Neither caller-attested completeness nor a file-only result substitutes for
  it. That wiring is a later controller task.
- `symbol_guard` -> absence and external-invocation evidence (SR-30).
- `embedded_scan.inventory(...)` / `embedded_scan.evaluate(...)` -> permanent
  embedded/shaded component gate (SR-31). Relocation prefixes are matched
  longest-first; `''` scans unrelocated packages and `org/jetbrains/` catches
  alternate roots such as `org/jetbrains/org/objectweb/asm`. Every relocated
  class under a third-party root is attributed to a package key (at most three
  segments). Unreadable or missing containers reject. Known blind spot: a
  relocation that drops the original third-party root (for example
  `org/jetbrains/kotlin/protobuf`) is only found through the explicit
  `namespace_map`; the heuristic cannot discover it.
- `init_script_guard.verify_pinned(path, approved)` -> the runner's pre-launch
  check: exact reviewed bytes (`R1_SCRIPT_SHA256`) and the reviewed structure
  (exact three constraints, scope, no override keywords; string-aware comment
  handling, unterminated input rejects). `check(path, approved)` is the
  structural part only. Neither is a general Gradle parser.
