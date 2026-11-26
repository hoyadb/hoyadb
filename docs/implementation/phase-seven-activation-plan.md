# Phase 7 approved dependency activation preparation

ADR-0035 explicitly approves the exact packet and restricted build policy.
This step prepares checked archives and a scripts-disabled npm installation;
it does not certify a mobile build, execute dependency hooks or add a dependency.
The native SDK lane owns the eventual trusted consumer, Codegen and build.

Before activation, verify the approved npm manifest/lock against approval.json,
the lock's exact SHA256 and every resolved host. Use existing Node22/npm with
an owned target project and cache, empty task-local user/global npm config,
`npm ci --ignore-scripts --no-audit --no-fund`; retain all output, installed
inventory, exact tool versions and unchanged manifest/lock hashes. npm verifies
the frozen archive SRI; do not use npx, execute lifecycle hooks or run codegen
as part of this preparation. A failed/missing offline cache may fetch only the
same frozen registry.npmjs.org archive inputs over HTTPS. No package update.

For iOS, fetch only the six URLs already pinned in
[the approved packet](phase-seven-dependencies/README.md), each into its exact
owned target filename. Reuse a cached archive only after rehashing it. Require
matching SHA256 and SHA512 before any inventory or extraction. Record byte
counts and a tar inventory; reject absolute/parent-traversing paths, device
entries and escaping links. No install hook, Podfile evaluation, extraction,
source fallback or new archive is authorized by this preparation. A mismatch
stops; never overwrite a different cached file silently.

Acceptance cases: original reviewed lock/manifest stay byte-identical; installed
selected package identities match the frozen lock; all six actual archive bytes
match both strong pins; inventory cannot escape its root; no lifecycle hook,
compiler, Gradle plugin or native binary is executed. Retain failures rather
than relabel them as a successful actual consumer. The later native lane must
review actual build commands and selected AGP/CocoaPods graphs, use isolated
trusted builds with no remote/shared cache, and prove real target behavior.

Cleanup is limited to the exact owned staging project/cache and archives after
consumer handoff. Existing global caches, environment, user projects and stores
are unchanged. Missing runtime, build or source provenance remains unknown.
