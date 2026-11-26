# Phase 7 observation-contract decision proposal

Status: **PROPOSED; operator decision and security review required. No gate changed.**
Updated: 2026-10-26

This proposal concerns only the evidence contract for the restricted local
build in [SPEC-0034](../../design/spec/SPEC-0034-restricted-build-security-remediation.md)
and [TC-0038](../../design/tests/cases/TC-0038-restricted-build-security.md).
It is not approval to execute a compiler, activate dependencies, enroll a
baseline, accept the open protobuf advisories, or count a final Phase 7 round.

## Decision

There are two separate operator choices:

1. **D1: adopt Option A's changed evidence contract, recommended for review.**
   Replace the authoritative runtime-property claim with startup-source checks
   and a justified exclusion of the affected LZ4 execution. Scope native
   evidence to verified Java-initiated load routes plus mapped-image snapshots.
   This changes the required assurance; it is not merely a parser fix.
2. **D2: explicitly accept or decline the scoped native-lifetime residual for
   this bounded build.** D1 approval does not imply D2 acceptance. Without
   explicit D2 acceptance, SR-22 remains open even if D1 is approved and its
   checks pass. Record both decision outcomes separately.

**Option B:** retain the stronger assurances and separately review first-party
instrumentation with demonstrated coverage. No complete mechanism has been
established by the reviewed evidence. Authorizing design investigation would
not establish that such instrumentation can meet the gate.

Without an explicit decision, the current requirements and holds remain.
CHG-4 in the pinned governance [change rules](../../../ai-governance/rules/04-changes.md)
requires operator and security review for this security-contract change.

## Exact proposed assurance under A

| Area | Required evidence | Guarantee not supplied |
| --- | --- | --- |
| Startup settings | Constructed environment and exact launch inputs; each option channel excluded and checked absent, or verified as represented in captured JVM data | General completeness of every JVM argument source is not inferred from the version probe |
| LZ4 exclusion | No affected guarded class loads in any compiling process, after all copies, runtime class origins and logging coverage are independently justified | Runtime property values throughout execution are not observed |
| Native behavior | Pinned-JDK `-Xlog:library` records (`library=trace` in the probe, successful-load/unload messages at `info` from `JVM_LoadLibrary`/`JVM_UnloadLibrary`): only successful Java-initiated non-builtin loads after logging starts, for statically verified routes; separately, JFR `jdk.NativeLibrary` snapshots. Classify every observed member against exact reviewed bytes or cache identities | Bootstrap loads before logging starts (including libjli/libjvm), direct native loading, dependent images and loads that disappear between snapshots may remain unobserved |
| Compilation identity | Actual process linkage, PID/main/class-source evidence, task execution/completion and verified dispatcher behavior | Requested strategy or loaded compiler classes alone do not prove actual compilation or its strategy |

The changed property guarantee loses detection of forbidden runtime property
changes that never lead to the excluded LZ4 behavior, whether persistent or
transient. The proposed effect evidence addresses that LZ4 consequence only;
it is not a general substitute for effective-property observation.

The native gap requires its own explicit disposition. Without acceptance of
that gap for this bounded build, SR-22 stays open. A snapshot must never be
converted into a native-lifetime completeness attestation. Structural JFR
completeness (finished chunks and final flag) remains a separate valid check.

## Conditions that remain mandatory

- The artifact-producing build used for RN Android acceptance and the final
  reviews must itself be captured and evaluated. A matching configuration
  from an earlier observed build is insufficient.
- Every affected class copy must be covered on the final selected inputs,
  including the pinned Gradle distribution and any separately approved delta.
  Runtime transformed/cache sources must map back to reviewed inputs. An
  unexpected affected-prefix load keeps SR-21 open; loading alone does not
  prove invocation and does not justify weakening the rule.
- A native-load absence claim requires a positive successful Java-initiated
  non-builtin JDK load after logging starts in the same PID's `-Xlog:library`
  log. A `jdk.NativeLibrary` snapshot cannot satisfy this control. The positive
  record confirms the log is active; it does not extend coverage to unverified
  loading routes or pre-logging bootstrap loads.
