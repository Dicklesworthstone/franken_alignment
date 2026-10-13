# Reconcile pending publication while completing guarded recovery

`FileGuardedRecovery::finish_reconciled` completes a fully replayed base guarded
cursor with the existing fixed `Fence -> Time -> Sweep` transaction. It returns
`ReconciledGuardedOwner`: the original live owner, all separately held guarded
roles and the original per-attempt reconciliation results. No partially replayed
owner, prior approval, or unacknowledged settlement escapes.

```rust,ignore
let mut recovery = FileOversight::begin_open_guarded_anchored(
    directory, profile, &independent_requirements, &independent_anchor,
)?.require_recovery_reserve(RecoveryReserve::terminal())?;
while recovery.progress().status == FileGuardedRecoveryStatus::Replaying {
    check_deadline_and_stop()?;
    let done = recovery.progress().replayed_events;
    recovery.advance(done, 1)?;
}
check_deadline_and_stop()?; // Ready is also a cancellation boundary.
let recovered = recovery.finish_reconciled(trusted_clock)?;
// Inspect every outcome, retain unresolved liabilities, and separate roles.
let ReconciledGuardedOwner { owner, roles, outcomes } = recovered;
```

This joins two existing capabilities: independent guarded cooperative replay and
restart-time reconciliation of the original publication endpoint. It is not a
new receipt verifier, event importer, remote-effect adapter or looser recovery
profile. The existing live `finish`, terminal methods, and
`open_reconciled_publication` remain unchanged.

The receipt-only operator now uses this path through
[`resume --guarded`](GUARDED_RECEIPT_RECOVERY.md). Its original submit document is
forwarded only for an existing request. `require_existing_request(request)` is
a consuming Ready-only preflight on the same cursor: it rechecks independent
requirements and original request existence without returning a ticket, payload,
outcome or authority. The original actor gateway still validates exact bytes.
Missing request keys refuse before the recovery clock or any journal mutation.

## Admission, durability and accounting

The cursor must be Ready. Its complete independent guard inventory, effective
policy, credential epoch and all recovery floors are checked again. The anchored
constructor continues to check exact retained-prefix continuity. The canonical
image is compared before the trusted clock and again before staging cleanup and
replacement. Invalid clocks, prefix/capacity failures, failed replay and callback
unwinds do not spend a preliminary fence or return a live role.

The already reconstructed original machine applies exactly three new original
events. There is no second historical replay. Every encoded prefix retains its
original event/byte admission rules. One native canonical replacement publishes
the fence, time observation and outcomes together. Replacement failure or unwind
returns no owner, role or sweep; a complete replacement may nevertheless already
be visible. A later live recovery fences that actual cut again and queries the
original endpoint. Unlike an already-drained terminal stop, live recovery is not
a zero-event or zero-clock retry.

Undispatched reservations are cancelled by the original fence. Sent effects are
reconciled only through original endpoint observations. Already executed effects
stay executed and charged. Expired unknown results remain in the outcomes map
and retain their charges. A returned owner never means every liability settled.
A fresh clock observation does not renew old source evidence or helper approval.

Time and Sweep are ordinary-capacity operations. Live reconciliation cannot
borrow the terminal-only stop reserve. When ordinary capacity is exhausted,
`finish_stopped` is still the explicitly supported terminal alternative. No
capacity limit or reserve is relaxed by this feature.

## Scope, costs and verification

This is the base guarded profile. Evaluated, predictive, mediated and learned
histories retain their separately admitted recovery routes; no failed guarded
open falls back to an ordinary opener. External scope authentication, trusted
clock/storage behavior, advisory-lock cooperation and independent nonrollback
custody remain deployment assumptions.

The implementation retains the original bounded canonical image and event list,
then one cloned event vector and one candidate image during preflight. Initial
I/O, guard checks, each original event, encoding and synchronization remain
synchronous. There is no hard cancellation-latency or remote exactly-once claim.

Eleven regression functions cover original full-replay and canonical-byte parity
across three event quanta and both prefix modes; old approval rejection paired with a freshly approved new publication;
incomplete/interrupted/cancelled recovery; independent policy/floor refusal;
stale/panicking clocks; same-counter forks and canonical changes around the
clock; emergency-reserve preservation; all five native storage barriers for
sent/executed histories; retained expired-unknown liabilities; and separate identity/governance roles without restored eligibility; and existing-request admission before recovery writes.

These tests are authored but not compiled or executed. Required remote-only RCH
verification is unavailable in the preparation environment. No runtime,
formatting, Clippy, release qualification, or FA-014 closure is claimed. This is
scoped implementation under plan 16.4/16.5/16.9, serving FI-A02; no dependency,
executor, journal format or effect authority rule changes.
