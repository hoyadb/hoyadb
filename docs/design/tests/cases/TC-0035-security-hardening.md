---
id: TC-0035
title: Protected storage and diagnostics preserve confidentiality and recovery
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [TP-0001]
verifies: [SPEC-0032, API-0010, REQ-0015, REQ-0028, REQ-0030, REQ-0031, REQ-0032, REQ-0045, THR-0019]
tags: [security, security, encryption, conformance]
---

# TC-0035: Protected storage and diagnostics preserve confidentiality and recovery

These scenarios are required before implementation; they do not claim passing
evidence. TC-0012 maps S11–S14, TC-0015 maps S01–S10. Exact dependency approval
and security/contract review precede runtime changes.

```gherkin
Feature: Engine encryption fails closed below the generic storage interface
  Scenario: S01 protected default requires supplied exact raw key
    Given a fresh owned path and protected open with absent or malformed key
    When open is attempted
    Then it fails before creating or changing any file
    And no default generated stored derived or replacement application key exists

  Scenario: S02 named authenticated engine protects complete local contents
    Given a supplied 32-byte synthetic key and declared schema
    When rows commands identities frozen retries and field clocks commit
    Then runtime reports the reviewed SQLCipher version and authenticated v4 profile
    And keyed reads reproduce every exact value
    And raw main files reveal neither canaries nor key text nor plain SQLite header
    And an unkeyed independent SQLite reader cannot read schema or records

  Scenario: S03 wrong missing and legacy inputs preserve existing data
    Given a valid encrypted file and snapshots of its main and sidecars
    When opening with a wrong key missing key plaintext legacy file or foreign format
    Then there is no fallback initialization conversion reset or file mutation
    And reopening the original encrypted file with its correct key still succeeds

  Scenario: S04 tampered authenticated pages fail without partial delivery
    Given an encrypted synthetic file with known application and recovery records
    When a stored authenticated page is corrupted
    Then the engine detects failure and returns no invented partial result
    And no page checkpoint progress or pending recovery is committed on failure

  Scenario: S05 sidecars temporary operations and crash recovery retain protection
    Given encrypted rollback journal and native WAL cases and large temp operands
    When a transaction is interrupted at named precommit and postcommit barriers
    Then every persistent page containing domain data remains encrypted
    And temporary query or sort data is memory-only
    And real reopen preserves the specified atomic data and recovery state

  Scenario: S06 keyed adapters preserve shared behavior
    Given memory plaintext comparison fixtures and protected native or OPFS stores
    When shared typed query model reactive command entity pull and reset cases execute
    Then the same production semantics and errors hold without a shadow algorithm
    And encrypted factories use the same generic storage interfaces

  Scenario: S07 key and cipher state survive only through application configuration
    Given an encrypted store closed and independently reopened or page reloaded
    When the app supplies its original key again
    Then durable queues checkpoints schema and counters resume correctly
    And queue retry bytes contain no key and are not redacted for storage
    And unsupported rotation or conversion is not silently attempted

Feature: Real hosts describe platform controls truthfully
  Scenario: S08 native backup default opt-in and replacement are verified
    Given a protected Swift desktop store in its owned directory
    When default policy opens it or replaces its own file
    Then the Foundation backup-exclusion value is set and read back before writes
    When the app explicitly opts in
    Then the exclusion value reads false
    And a policy failure prevents protected DB creation without touching unrelated paths
    And invalid key name symlink or nondirectory components reject before filesystem changes
    And the existing canonical owned temp-root confinement is retained
    And replacement while open rejects and closed replacement revalidates policy before reopen

  Scenario: S09 browser backup capability is explicit
    Given an actual OPFS worker protected store
    When its protection status is read
    Then encryption is verified but backup exclusion reports unsupported
    And no persist request or origin-isolation claim substitutes for exclusion

  Scenario: S10 protected foreign opens do not downgrade or leak keys
    Given actual Swift C ABI and browser worker key-bearing requests
    When a key is invalid wrong or absent
    Then no plaintext owner or leaked error frame is produced
    And a correct-key reopen and real restart still works
    And the complete key is absent from files queue events logs and diagnostics

  Scenario: S10 pending protected and legacy opens reserve one client
    Given a host open RPC whose reply is suspended
    When a second protected or explicit legacy open targets the same client
    Then it rejects Busy before a second open RPC
    When close is requested before the first reply arrives
    Then close waits and the cancelled open never publishes an owner
    And the cancelled open rejects Closed
    And an acquired late handle receives exactly one close RPC
    And close resolves only after that disposal is acknowledged
    And a new open is Busy until cleanup completes
    And a later fresh open uses its new configuration without replaying the old key request

  Scenario: S10 failed opens and failed cleanup retain honest ownership
    Given close was requested during a suspended open
    When that open fails without acquiring a handle
    Then the cancelled open rejects Closed and close resolves without a fabricated close RPC
    When disposal of an acquired late handle instead fails
    Then close rejects and replacement opens remain Busy
    And a subsequent close may retry that retained handle
    And concurrent closes share an in-flight cleanup
    And a transport that never settles does not imply a cleanup completion guarantee

Feature: Identity diagnostics and egress boundaries stay explicit
  Scenario: S11 foreign context rejects reassignment through awaits
    Given a captured identity with nested objects and arrays
    When a JS handler attempts identity nested identity or context replacement
    Then assignment is rejected and the original capture survives an await
    And actual outgoing command identity remains exactly the caller-provided value

  Scenario: S12 diagnostics and automatic Debug use an exact bounded allowlist
    Given canaries in keys identity args row values outcomes reasons scope and checkpoint
    When diagnostics or owned context recovery transport and event Debug is requested
    Then only declared type variant count or capability fields are present
    And no canary identifier path namespace or checkpoint appears
    And complete local retry bytes and explicit data access remain unchanged

  Scenario: S13 library execution creates no hidden output or network
    Given captured stdout stderr and a caller-owned fake or HTTP transport
    When local commands entity writes reads retries pull and reset execute
    Then the library emits no sensitive logs telemetry or diagnostics
    And only explicit caller transport performs network activity
    And errors at worker and C ABI boundaries contain static kinds only

  Scenario: S14 regressions and reviewed provenance close the phase
    Given exact reviewed dependencies source and pre-runtime artifact hashes
    When prior phases and protected native browser process and backup cases execute
    Then all required formatting lint builds container and traceability gates pass
    And final bounded Claude Opus review addresses real reachable issues
    And unsupported platforms residual memory copies and OS backup limits are stated
```

No compromised-process, key entropy, guaranteed OS backup suppression, production
backend auth, multi-writer, mobile/new-platform or FIPS assertion is tested or
claimed here. Reusable prior assertions remain unchanged; explicit unprotected
legacy tests never count as encrypted conformance. Report each executed case,
actual source/artifact/profile/versions, observed failures and remaining gates.

## Verification status
