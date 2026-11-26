# Approved mobile dependency preparation — observed result

The [reviewed plan](../phase-seven-activation-plan.md) ran after ADR-0035 approval.
No mobile build or runtime support is inferred from this preparation.

Existing Node22.18.0/npm11.17.0 installed exactly477 frozen npm package locations
with lifecycle scripts disabled, no audit and no package updates. Every selected
version, resolved URL and SRI matches the approved lock; each installed package
manifest reports the expected version. A second `npm ci --offline` with the same
scripts-disabled arguments passed. Both source manifest and lock stayed byte
identical. [Exact commands/results](npm-result.json) and the
[installed inventory](npm-installed-inventory.json) retain the observations.
The npm update notice was informational; no update command was run.

All six approved iOS archives were fetched or rehashed. Their actual217,235,588
combined bytes match both SHA256 and SHA512 pins. Pure inventories found only
admitted file/directory/link entries with safe relative paths and nonescaping
links under the declared bounds. Nothing was extracted or executed. The first
Python urllib GET returned403 and accepted no archive; the existing curl client
retried the identical approved HTTPS URL successfully. No source fallback,
substituted container or relaxed hash was used. [Per-container evidence](ios-verified.json)
records exact bytes, hashes and retained inventory locations.

Prepared files remain in ignored `target/phase-seven-activation/`:
`npm/` is the scripts-disabled installed project, `npm-cache/` its isolated
archive cache and `ios/` the six checked containers plus inventories. This is a
handoff to the native consumer lane, not its eventual external consumer layout.
Actual Codegen/Gradle/Pod commands and selected graphs still need their source
review and restricted execution policy. All47 disclosed build-only advisory
matches remain accepted risks, not patched or proven unreachable.

No global cache, toolchain, user store or project was changed. No dependency
hook, native binary, plugin or compiler from this npm project was executed.
No package was published, pushed or tagged.
