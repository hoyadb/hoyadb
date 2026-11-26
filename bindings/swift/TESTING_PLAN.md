# Swift testing-entry completion packet

Consumes API-0011 testing entry and injected monotonic/timer capabilities;
TC-0036 G05/G19/G20/G21/G22. No dependency or storage/wire change. This is
preparation for review before the new public surface/tests/implementation.

## Additive public host timing seam

The existing internal MonotonicClock/TimerScheduler/TimerCancellation are used
by real tests through @testable, but ordinary package consumers cannot inject
them. Export those existing type aliases/protocol, and extend the public
Database initializer with optional monotonicClock/timerScheduler arguments.
Nil uses the existing production defaults. The internal NativeCalling injection
stays internal. TimerCancellation.cancel remains the sole cancellation method.
No UTC clock substitutes for observation uptime; no throttle/deadline logic is
copied into the test helper.

## Separate HoyaDBTesting SwiftPM product

The product depends only on HoyaDB and installed Foundation. It ships no alternate
database, storage provider or resolver and is not linked by the production target.

- ManualClock: locked Int64 now/set/checked advance, with overflow rejecting
  without mutation. Used as captured UTC clock, not uptime.
- ManualUptime: equivalent UInt64 clock for count delivery. ManualTimer schedules
  owned callbacks without executing them until fireNext; cancellation removes
  work and callbacks run after its host lock is released. Expose pending delays
  for deterministic assertions, never compute core due times.
- MemoryTesting.open: explicit namespace and caller schema, real core memory,
  optional real MemoryStore/migrations/IDGenerator/options and the public timing
  seam. No path/key/entropy fallback, network or crypto-provider initialization.
  Native core is already linked; no bootstrap work is hidden in this factory.
- FakeTransport: actor with immutable command-name/entity-table handler maps.
  Handlers receive value-owned JSONValue envelopes and return enum business
  applied/corrected result or rejection, never correlation fields. Production
  JSONValue.decodeJSON/encodeJSON preserve i64, Float and Unicode. Attach command
  idempotencyKey or entity table/recordId/writeId from that exact envelope.
  Explicit async delay and drop-ACK hooks plus reverseReplies exercise failure,
  retry and reordering; no array-position correlation, retry planner or SQL.
  Retain value-owned pushes for assertions. Missing/malformed handler envelopes
  reject with a static test-helper error. This is a transport fixture, not a
  production backend implementation.

## Behavioral RED/GREEN acceptance after review/commit

Compile the public timing constructor with a stub that ignores supplied timing;
observe a managed count initial0 then insertion with manual scheduling, and
require the expected pending16ms timer and delivery only when explicitly fired.
The ordinary import HoyaDB/HoyaDBTesting tests cannot use @testable. Observe
behavioral failure, then connect the existing scheduler/clock unchanged.

Compile testing helpers against stubs first. Require real memory insert/fetch,
explicit namespace with an exhausted generator rejecting without fallback,
retained memory migration, manual clock overflow unchanged, cancellation of a
manual timer and callback execution outside its lock. Fake tests use actual
queued core envelopes: fixed local ID, i64max/integralFloat payload, command
application/rejection, two entity writes to the same record with distinct
writeIds, reversed replies correlated by key, delayed reply and dropped first
ACK followed by exact frozen retry at the core's returned deadline. The fake
must not own/query a database. Public ordinary consumer runs original
TC-0016/0017 examples using this helper rather than a private fixture dispatcher.

Keep all existing Swift tests/ordinary consumers, strict formatting and warnings
as errors. Record formal prep before RED, exact output and current package/source
hashes. Existing exploratory35-test foundation remains a distinct earlier cut.

## Named acceptance cases (TC-0036 mappings)

```gherkin
Feature: Public Swift testing support uses the production runtime
  Scenario: ST01 public timing injection controls a managed count deadline
    Given an ordinary import of HoyaDB and HoyaDBTesting with manual uptime and timer
    And a real memory owner whose initial count0 has been delivered
    When one insertion commits before16ms elapses
    Then exactly one16ms timer is pending and no second count is delivered yet
    When uptime advances to16ms and that timer fires
    Then count1 is delivered through the selected executor without manual drain

  Scenario: ST02 explicit memory factory preserves generator and retained migration
    Given an explicit namespace caller schema fixed clock and finite ID generator
    And a retained real MemoryStore
    When the factory opens and inserts a row then the generator is exhausted
    Then its fixed ID and timestamp are exact and the next insertion rejects atomically
    When the owner closes and an authored v1 to v2 migration reopens the same store
    Then the original row remains and schema2 is current without any path key or network

  Scenario: ST03 manual clocks and timer cancellation preserve owned scheduling
    Given manual UTC and uptime clocks at their integer boundaries
    When an advance would overflow
    Then it rejects without changing the previous value
    Given two scheduled timer callbacks and one cancelled token
    When the next live timer fires
    Then the cancelled callback never runs and the live callback can schedule work
    And no callback executes while the timer's host lock is held

  Scenario: ST04 fake command outcomes retain typed payloads and exact correlation
    Given two real queued commands with distinct keys and tagged i64max Float and Unicode
    And named fake handlers returning only application result or business rejection
    When their exact production envelopes are pushed and replies are reversed
    Then each response carries its own command key with the original typed payload
    And receipt plus explicit application plans reaches the correct terminal state

  Scenario: ST05 fake entity outcomes distinguish writes to the same record
    Given two real entity envelopes for one table and recordId with distinct writeIds
    And an entity-table fake handler returning only a business result
    When the fake replies in reverse order
    Then table recordId and each exact writeId come from their own envelope
    And the production receive path does not correlate by array position

  Scenario: ST06 delayed and dropped acknowledgements retain the real frozen retry
    Given a real command sent at a fixed injected UTC clock with fixed jitter
    And a fake whose reply delay is suspended on an application gate
    When that gate is released and the fake drops the first acknowledgement
    Then the core record remains Sent and no acknowledgement is fabricated
    When the caller retries at the exact core-returned due time
    Then the original envelope bytes are unchanged and a new fake reply applies once
```

## Rollback

These products/APIs are unpublished and additive. Revert their source/manifest
changes together; no persisted store or queue format is changed or migrated.
Existing production defaults and the pre-existing public SDK remain intact.
Never discard a store or recovery record to remove a testing helper.
