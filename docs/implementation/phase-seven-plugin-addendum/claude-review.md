# Claude Opus dependency approval checkpoint

Reviewed snapshot:079e821. This is source/data consistency review only; no final SDK/mobile/performance acceptance is inferred. The reviewer requested a nonblocking publisher-attestation follow-up, tracked separately in the packet.

Showing 1 of 330 activities (limited to 1)

## Plugin addendum (079e821) — sufficient for an approval decision, with one recommended correction

Answer to your question: **yes**, this packet gives an operator what they need to decide. The scope is drawn precisely, the delta is small and enumerated, the limitations are stated rather than glossed, and the claims I could check against the raw data all held. One provenance gap is worth closing first, but it is not a blocker for a build-time-only, offline, hash-compared activation.

### Claims I verified against the evidence, not just read

- **The six coordinates and four JAR hashes are consistent** across `README.md:11-18`, `bundle.json:6-54` and `additional-plugin-coordinates.json`. No drift between the human table and the machine record.
- **The "unchanged 87/87/2/145" claim is real.** `metadata/resolution.json` carries seven configurations whose component counts are 88/88/3/146/3/6/19 — each one element higher than the stated figure. I checked why: the first entry of every list is `{"id":"root project :","reason":["root"]}`, not a dependency. Net external components are therefore exactly 87/87/2/145, plus `foojaySettings` 2 and `ktfmtPlugin` 5. The baseline sets are untouched.
- **The plugin closure is exactly what the README says.** `ktfmtPlugin` resolves to 5 externals (marker, plugin, java-diff-utils, kotlin-stdlib 2.1.10, annotations 13.0) and `foojaySettings` to 2 (marker, foojay-resolver) — six new coordinates plus already-approved annotations 13.0, with nothing else selected.
- **The unapproved formatter is structurally segregated**, not buried in prose: `ktfmtFormatter` is its own named configuration (root + 18 components), matching `bundle.json:83-106`. Kotlin stdlib 2.0.0 and reflect 1.6.10 appear only there, never in the proposed set.
- **OSV coverage matches the claim.** `osv-query.json` holds exactly 7 queries — all six coordinates including both marker POMs, plus `com.google.code.gson:gson:2.13.1` — and `osv-result.json` records seven empty result objects. The README correctly calls this a registry check at retrieval, not a safety proof, and does not let the addendum overwrite ADR-0035's 47 disclosed build-only advisories.
- **Presentation normalization is handled honestly.** `presentation.json` records both the original (`303d2bf7…`) and readable (`b537469f…`) hashes with `parsed_json_equal: true`, and points at `metadata/resolution-original.json` for exact bytes — so the byte-identical online/offline claim is anchored to the unnormalized form, as you flagged.
- **`files-sha256.json` is a 32-entry self-integrity index** covering README and bundle themselves, so the packet is tamper-evident as a unit.

### Recommended correction (provenance; not blocking)

`README.md:36-38` states that java-diff-utils 4.15 "had no publisher SHA256 declaration available" and that these pins "are not independently authenticated publisher signatures." I extracted every URL from `archive-review.json` (63) and `repository-responses.json` (9). The fetches attempted were `.jar`, `-sources.jar`, `.pom`, `.module`, `.jar.sha256` and `.pom.sha256` on both Maven Central and `plugins.gradle.org`. **No `.asc` was requested for any of the four JARs, and no `.sha1` for java-diff-utils.**

Maven Central mandates a detached PGP signature for every release, and `.sha1` is present for every artifact including older ones that predate `.sha256` publication. So for the one JAR now resting on a self-computed pin, two publisher-side attestations exist and were not tried — and for all four, the `.asc` route is exactly the "independent authentication" the README identifies as missing. The gap is partly self-inflicted rather than unavailable.

**Smallest correction:** fetch `java-diff-utils-4.15.jar.sha1` and `java-diff-utils-4.15.jar.asc` (and the three other `.asc` files) from Central, verify against the published signing keys, and record the outcome in `archive-review.json`. This is metadata-only and executes nothing, so it fits this packet's own method. If a signature genuinely cannot be verified, saying so after attempting is materially stronger than the current disclosure.

### Smaller observations, no action needed

- The java-diff-utils `.jar.sha256` probes went to both Central and `plugins.gradle.org/m2`; recording those as attempted-and-absent is good practice and is what makes the gap above visible at all.
- The restriction "stop on any newly selected component" relies on offline resolution against a verified local repository, so a lazy ktfmt 0.54 resolution would fail the build rather than silently fetch. That is a hard stop in practice, not just a policy sentence.
- Kotlin 2.1.10 here is a build-plugin dependency and the app/SDK 2.1.20 pin is untouched — `kotlinSdkRuntime` still resolves to its 2 components, confirming no runtime bleed.

### Scope

Source and data only this pass: no builds, no network, no plugin execution, no repo edits, and I did not open any archive under `target/phase-seven-plugin-addendum/archive-review`. I verified internal consistency of the packet against itself; I did not independently re-fetch or re-hash any upstream artifact, so the recorded bytes are the runner's observation, not mine. This is the dependency-approval checkpoint only — full Phase 7 SDK, mobile and performance review remains pending.
