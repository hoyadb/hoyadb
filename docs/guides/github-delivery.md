# GitHub delivery

Scope: SPEC-0050 / TC-0054. Destination: `hoyadb/hoyadb`. Workflows are preparation,
not evidence of passing hosted jobs, configured accounts or mobile conformance.

## Implementation

[CI](../../.github/workflows/ci.yml) runs on PRs, main, version tags, merge queues
and manual requests. It retains the [GitLab](../../.gitlab-ci.yml) verification
commands, fetches locked graphs before offline execution, builds ordinary WASM
before JavaScript tests, then validates installed client/testing tarballs. Each
candidate records checksums and its source revision. Coverage/mutation evidence
is uploaded even when validation fails. `CI / Required checks` accepts only
successful prerequisite jobs; coverage floors and mutation scope are unchanged.

[Release](../../.github/workflows/release.yml) is dispatched **from main** with
an existing strict `vMAJOR.MINOR.PATCH` tag. Preflight checks main ancestry,
versions, MIT/public metadata, dependencies and GitHub provenance identity. It
resolves the tag once; CI and native builds use that exact commit. Start with
`publish: false` to produce candidates. The [native recipe](../../tools/ci/native.sh)
uses the existing Apple builder/validators, Swift tests/API comparison, reviewed
Android provider/JNI/class builders, final AAR certification and RN packer.
Stale source/provider/notice evidence remains a failure, never an override.

With `publish: true`, separate environments govern Central, GitHub Releases and
npm. Central artifacts get POM metadata, sources, real usage documentation,
notices, signatures and checksums. Downstream publication waits for Central's
`PUBLISHED` state. GitHub Releases receives the Apple XCFramework ZIP and build
evidence. npm receives the checked client, testing and RN tarballs in dependency
order, with provenance. Cross-registry publication is not atomic: inspect partial
results before retrying; never overwrite, unpublish or silently skip a version.

[Documentation](../../.github/workflows/pages.yml) runs only on main. It executes
the complete existing documentation verification, site/reference build, real
Chrome inspection and report-bundling rebuild before uploading `docs/public/dist`.
A hosted deploy job sends that artifact to Pages. The expected project URL is
`https://hoyadb.github.io/hoyadb/`; it is not claimed deployed. Relative links
support the project prefix. Full verification currently needs the native tools
and task-owned Android emulator; hosted Python alone cannot replace those checks.

## Rules

PR jobs use hosted Ubuntu, read-only contents access, no secrets and no submodules.
External actions are SHA-pinned; Dependabot proposes updates. First-party code
and npm packages carry MIT; existing third-party terms remain. Root npm stays private.
Both native notice corpora carry the same MIT text and bind its digest. Their
validators require MIT rather than the former no-license state; third-party
notice/provider/lock checks remain enforced. Historical packets are unchanged.

Native and complete documentation builds need a **dedicated reviewed Intel Mac
runner**, newly labelled `self-hosted`, `macOS`, `X64`, `hoyadb-native-reviewed`.
This is a provisioning requirement, not an already registered runner. Restrict
it to the main-only release/documentation workflows, isolate it from contributor
PR execution, and use disposable runner/work directories without unrelated
stores or credentials. Native/Pages builds share `native-reviewed` concurrency
to serialize access to their task-owned emulator. They receive no publishing secrets.

Existing absolute paths and hash checks in
[toolchain-review.json](../../bindings/android/toolchain-review.json),
[Android build.py](../../bindings/android/tools/build.py),
[package-aar.py](../../bindings/android/tools/package-aar.py) and documentation
language runners remain authoritative. Provision their exact compiler/runtime
closure, JDK 17, SDK 37 and NDK 27.1.12297006 at the recorded locations. Also
provide the pinned Rust targets/caches, Xcode/Swift, LLVM 22, Chrome,
Node 22.18.0/npm 11.17.0 and TypeScript 7.0.2. Set `HOYADB_WASM_BINDGEN` to an
independently installed wasm-bindgen **0.2.129** outside the disposable checkout.
The ordinary web builder otherwise expects it under `target/tools/wasm-bindgen`.
See [activation](../../packages/react-native/ACTIVATION.md),
[Swift](../../bindings/swift/README.md) and [docs verification](../public/README.md).

