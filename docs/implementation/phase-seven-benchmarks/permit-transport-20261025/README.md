# Page permit transport: functional checkpoint

Preparation416fffb precedes the N01–N08 tests and implementation. This records
functional verification of the browser launcher, not an accepted performance
baseline or final Phase7 review. The earlier functional checkpoint is unchanged.

The new41-file lane closure is `bdbd890c52efe9925df32fe0eb51e5332fa1fff3afe7be66c3e386c064d63ffe`.
Root independently rehashed every recorded file with zero mismatches. The agent's
final logs report79 passing JavaScript tests and86 passing Python tests. Existing
A01–A08 tests and negative controls remain in the suite.

The original stub run includes setup errors for a missing waiter attribute;
those are separate from the subsequent behavioral RED. REVOKE testing exposed
a lock-not-held notification regression in the implementation, which was fixed
before the final green run. All intermediate logs are retained here.

The actual bounded N08 run uses the existing Paseo Electron/Chromium host and
unchanged source hashes. Its manifest marks the observations contaminated.
Two cold memory workers exercise request-before-admission; a fresh explicit
page invocation exercises admission-before-request for the memory resource run.
The control log records one admit/claim/start/between-workers sequence per run,
then cancellation. Only the two cold runs count as timed samples; the resource
run contributes none. Timeout stops the page loop without replay, and cancellation
wakes its waiting request. Tab/server/temporary-app cleanup is reported complete.

A later direct start after cancellation hit the already-stopped server. It is
not a valid rejection test; no-permit rejection has separate unit and earlier
A08 integration coverage. No replacement rejection result is inferred.

Original raw files, commands, manifests, source maps and test logs are copied
without normalization. Package archives are omitted from this evidence copy;
their identities remain in the original manifests and package evidence. The
original target-directory capture is preserved. `files-sha256.json` indexes
the copied bytes and this report.

An attempted focused Opus5.5 cross-review did not run: the installed Claude Code
2.1.220 returned an error requiring2.1.280 or newer for that model. A compatible
Opus model is being checked without updating installed tools. No review verdict
is claimed here. Accepted measurements remain gated on core scaling diagnosis,
coordinated quiet admission and the remaining matching-artifact checks.
