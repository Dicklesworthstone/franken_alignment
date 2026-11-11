# Independently authorized request cancellation

The native publication supervisor can grant an operator finite authority to
cancel one original **undispatched** request. This closes the gap between an
actor cancelling its own ticket and an independent operator fencing the entire
domain: an operator can retire unwanted pending work while another request keeps
its reservation and can still publish.

This is the original terminal Cancel transition. It is not a temporary Hold:
Cancel releases the selected request's undispatched reservations and applies the
original dependent-buffer cleanup. The capability cannot refund dispatched or
uncertain effects, publish, refresh a source, acquire a clock, approve a human
key, or manufacture an actor ticket.

## Native API and custody

The original `FileActorSupervisor<FileOversight>` offers:

- `cancellation_port(queries, operations, max_replay_events)`
- `cancellation_port_with_numerical_budget(queries, operations,
  max_replay_events, numerical_replay_events)`

Both return a weak `FileSupervisorCancellationPort` and a nonclone
`FileSupervisorCancellationGrant`. Retain the grant for the lifetime of the
operator's authority. Port clones share its counters and exact-result cache.
Dropping or consuming the grant's `revoke` method removes that authority. The
port identifies the exact original owner, not just a numerically equal scope.

Queries and operations must be positive and at most
`MAX_CANCELLATION_QUERIES` (1024) and `MAX_CANCELLATION_OPERATIONS` (128).
The per-operation event ceiling uses the existing journal-event bound. These
are finite original replay-work allowances; they do not estimate cost,
information gain or model quality.

Call `port.next(request, operation)` to observe the original request and this
grant's remaining work. A known `FileCancellationPlan` contains the complete
`FileHoldDiagnosisBasis`, original stage, nonzero caller-selected operation
key and a possible block reason. Its `request()` returns a
`FileCancellationRequest` only when cancellation can be attempted. Next
consumes one query, reserves no operation identity, and mutates no source,
clock, request or admission snapshot.

Call `port.cancel(request)` with that exact request and the same retained
custody. The full predecessor is rechecked against the original native owner
before entry. A fresh operation accepts only Proposed, Prepared, Reviewing or
Authorized. Dispatching and Unknown retain their charge and refuse;
IrrecoverablyUnknown and all terminal stages also refuse.

The public results use the existing `Knowledge` wrapper. A missing request or
temporarily unavailable observation does not become affirmative authority.
Malformed requests, stale predecessors, conflicting operation identities,
exhausted capacity and unavailable custody remain explicit failures.

## Accounting, replay and failure ordering

Execution calls the existing `FileOversight::cancel_request` transition.
No new journal event, durability law, effect permit or dependency is introduced.
A successful `FileCancellationReceipt` records the native before and after
cuts, prior stage, operation key, Cancelled stage and actual refunded units.

The postcondition requires exactly one acknowledged original Cancel event,
unchanged recorded time, unchanged charged liabilities, and an increase in
available units equal to the decrease in reserved units. Only that original
undispatched reservation can be released. Other requests retain their state and
the native Cancel reducer performs dependent-buffer cleanup.

The complete retained history plus the proposed Cancel event must fit the
per-operation replay bound. Any history that requires numerical reconstruction,
including a raw-frame predictor, also requires the independently selected
cumulative numerical-event allowance. The ordinary constructor deliberately
refuses that work. Reserving an exact sufficient numerical allowance permits
the existing native reconstruction; a short allowance refuses before entry.
Cancellation does not acquire a source or create a new forecast.

Before entering native work, execution consumes an operation, records its exact
identity, reserves any required numerical work, and invalidates the original
admission snapshot. A storage failure or interruption after entry does not
restore those allowances, repeat native work or acknowledge a speculative
refund. Pre-entry refusals preserve the snapshot and mutation allowance.

An exact retry under the same live grant and healthy original owner returns
the original cached result without consuming another operation, reacquiring a
clock, replaying history or invalidating a later admission snapshot. A different
request under the same operation key conflicts. An entered operation without
an acknowledged result cannot execute again. The cache is scoped to that live
grant; reopening an owner or provisioning a replacement grant does not create
durable cross-process idempotency.

## Regression sources and verification

Eleven native regression functions cover a real two-key publication after
cancelling another Authorized request; Reviewing cancellation; unchanged
charged Dispatching, Unknown, abandoned and confirmed effects; every
predecessor coordinate; exact retry identity; finite queries, operations and
replay work; custody and foreign owners; all five storage replacement barriers;
and interrupted-operation retention.

The numerical cases compare an actual learned-stream cancellation with the
complete original direct-Cancel result, including message, audience, stream and
numerical state. They also exercise raw-frame predictor history, exact versus
short numerical reservations, other pending work and failed-storage accounting.

Sources:

- `crates/fa-reference/src/action/consequence/delivery/persistent/requests/actor/cancellation.rs`
- `crates/fa-reference/src/action/consequence/delivery/persistent/requests/actor/cancellation/port.rs`
- `crates/fa-reference/src/action/consequence/delivery/persistent/requests/actor/cancellation/tests.rs`
- `crates/fa-reference/src/action/consequence/delivery/persistent/requests/actor/cancellation/tests/numerical.rs`

The 2026-11-11 native increment passed syntax parsing of all five changed Rust
files, formatting checks of the four new files, and whitespace checks.
Independent source reviews covered implementation and regressions. The required
focused RCH test launch exited 127 because `rch` is unavailable; no compiler,
runtime, Clippy or full-gate success is claimed. The exact command and output
are retained in
`artifacts/execution/2026-11-11-scoped-cancellation-native-verification.log`.
FA-009 remains open; these reference capabilities do not establish the complete
production state-machine packet.
