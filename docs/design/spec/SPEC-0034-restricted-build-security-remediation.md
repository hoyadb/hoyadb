---
id: SPEC-0034
title: Remediate restricted mobile build dependencies with exact verified graphs
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-26
parents: [ARC-0002]
satisfies: [REQ-0046]
depends_on: [ADR-0035, SPEC-0033, DEF-0028]
tags: [sdk, security, build-tooling, dependency-integrity]
---

# SPEC-0034: Restricted mobile build security remediation

## Required behavior

1. Keep official RN plugin sources unchanged. A pinned first-party init script
   applies upgrade-only constraints to the included plugin compile classpath
   using already-approved Guava33.3.1-jre and Gson2.11.0. Record the exact
   configuration scope and resulting graphs. No force-all rule, dependency
   exclusion, source patch or scan suppression substitutes for a compatible
   resolution. Earlier compiler-bundle identities remain historical; record
   precisely which old compile-only components disappear.
   This is an intent to stay inside approved identities, not a claim that
   Guava/Gson's new transitives are already approved. Verify the complete named
   configuration closure, including failureaccess, error_prone_annotations,
   j2objc-annotations, checker-qual, jsr305 and listenablefuture. Record both
   added and removed selections. Add scoped upgrade-only constraints to
   already-approved support versions where needed; any other selection stops
   and enters the separately reviewed delta before compilation.
2. For AGP-transitive findings, prepare a separate exact fixed-version graph
   with hashes, transitives, licenses, primary-source fixes and fresh advisory
   results. Candidate versions in the triage are unapproved inputs, not an
   activation allowlist. Include the newly observed Gradle bootstrap stdlib
   coordinate and missing parent POM in the review packet if needed. A new
   coordinate embedded in an approved tool is not silently treated as an
   approved selected module. Review and record the complete delta before any
   activation; honor ADR-0035's separate approval gate where required.
3. Disable Kotlin incremental compilation for both consumer and included build,
   use fresh task-owned build directories, and reject enabling
   idea.use.native.compression before launch. Check effective JVM/project
   settings without exposing unrelated environment values.
   Enumerate org.gradle.jvmargs, kotlin.daemon.jvmargs, GRADLE_OPTS, JAVA_OPTS,
   JAVA_TOOL_OPTIONS, _JAVA_OPTIONS and JDK_JAVA_OPTIONS plus explicit launch
   arguments and project properties. Each inherited option source has its own
   enabling-property rejection control; a project-file-only check is inadequate.
   Include ORG_GRADLE_PROJECT_ environment properties, -Dorg.gradle.project.
   properties and nested daemon-option values. Verify the effective arguments
   and system properties of every observed compiling JVM as the authoritative
   check; an unobserved JVM or unsafe effective setting keeps the gate open.
   Verify that no remaining classpath-snapshot/storage path reaches the vulnerable embedded
   LZ4 entry points in the selected workflow. A property name or missing cache
   directory alone is not proof; if a reachable path remains, keep the gate
   open and propose the smallest compatible correction.
   Pin and record the Kotlin execution strategy and every compiling JVM being
   observed. Use existing JDK observation facilities such as class-load logging
   or JFR, not a newly downloaded agent. The path-enabling control must prove
   the observation can detect the affected code in that process. Observe Jansi
   native loading in the same bounded build and record the actual outcome
   against the known embedded native inventory; do not silently drop the
   earlier unverified-load residual or label native loading itself a vulnerability.
   Native loading requires native-library-level observation with the pinned
   JDK's verified facilities; class-load logging alone is an incomplete capture.
   Classify every observed library exactly once as a reviewed embedded payload,
   a file-backed JDK/platform baseline member, or an explicitly reviewed macOS
   system shared-cache image. File-backed members retain exact SHA256 matching;
   a missing digest is never a general exemption. Version1 rejects every
   regular file in the shared-cache class. The separately specified Version2
   case below permits only explicitly reviewed dual-resident images;
   ordinary file-backed libraries retain their existing checks.
   For shared-cache images, compare a separately supplied expected host/cache
   record with independently collected current host evidence: exact OS build,
   build identifier and version-plist hash, architecture, cache directory,
   main/subcache UUIDs and complete cache-file SHA256 inventory. Record host
   integrity observations and reject a changed or incomplete identity. No
   UUID-only fallback replaces the complete hashes. The image allowlist names
   only explicitly reviewed observed paths, each bound to its image UUID,
   unslid address and text size from that exact cache. No path prefix, wildcard,
   or automatic acceptance of the cache's entire image set is allowed.
   Pin the capture source and its address semantics against the reviewed JDK.
   Match each cached image's observed address to one common, page-aligned slide
   per JVM within the reviewed cache bounds; reject missing addresses, mixed
   slides, unknown paths and aliases not explicitly reviewed. Reject DYLD_*
   environment settings before launch. Report system-cache matches separately
   from payload and file-baseline matches. JFR supplies no mapped-cache UUID:
   association with the rehashed host cache is an inference under the trusted
   host boundary, not direct process attestation. Preserve that limitation.
   Bind independently collected pre-capture and post-capture host evidence and
   the captured JVM to the same boot-session UUID. Require the cache's recorded
   architecture-specific mapping alignment for slide checks (4 KiB for the
   studied x86_64h cache); do not assume one alignment for unverified targets.
   Full cache-file hashes are the integrity anchor: host seal/signature flags
   are observations, not a substitute, because this cache resides on Preboot
   and is ad-hoc signed rather than covered by the system snapshot seal.
   The reviewed record must name a non-empty mandatory-image subset of its
   exact image allowlist, including `/usr/lib/libSystem.B.dylib` for this
   studied macOS workflow. Every observed JVM must report that subset; zero
   cache images or a missing mandatory image is an incomplete capture, not
   a vacuous successful slide check. Mandatory entries are canonical paths in
   `images`, never aliases; match observations after reviewed-alias lookup so
   an explicitly reviewed alias satisfies its canonical mandatory image.
   Input dictionaries remain trusted caller
   snapshots: a shallow-frozen container is not a deep-immutability guarantee,
   and the caller must bind approval identities and prevent concurrent mutation.
   Discovery of a new image produces evidence for review and keeps the gate
   open; a capture cannot enroll its own images and certify itself.

   **SR-22d proposed record successor.** SR-23 observed three system images
   both as regular on-disk files and at addresses consistent with the studied
   cache layout. Keep `hoyadb.system-shared-cache/1` behavior unchanged. Add
   `hoyadb.system-shared-cache/2` with one additional required field,
   `dual_resident_images`: a dictionary keyed only by canonical paths already
   in `images`, never aliases, prefixes or wildcards. Each value has exactly
   `bytes` (positive non-Boolean integer) and `sha256` (64 lowercase hex
   characters), describing the on-disk file only. An empty dictionary grants
   no exception. No record or image is approved by adding schema support.

   A Version2 pre/post host record additionally requires
   `dual_resident_files`, with exactly that dictionary's paths and independently
   re-read file sizes and hashes. `image_path_status` must report each such
   canonical path as a non-symlink regular file (`regular`); all other reviewed paths and aliases must
   still report `absent`. A symlink, missing or changed file, extra host entry,
   unregistered path or incomplete observation rejects. Keep exact field sets
   and type-strict comparisons for both versions. A Version1 record cannot
   acquire the exception by carrying Version2 fields or a new schema label.
   A Version1 host record carrying `dual_resident_files` rejects, and each
   Version2 host record missing that field rejects. Pre and post must each
   match the reviewed record independently, not merely match one another.
   Before approving any concrete dual-resident entry, record-review evidence
   must show the on-disk architecture's Mach-O UUID agrees with its cache image
   UUID. That is review-time evidence; runtime checks bind the disk bytes by
   size and SHA256 rather than adding a Mach-O parser to this pure guard.

   Capture classification is unchanged: an observed cache entry carries no
   file digest and must satisfy the cache image identity, mandatory-image and
   common bounded aligned-slide rules. A digest-bearing observation claiming
   a cache path still rejects, even if its digest equals the dual-resident
   file's hash. Host-side file hashes bind the on-disk copies; they do not
   prove the bytes of the mapped cache images. The complete cache-file hash
   inventory remains the cache integrity anchor. Preserve the trusted-host
   inference limitation, exactly-once classification and every existing
   rejection for unlisted real files. This change does not assert complete
   native lifetime observation, supply effective runtime properties, authorize
   enrollment or release an execution hold.
