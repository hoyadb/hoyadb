---
id: SPEC-0047
title: Complete approved engineering gaps after project remediation
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [ARC-0001]
satisfies: [REQ-0017, REQ-0043, REQ-0046, NFR-0006]
depends_on: [SPEC-0037, SPEC-0040, SPEC-0046, API-0011, API-0012]
tags: [review, implementation, compatibility]
---

# SPEC-0047: Approved engineering completion

## Authorization and scope

After receiving the 2026-10-30–31 remediation report, the operator requested
"Fix everything you can". Complete implementable remaining engineering work
under the existing accepted decisions and contracts. This authorizes fixes and
verification, not commits, publication, new dependencies, invented distribution
policy or operations on application-owned stores.

The starting state is the entire existing dirty tree on `035bd0f`, preserved in
`/tmp/hoyadb-followup-20261031/` before edits. Earlier implementation reports,
accepted ADRs, imported references and recorded evidence remain historical.
Record new outcomes separately; never recertify old binaries by copying them.

## Work packets and acceptance

1. Correct stale active custom-ID guidance using the already accepted ADR-0035
   decision, preserving the historical scenario and exact-ID recovery guards.
2. Enforce source/API/export compatibility checks required by API-0011. Keep
   historical baselines intact, inspect fresh Swift compiler/digester results,
   distinguish conservative source-drift checks from semantic compatibility,
   and fail on malformed evidence, failed tools or changed inputs. New approved
   surfaces require an explicit reviewed comparison, not silent rebaselining.
3. Implement the complete approved SPEC-0040 Stop/Discard feature in core,
   facade and bindings, using TC-0044 as the authoritative acceptance packet.
   Default Stop, truthful stored Discarded state, guard/error precedence,
   bounded atomic reports and callback/lifecycle semantics are mandatory.
   The approved old-binary refusal requires the real positive-control probe.
4. Complete SPEC-0037 Phase 3 R9–R11 in its stated order: case/counter proof,
   core transaction/session overlays, adapter overrides, format 3, then the
   reversible offline converter. ADR-0038 already authorizes this design;
   SPEC-0038 has landed. Characterize error order before changing planners,
   including the approved conflict-policy behavior once implemented. Preserve
   legacy-format behavior and strict open/reset validation. No user-store
   conversion is authorized.
5. Rebuild and exercise current packages, consumers and available platform
   evidence with the existing approved toolchains. Fix demonstrated failures;
   report unavailable runtimes or distribution decisions precisely. Performance,
   coverage and mutation evidence must bind to the final relevant source.

Reassess the factory startup-plus-cleanup double failure against its existing
contract. Do not invent an error API or an unbounded retry loop to hide it. A
contract-preserving correction requires its own reproduced failure and review.

## Workflow and completion

TC-0051 maps these packets to existing authoritative cases. Add meaningful
behavioral failures before implementations, using inert signatures only where
needed to execute a new-contract test. Compiler failures are not behavioral RED.
Review consequential packets with Claude Fable 5.1, resolve confirmed findings,
and run affected checks plus CONTRIBUTING's required source checks.

No distribution license, reporting address, release-support promise, registry
ownership or remote artifact endpoint is selected by this engineering task.
Unanswered policy decisions do not block independent engineering work.

Rollback restores only this task's changes against its captured dirty baseline.
Respect SPEC-0040's approved stored semantic break and SPEC-0037's explicit
conversion/backup rules; reverting code never authorizes deleting a store.
