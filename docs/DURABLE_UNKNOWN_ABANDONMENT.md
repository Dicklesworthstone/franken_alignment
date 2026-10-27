# Explicit durable unknown-delivery abandonment

`FileDelivery::abandon_unknown` and `FileOversight::abandon_unknown` persist the
original broker's trusted `Unknown -> IrrecoverablyUnknown` transition. This is
the bounded FA-014 reference implementation of plan sections 8.2-8.5 and
16.4/16.9, serving FI-A02/FI-A06 and FA-INV-005/006. The operator records an
irrecoverable liability; the operation does not establish an endpoint outcome.

## Authority and retry contract

The trusted supervisor supplies a `FileAbandonRequest` with a nonzero operation
and attempt plus the independently selected control sequence and authority
epoch. A new operation requires that exact current predecessor and an original
Unknown attempt. Dispatching, pre-dispatch and already settled attempts refuse.
Retention expiry alone still leaves Unknown and never selects abandonment.

The owner also requires its current journal revision. An exact retry of a
previously acknowledged operation returns the original `FileAbandonReceipt`
without another event, even after a recovery Fence advances the current epoch.
Changing any field under that operation ID refuses. A new operation cannot
rewrite an already irrecoverable attempt. A poisoned owner refuses both mutation
and receipt lookup until canonical recovery; an I/O error cannot prove that a
replacement did not become visible.

The receipt binds the complete original `FrozenAction`. Its `charged_units()`
are that action's original conservative charge, not a refund or available
balance. The receipt is not an `EndpointReceipt` and cannot be accepted as one.
No actor port acquires this supervisor-only mutation.

## What the transition preserves

The native reducer changes the selected disposition and the durable host removes
only that attempt's retained sendable envelope. The same owner therefore cannot
later publish that envelope. Other eligible requests keep their original path.
The operation neither contacts the endpoint nor clears a pending stream: a
message or Finish successor still cannot advance past the unresolved attempt.

Available, reserved and charged units; previously published payloads; execution
count; target version; clocks; policy; authority epoch; source observations; and
numerical continuation state keep their original meanings. In particular an
already executed but unreconciled effect remains visible and charged. A normal
trusted reconciliation before abandonment can still establish its real outcome.
Ordinary reconciliation cannot subsequently rewrite the terminal disposition.

Actor polling continues to return the existing Unknown outcome. A pending sweep
omits irrecoverable attempts, but Stop progress retains each in both `unresolved`
and `irrecoverable`; the domain is not drained. An empty sweep is never evidence
that the liability disappeared.

## Storage and compatibility

Base journal tag 18 carries only the four request integers. Its record occupies
37 bytes including framing, or 38 bytes inside the observed journal's Core
wrapper. Replay recomputes the receipt through the original reducer; imported
receipt data cannot invent a disposition. Repeated canonical abandonment events
are invalid even though an exact public API retry is read-only.

Existing bootstrap encodings, version domains and event tags are unchanged.
Histories without this event retain their existing bytes. Older readers reject
the unfamiliar event instead of ignoring it; a history containing abandonment
must stay with a reader that supports tag 18. No permissive downgrade is supplied.

Abandonment uses **ordinary journal capacity**. It cannot spend the existing
three-event, 50-byte Fence/Stop/StopProgress reserve. Exhausted ordinary capacity
refuses explicitly; the change does not reinterpret an existing reserve or
promise unlimited restarts. Both dimensions are checked before persistence.

## Validation boundary

The authored regression cases exercise actual original base and two-key
dispatches, state and predecessor refusals, exact retries, retained execution,
actor and stream behavior, canonical encoding/replay, ordinary capacity and all
five native replacement fault barriers. Fault injection is not a physical power
cut or proof about a remote endpoint.

The preparation environment has no RCH executable or configured worker. New
source is checked with the pinned nightly-2026-09-08 rustfmt parser; no local
compilation fallback is used. Compilation, runtime tests, Clippy, canonical
admission and full FA-014 completion remain pending. The existing dependency
admission guard is unchanged, and the scoped bead remains in progress.