`stale-provider-evidence` and lock/notice mismatches require fresh reviewed
evidence through the original process. Do not update expected hashes just to
pass. The native release recipe does not automate ordinary RN app conformance:
execute the current packed candidate through the iOS/Android activation consumers
before approving publication. Physical/arm64 support needs its own evidence;
host tests and environment review do not supply runtime acceptance.

## Repository and account setup

1. Enable Actions on public `hoyadb/hoyadb`, allow pinned official `actions/*`,
   keep default token permissions read-only and require `CI / Required checks`
   on main. Protect `v*` release tags from changes/deletion. GitLab's server
   history is independent of changing local remotes.
2. Provision the reviewed native runner above. Create `native-build` and
   `documentation-build` environments restricted to main, with source/runner
   review before allowing native execution.
3. In Settings → Pages choose **GitHub Actions**. Restrict `github-pages` to main.
   Pages uses the scoped workflow token; it needs no personal access token.
4. Create the `npm` environment. Configure a GitHub trusted publisher on **each**
   package: owner `hoyadb`, repository `hoyadb`, workflow **`release.yml`**,
   environment **`npm`**. First-time package creation needs bootstrap: publish the
   reviewed tarballs interactively with 2FA, then configure OIDC, or temporarily
   set environment secret `NPM_TOKEN` to a narrowly scoped granular publishing
   token. Remove it after trust is configured. Public source access does not grant
   publishing rights. See [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/).
5. Verify `dev.hoyadb` in Sonatype Central. Ownership of `hoyadb.dev` is **not yet
   confirmed**. If unavailable, explicitly change the coordinate across Android,
   RN and packaging first. In `maven-central`, set `CENTRAL_TOKEN` (the Portal
   username/password encoded as its documented Bearer token),
   `MAVEN_GPG_PRIVATE_KEY` (armored key) and `MAVEN_GPG_PASSPHRASE`. Publish the
   signing public key. See the [API](https://central.sonatype.org/publish/publish-portal-api/)
   and [Central requirements](https://central.sonatype.org/publish/requirements/).
6. Create main-restricted `github-release`, `npm` and `maven-central` environments
   with publication review. Review current artifacts and native runtime evidence
   before approving. Enable private vulnerability reporting and update
   [SECURITY.md](../../SECURITY.md) with the actual configured channel before launch.

No workflow creates these accounts/settings. CI/Pages need no publishing secrets;
Central needs credentials/signing, and npm uses OIDC after bootstrap. Never commit
credential values. A public repository still requires write authentication to push.

## Required checks

```sh
python3 -m unittest discover -s tools/ci -p 'test_*.py'
python3 -m unittest discover -s tests/packaging -p 'test_*.py'
python3 -m unittest discover -s tools/release -p 'test_*.py'
python3 tools/release/compatibility.py sources
python3 tools/ci/delivery.py release --tag v0.1.0 --repository hoyadb/hoyadb
actionlint
bash -n tools/ci/native.sh
python3 tools/traceability/trace_check.py docs/design
python3 tools/traceability/rtm_gen.py docs/design --check
```

Install `actionlint` with `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`.
CI also installs ShellCheck. Run the normal contribution/package checks. The local
release command checks metadata, not tag existence or native/account acceptance;
GitHub preflight checks ancestry separately. Hosted and registry outcomes must
actually be observed before claiming delivery passed.

## Update this page when

Update with workflow triggers, permissions, runner requirements, versions,
credentials, destinations or native evidence requirements. Preserve historical
verification packets and accepted ADR bodies.
