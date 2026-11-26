# Managed sync notification checkpoint

DEF-0012 now has authentic post-preparation33a9d97 RED/GREEN coverage on the real
WASM memory facade through the JavaScript SDK. Preparing a queued command did
not deliver its managed Sent callback; independently, receiving a wire outcome
did not deliver Acknowledged. Both remained queued for an explicit later drain.

A private notification-only helper now drains managed listeners after successful
prepare/receipt calls, checking the captured generation. It does not wake the
sync driver, add timers or drain an unowned raw stream. Opaque payloads and core
retry/deadline behavior remain unchanged. The shared consumer contains both
cases so the final protected package runs the same assertions.

Actual WASM memory passes **2/2** new cases and the full JavaScript workspace
passes **83/83**, including the scheduler guard: exactly one push and two prepare
calls (send, then empty due check), with no continuing pump. Syntax checks pass.
The original self-wake test incorrectly expected a cleared timer handle; the
existing implementation retains its destroyed handle. That incidental assertion
was corrected before the patch; required call-count and idle-state assertions
remain. No production timer behavior was changed.

[Source and raw-log hashes](phase-seven-web-managed-sync.json) and the
[full Node output](phase-seven-web-managed-sync-node.txt) retain the evidence.
Matching packed memory/protected browser reruns remain pending this checkpoint.
The [82-test lifecycle checkpoint](phase-seven-web-lifecycle.md) is unchanged.
