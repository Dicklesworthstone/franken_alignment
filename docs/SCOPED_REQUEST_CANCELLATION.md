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

## Tower and authenticated operator integration

The existing `next` and `resolve` verbs now accept a closed, explicit
`"action": "cancel_undispatched"` discriminator for this separate capability.
Omitting that field preserves the existing receipt-reconciliation format and
authority. An unknown action refuses. Cancellation adds no registered verb:
the publication adapter still connects thirteen of the thirty-five verbs.

Mount the original port with `PublicationTower::set_cancellation`. A foreign
owner refuses installation even if its numerical scope matches, and a rejected
replacement clears the previously installed port. Restricted control operators
accept the separately granted capability. The reconciliation-only constructor
and learned observation-only mount do not gain cancellation authority. A
learned control operator additionally requires the original explicit numerical
replay allowance.

For a live authenticated operator endpoint, add this optional member to the
existing public `fa.tower-operator-service/1` profile:

```json
"cancellation": {
  "queries": 32,
  "operations": 8,
  "max_replay_events": 4096,
  "authority": "cancel_undispatched_requests"
}
```

All values are unsigned JSON integers. Queries allow 1–1,024, operations 1–128
and the complete per-operation history 1–4,096 events. The authority marker must
match exactly. Omission or null disables cancellation. Unknown fields,
duplicates, malformed or excessive values refuse before custody or socket setup.
A cancellation-only endpoint can set inspection and admission explanation to
zero with `explanation_disclosure: "none"`.

The object may also select a positive `numerical_replay_events` integer, at
most `operations * max_replay_events`. Learned actor and resident services
require it when cancellation is selected. Ordinary services may omit it,
which leaves numerical histories unsupported by that grant. Explanation,
reconciliation and cancellation numerical allowances cannot fund one another.

The service retains the original cancellation grant across authenticated
reconnects, scheduled requests, resident handoffs and terminal reply grace.
Neither a new connection nor a new job renews spent work. Retirement drops the
custodian. Selecting cancellation does not select Fence or reconciliation, and
receipt-reconciliation authority does not authorize cancellation.

### Explicit request and response

For the example scope tenant 1, principal 2, run 3, branch 4, authority 5,
request 7 and route generation 41:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "operator-cancel-plan-7",
  "address": "fa://1/attempt/request-3-2-4-5-7@41",
  "verb": "next",
  "arguments": {
    "action": "cancel_undispatched",
    "operation": "9001"
  },
  "idempotency_key": null
}
```

A known plan supplies a complete explicit `resolve` document containing the
same discriminator, canonical operation key and full acknowledged predecessor.
The operation key must be repeated as the Resolve idempotency key. The original
registration generation and every native basis coordinate are checked. The
operator must explicitly choose that document; neither Next nor the client
submits it automatically.

Using an already qualified executable, first send the plan document:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE NEXT_JSON
```

After inspecting the returned action and saving the complete action document as
`CANCEL_JSON`, submit it explicitly:

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE CANCEL_JSON
```

This second connection requires remaining connection allowance. The owner may
have advanced since Next; a stale predecessor refuses and requires a fresh,
explicitly selected observation. The client does not alter the predecessor or
invent an operation to overcome that refusal.

The distinct result kinds are `cancellation-next` and `cancellation`, with
schemas `fa.publication-cancellation-next/1` and
`fa.publication-cancellation/1`. Both identify scope
`original_undispatched_request`, idempotency scope
`live_cancellation_grant`, request, registration and operation. The plan
reports stage, `can_cancel`, an optional block reason and explicit action.
Cost, information gain, ranking and current publication eligibility remain
Unknown. The receipt includes the before cut, after basis, original stage,
Cancelled state and canonical refunded-unit string.

The authenticated explicit-document client validates the closed schema,
complete correlation, action, scope, original predecessor, unchanged clock,
single journal increment, permissible prior stage and refund spelling before
output or any later explicit request. It rejects mixed reconciliation and
cancellation replies. It never follows an action, reconnects or resends by
itself.

### Continued service after cancellation

The existing driver observes the original terminal Cancelled disposition,
retires the current review/Stop endpoints, reaps the cancelled job and continues
the finite request schedule. No domain Fence or new actor/source/operator
custody is needed.

A real-socket regression exercises both plain and checked mediated services:
cancel request 7 while it is Authorized and waiting for its human key, observe
its released reservation, reconnect with the same spent grant, preserve the
original cancellation receipt, and publish request 8 through its actual
reviewer. The selected source budget permits exactly the initial cut and the
one final cut for the surviving publication. The assertions include one
external execution, the second payload, unchanged target version until that
publication, correct final charge, zero reservations and retired endpoints.

The integration increment adds seven public Tower regressions, two codec
regressions, five operator/profile regressions, three client regressions and
the single plain/checked composition function above. All twenty changed Rust
files pass syntax parsing; all eight new integration files pass formatting
checks. The required focused RCH launch again exits 127 because the runner is
absent. These are authored regression sources and source checks, not executed
runtime results. See
`artifacts/execution/2026-11-11-scoped-cancellation-integration-verification.log`.

## Native regression sources and verification

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
