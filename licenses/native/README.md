# Native notice inputs

`apple-build.json` and `apple-providers.json` retain the exact compilation-parent
and provider-symbol records bound by the Apple notice index. Their bytes and
hashes are unchanged. They are validator inputs, not evidence that a current
build has passed. Fresh binaries must match the provider and parent checks.

[native-dependencies.json](../native-dependencies.json) records the exact active
native package identities and licenses used by the Android notice validator.
Upstream license grants remain in the indexed platform notice corpora.
