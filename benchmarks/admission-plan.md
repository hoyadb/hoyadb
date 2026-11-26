# Between-worker admission protocol

Reviewed internal orchestration for TC-0037. This changes no dataset, sample
count, result oracle or numerical budget. Implementation follows behavioral
RED/GREEN; only an explicitly coordinated quiet admission permits measurement.

The browser already launches exactly one independent worker/run per explicit
`hoyaBenchmark.warm(args)` call. Add a controller-side one-use permit for exact
engine/cohort/run/worker identity. `start` consumes the permit before platform
launch. With no permit, `start` rejects before any worker or store acquisition.
Do not pause an active worker, add waits to sample clocks, poll admission from a
worker, alter counts or replay failed opens.

A parent control command `ADMIT <JSON coordinates>` may create that permit only
while no worker is active and after checking the frozen source/artifact closure.
After an accepted `finish`, emit `BETWEEN_WORKERS <run identity + raw hash>` only
after the exact report is persisted and cleanup has validated owner closed,
worker terminated, pool baseline and neighboring canary. Remain in this state
until the next explicit admission. A run rejected after raw capture records a
failure and requires explicit recovery; it cannot silently admit another run.

With one permit consumed and no pre-issued next permit, JavaScript cannot
automatically launch a later worker before the controller observes completion.
While BETWEEN_WORKERS is observed, other correctness work may run. Record the
hold start/end and competing-work declaration outside each worker's timings;
thermal/cache/environment continuity across independent workers is uncontrolled
and explicit. Recheck frozen hashes before the next admission. Preserve every
original raw result, including failures and interrupted observations.

For native, the analogous reliable state belongs after exact child reaping,
raw/proof validation and persistence, before any possible next Popen. The input
wait must precede the next child's launch clock. Browser worker termination is
the synchronous browser Worker API request, separately named from native wait4
reaping; do not claim browser OS-child process reaping.

The durable CLI/installer can be built now with measurements disabled. Permit
commands/start enforcement and their RED/GREEN tests wait for parent review and
committed protocol wording. No signals, sleeps, numerical thresholds or schema/
sample-count changes are part of this proposal.


## Named acceptance cases before implementation

- A01: with no permit, start rejects before a worker/store/child is acquired;
  a valid one-use exact engine/cohort/run/worker permit admits only that run.
- A02: wrong, duplicate, stale or pre-issued next permits reject; one consumed
  permit cannot launch a second worker. No timer/sample-loop polling is added.
- A03: BETWEEN_WORKERS is visible only after native child wait/reaping or the
  separately named browser termination request, complete raw persistence and
  the applicable exact owned-store/pool/canary cleanup validation.
- A04: a hold cannot race an automatic next launch: after the observed held
  state, no worker starts until a new explicit permit and frozen-hash recheck.
  The wait is outside child launch-to-ready/exit and every sample clock.
- A05: source/artifact drift, failed cleanup/proof or malformed control input
  blocks the next admission and preserves the failed/incomplete original raw.
  Cancellation while held cleans only owned resources and records failure.
- A06: declared competing correctness work is allowed only in the held state;
  record hold/control timestamps and per-admission environment observations.
  Thermal/cache continuity is explicitly uncontrolled across independent runs.
- A07: default measurement-disabled preparation cannot fabricate timings or
  auto-admit measurements. All5 independent runs and5+30 samples remain required
  for a complete accepted warm cohort. A failed/interrupted cohort stays open.
  A new native or browser run requesting run status `ok` requires stdin
  admission; the command rejects at argument validation, before any build,
  output directory, fixture, store or child/worker acquisition. Unadmitted runs
  remain `contaminated`. Earlier immutable evidence keeps its recorded protocol.
- A08: real bounded integration demonstrates first worker completion/cleanup,
  a visible held state with no possible second launch, then one admitted second
  worker. Synthetic protocol tests are correctness evidence only, not latency.

## Reviewed amendment: page permit transport

Reviewed for implementation after this preparation is committed. It adds
transport for a permit the parent already issued; it adds no new way to
issue, recover or widen a permit. A01–A08 and their negative cases are
unchanged and must still pass with the same counts.