4. Classify findings from the actual bytes and call paths. KAPT/log4j absence
   guards must identify advisory-affected symbols, with synthetic negative
   controls. For protobuf and shaded commons-lang3, inspect the actual fork,
   advisory preconditions and input boundaries before claiming applicability
   or non-applicability. No generic residual-risk acceptance is inferred from
   the operator's request to fix issues. Unresolved applicable issues stay open.
5. Add a permanent embedded/shaded inventory check to the dependency gate.
   Include pom.properties and package-specific source-declared versions with
   distinct confidence labels; do not equate declarations with byte proof.
   Detect the known shaded commons-lang3 case and reject an untriaged new
   advisory/component. Unknown versions or unavailable scans are explicit,
   never silently classified as having no advisories.
6. DEF-0028's offline assembler verifies exact approved inputs and complete
   output before any graph/build consumer. Keep supplied JDK/AAPT2 hashes,
   isolated Gradle homes, no remote/shared cache, offline/outbound-denied
   execution, disabled tool downloads and no source fallback. Preserve all
   failed attempts and do not touch unrelated processes or caches.
7. Once security validation and exact graph gates pass, prove the unchanged
   official plugin compiles and the real RN Android app consumes the certified
   AAR. Exercise debug-keystore generation and runtime/restart/parity cases.
   Compare the app/SDK runtime coordinates and bytes against approved pins;
   no build-only remediation may silently alter the shipped runtime graph.

## Verification, reporting and rollback

Before implementing the observation collector, run TC-0038 SR-23's bounded
facility probe with the already approved pinned JDK. It characterizes capture
formats and limitations only. It cannot approve a native baseline, accept an
advisory, activate a dependency, release the compiler hold or count as SR-21/22
evidence. In particular, an end-of-chunk native-library snapshot alone cannot
establish the absence of a library that loaded and unloaded during the run.
Keep native-load completeness unresolved unless a verified load-time facility
covers that gap. Keep process linkage and observed compiler-strategy derivation
for separately reviewed collector cases after this probe.

TC-0038 enumerates cases before implementation. Bounded security probes may
use only reviewed tools and synthetic inputs after prep review; they do not
constitute final app acceptance or authorize unknown dependency execution.
Every claimed fix needs genuine RED/GREEN or an explicitly described existing
guard with negative control. Compatibility is established by actual build/run
evidence, not a metadata mirror. Scanner results, unsupported targets and
unresolved findings remain visible. No final clean review round is counted
until matching-artifact platform SDK gates are complete.

Rollback reverts unpublished constraints/control scripts and derived outputs,
preserving previous manifests, immutable input archives and failure evidence.
No user store, published package, accepted decision body or global environment
configuration changes are authorized by this slice.
