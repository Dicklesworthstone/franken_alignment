# Independently authorized live Unknown abandonment

A live publication supervisor can grant an operator finite authority to record
one original Unknown delivery as IrrecoverablyUnknown. This connects the
existing durable disposition to an original request without reopening the
journal, constructing an actor ticket or stopping unrelated work.

The transition records an unresolved liability. The original charge stays
spent, actor polling still reports Unknown, and a pending stream remains
blocked. It establishes neither execution nor nonexecution. The native
[durable abandonment contract](DURABLE_UNKNOWN_ABANDONMENT.md) continues to
govern storage, replay, ordinary capacity and retained endpoint uncertainty.

This is scoped reference implementation work under FA-014 and the Tower
dispatcher work in FA-138, serving plan sections 8.2-8.5, 16.4/16.9 and 17.8,
FI-A02/FI-A06, and FA-INV-005/006. It introduces no new consequence, journal
event, dependency or permission-widening mechanism.

## Native API and independent custody

The original `FileActorSupervisor<FileOversight>` exposes:

- `abandonment_port(queries, operations, max_replay_events)`
- `abandonment_port_with_numerical_budget(queries, operations,
  max_replay_events, numerical_replay_events)`

Each returns a weak `FileSupervisorAbandonmentPort` and a nonclone
`FileSupervisorAbandonmentGrant`. Retain the grant while the operator may use
the capability. Cloned ports share its finite counters and exact-result cache.
Dropping or revoking the grant withdraws every clone. A port belongs to the
exact original owner, including when another owner's numeric scope is equal.

Queries allow 1-1,024 operations of observation; mutations allow 1-128 entered
operations. The complete per-operation replay ceiling is 1-4,096 events.
An explicitly selected cumulative numerical allowance must be positive and
cannot exceed operations multiplied by the per-operation event ceiling.
These are bounded replay-event reservations, not CPU measurements or estimates
of model safety.

Call `port.next(request, operation)` with a nonzero original external request
ID and a nonzero operation ID. A known `FileAbandonmentPlan` contains the
complete `FileHoldDiagnosisBasis`, original stage, operation and optional
block reason. Its `request()` returns a `FileAbandonmentRequest` only when
the original stage and remaining work allow an attempt.

Next is an observation. It spends one query but reserves no operation,
performs no replay, consumes no admission snapshot, and does not discover an
endpoint outcome. Unknown or unavailable requests remain typed Unknown.
An already-used operation is not offered as a new mutation.

Call `port.abandon(request)` with the selected plan's exact request. Only
the original native Unknown stage is eligible. A caller-visible Unknown
observation of a still-Dispatching attempt is insufficient. Undispatched,
confirmed, cancelled, denied and already irrecoverable attempts cannot acquire
a fresh disposition through this operation.

The complete predecessor binds scope, external request, request generation,
native attempt, acknowledged journal revision, control sequence and recorded
time. The original authority epoch is read from that same exclusively held
native cut. A stale predecessor refuses before entering the native transition.

## Original effects and conserved work

The capability calls `FileOversight::abandon_unknown` with the original
`FileAbandonRequest`. The underlying durable owner remains the single owner
of the effect state and the durable abandonment operation namespace. An
operation already recorded by direct native use or another grant cannot become
a fresh mutation under a replacement grant.

A successful scoped `FileAbandonmentReceipt` records the before and after
cuts, operation, prior Unknown stage, final IrrecoverablyUnknown stage, and
the selected effect's original `charged_units`. These units describe the
retained liability, not a refund or an available balance.

One successful entry appends exactly one original abandonment event. The
available, reserved and total charged balances remain unchanged, as do the
recorded clock and authority epoch. No endpoint request, resend, seal, source
read, helper review, human approval or fresh effect clock is introduced.
Native abandonment withdraws the selected retained sendable envelope; it
does not release another request's reservation, clear a stream obstruction,
or erase an unresolved Stop liability.

The complete retained history plus the proposed event must fit the existing
per-operation replay bound. Learned decoder histories and raw-frame predictor
histories additionally require the independently selected cumulative numerical
allowance. The ordinary constructor cannot spend numerical work. A short
allowance refuses before snapshot loss or mutation. The original numerical
recipes and work limits remain authoritative.

Before native entry, the port binds the exact operation, spends its mutation
allowance, reserves required numerical replay work and invalidates the original
admission snapshot. A storage failure or interruption cannot return those
allowances, rerun entered work or manufacture an acknowledgment.

An exact retry under the same live grant and healthy original owner returns
the cached acknowledgment without another event, replay or snapshot
invalidation. Changed input under the same key conflicts. A faulted owner
does not expose cached acknowledgments as current evidence. An entered
operation without an acknowledged result cannot be executed again through
that grant.

The scoped cache has live-grant lifetime. The original native disposition
remains durable and can be inspected through the existing canonical recovery
path. Reopening an owner or provisioning a new grant does not recreate the
scoped acknowledgment or renew an old operation's identity.

## Validation boundary

The authored native regression sources accompany the implementation. Their
positive cases use the original publication and abandonment machinery;
negative cases cover predecessor, stage, custody, work, retry and persistence
boundaries. Source review is not execution evidence.

The required command `RCH_REQUIRE_REMOTE=1 rch exec -- cargo
+nightly-2026-09-08 run --locked -p xtask -- check` was attempted before
implementation and returned exit 127 because `rch` was unavailable. The
workspace then became unavailable before any edits could be syntax checked
or executed. The new source was prepared through the GitHub connector.
No rustfmt, compilation, runtime-test, Clippy or full-gate pass is claimed
for this increment. Existing packet and production qualification states
remain unchanged.