The parent still creates every permit with `ADMIT <exact coordinates>` on the
controller's stdin. A new loopback route `POST /benchmark/next` lets the page
learn that one admitted, unconsumed permit exists. The request body is exactly
`{}`, at most 256 bytes, with the existing loopback-origin and frame checks;
other methods, bodies or origins reject. Reading the route never admits,
starts, consumes, recovers or revokes anything. Only the existing start routes
consume a permit, and they still compare the request with the exact permitted
coordinates, so the page cannot choose different coordinates.

The route accepts a request only while no worker is active and the collector
is idle, recovered, held with completed cleanup validation, or admitted with
an unconsumed permit. The admitted state permits both request-before-ADMIT
and ADMIT-before-request orderings. During an active worker it
rejects immediately, so the page cannot prefetch a later permit. If no permit
exists yet, the controller waits up to 25 seconds, a fixed operational transport
limit recorded in the manifest, not a performance threshold. Tests use an
injected wait bound to exercise expiry without a 25-second test delay. The wait ends early on
`ADMIT`, `CANCEL` or server shutdown, and then returns either the permit or a
static `none` reason (`timeout`, `cancelled`, `blocked` or `already_claimed`). The wait runs on
the page's main thread, outside every worker, sample and launch-to-ready
clock. Workers never poll.

At most one waiting request and one claim may exist per collector; additional
waiters receive `none: already_claimed` without registering another wait. The
first response that carries a permit
records a claim in `control.jsonl`. Concurrent or later requests for the same
permit get `none`. A claim does not consume the permit; the next matching
start does. If the page disconnects or never starts, the claimed permit stays
unconsumed. A new parent command, `REVOKE {"reason": ...}`, may withdraw it
only while no worker is active, including the admitted-but-unstarted state.
Revocation blocks further admission until
explicit `RECOVER`, the same as the existing failure path. Consuming a permit
rechecks the frozen source/artifact closure for every start route, including
the existing direct ADMIT-then-start path; on drift the start rejects before
any worker exists and blocks admission. A page loop (`hoyaBenchmark.matrix()`)
requests the next permit only after its previous run's finish reply. That reply
is returned only after the held state is recorded. The loop runs the one flow
whose kind matches the permit and stops on any `none` response or failed flow.
The parent may explicitly restart the page loop after a hold or recovery;
there is no automatic busy retry or replay of a failed flow.

### Named cases for the amendment

- N01: `POST /benchmark/next` with no admitted permit and no `ADMIT` returns a
  bounded `none: timeout`. No run, worker, store or control-log admission is
  created.
- N02: after `ADMIT`, one request receives the exact coordinates and a recorded
  claim. A concurrent or later request receives `none`. A start whose
  coordinates differ from the permit rejects without consuming it.
  Both ADMIT-before-request and request-before-ADMIT orderings work; a second
  concurrent waiter is rejected without allocating a new wait, and an already
  claimed permit is never redelivered after transport failure.
- N03: while a worker is active, the route rejects immediately and cannot
  deliver or claim a later permit. No waiter exists across a running worker.
- N04: GET, oversized or non-`{}` bodies, foreign origins and query strings
  reject. No request to the route issues, recovers, revokes or consumes a
  permit.
- N05: a waiting request wakes on `CANCEL` and on server shutdown with
  `none: cancelled`. Shutdown completes within the bounded wait and leaves no
  thread or connection behind.
- N06: a claimed but unstarted permit after a disconnect stays unconsumed.
  `REVOKE` with no active worker, including an admitted-but-unstarted permit,
  withdraws it, records it and blocks until
  `RECOVER`. `REVOKE` during an active worker rejects.
- N07: source/artifact drift between `ADMIT` and consumption rejects the start
  before any worker or store acquisition and blocks admission. The original
  failure records are preserved, including on the original direct start path.
- N08: real bounded integration (contaminated): the page loop runs exactly the
  admitted sequence, and no second worker exists while the controller is held.
  The page start-rejection and blocking counts from A01–A08 are unchanged.
  A timeout or failed flow stops the loop without automatic replay; a fresh
  explicit page invocation is required to resume.

Rollback removes unpublished orchestration code. Previously captured raw files
and their identities remain immutable; old/new protocols are not silently pooled.
No new dependency, SDK API, persistent format or product policy is introduced.