- Unobserved compiling JVMs, incomplete process linkage, missing required
  capture data, unknown libraries or class sources, and host drift keep the
  applicable gate open. ProcessStart is not a universal OS process census.
- Preserve exact native payload and JDK file hashes, Version2 dual-resident
  host checks, complete cache-file hashes, mandatory libSystem, the DYLD_*
  restriction, separate baseline approval and rejection of self-enrollment.
- Report SR-20, SR-21 and SR-22 separately. An unresolved finding must remain
  visible and must not be converted into an overall security PASS.
- Exact dependency approval, advisory disposition and bounded execution
  approval remain separate. This proposal accepts no protobuf finding or
  unknown component, and authorizes no build or positive-control execution.

## Named cases to formalize before implementation

These are proposed case obligations, not implemented tests or accepted changes
to TC-0038. If A is approved, review and commit the exact formal specification
and case delta before changing the guards or collector.

| Case | Expected behavior |
| --- | --- |
| OC01 | Startup or callback property data is never labelled as lifetime effective-property evidence |
| OC02 | Snapshot-only native evidence never produces a native-lifetime completeness result; structural JFR completeness stays distinct |
| OC03 | An unsafe startup option rejects through every supported channel; an unverified channel rejects or remains explicitly unresolved |
| OC04 | A missing or unreviewed affected class copy/source prevents an LZ4 exclusion claim |
| OC05 | Missing child-JVM capture or process/compile evidence rejects; strategy is never copied from the request |
| OC06 | A native absence claim without a same-PID successful Java-initiated non-builtin JDK load after logging starts in `-Xlog:library` is unsupported; a JFR snapshot cannot substitute, and pre-logging bootstrap loads remain outside this log |
| OC07 | Unknown, changed, deleted or otherwise unbound native payloads cannot be certified by pathname alone |
| OC08 | Missing macOS cache evidence, host drift or a self-enrolled candidate rejects |
| OC09 | A successful earlier capture cannot certify an unobserved artifact-producing build |
| OC10 | Separately approved real affected-path and process/strategy controls demonstrate detection in the relevant actual process; synthetic tests alone do not establish this |
| OC11 | Record D1 and D2 separately; the changed property assurance and scoped native residual remain explicit in each applicable result and final acceptance record, and absent or declined D2 acceptance leaves SR-22 open |

## Minimum subsequent work

One bounded static pass can proceed without changing the contract: inventory
all final-input affected classes and native payloads, verify loader/extraction
routes (including the unverified lz4-java path) and actual compiler dispatch,
and enumerate startup-option sources.
Unresolved routes are reported rather than followed by another facility probe.
Property-interception research is conditional on a claim actually needing it.

After a contract decision, review the smallest collector and its cases, then
use pure fake-based TDD. No property snapshot observer or additional version-only,
JDK-only or os=info probe is part of the recommended minimum. Reopen in-process
observation only for a necessary named datum that other evidence cannot supply.

After the separate gates permit execution, plan final-workflow discovery,
independent baseline review, an artifact-producing verification build, and
bounded affected-path and process/strategy controls. Discovery cannot certify
its candidates. A reviewer-agreed campaign budget limits repeated attempts;
exhausting it leaves the gate unresolved rather than producing acceptance.

## Evidence and committee resolution

The [committed probe review](../phase-seven-jdk-capture-probe-review.json)
records startup-property, native-lifetime, process and strategy limitations.
The [original probe](../phase-seven-jdk-capture-probe/README.md) remains frozen.
The input hashes and committee message identities are in
[evidence.json](evidence.json).

Sol initially allowed a callback observer under a bounded property contract.
Opus recommended omitting it. They converged on omitting it under A because
no necessary decision datum currently requires it. They also agreed that
static prep can proceed while approval is pending and that pure evaluator
tests can preserve known limitations without another real mutation probe.
Opus withdrew broader claims about exclusive native loading routes, universal
startup-option reporting, risk-free requirement changes and the sufficiency
of synthetic strategy tests. Newly reported Gradle-native and additional
compiler-copy findings remain candidates for the static pass, not independently
approved inventory entries.

DEF-0031 is separately committed as `d0439b9`; its 130 passing tests per Python
mode do not establish runtime observation. Zero final Phase 7 rounds count.
