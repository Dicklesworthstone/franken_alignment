# Direct stopped completion of cooperative guarded recovery

The base `FileGuardedRecovery` cursor can now finish directly into stopped
observations, instead of first issuing a recovered live owner and roles.
The existing live `finish` route is unchanged. This source implementation is
unqualified until its required remote tests and gate execute.

```rust,ignore
let mut recovery = FileOversight::begin_open_guarded_anchored(
    directory, profile, &independent_requirements, &independent_anchor,
)?.require_recovery_reserve(RecoveryReserve::terminal())?;
// Drive the original cursor in bounded event quanta, checking cancellation.
while recovery.progress().status == FileGuardedRecoveryStatus::Replaying {
    let done = recovery.progress().replayed_events;
    recovery.advance(done, 1)?;
}
// The operator must check its deadline/cancellation again at Ready.
let request = recovery.stop_request(operation)?;
let history = recovery.finish_stopped_with_anchor(request, trusted_clock)?;
let (stopped, anchor) = history.into_parts();
```

`finish_stopped` returns only `FileStoppedRecovery`. The anchored variant returns
the existing `FileStoppedHistory`; neither returns a live owner, identity
observer, policy governor, reviewer or permit. `stop_request` is a read-only
selection from complete guarded replay, not an authority-bearing capability.
A matching historical operation reuses its exact original request.

## Boundaries preserved

Construction retains the original exclusive Store lock, bounded canonical
image and independent requirements. The full guard inventory, effective policy,
credential epoch and all recovery floors are checked at replay completion and
again before stop selection/completion. A failed or interrupted cursor cannot
select or finish a stop. Dropping it, even at Ready, changes no files. The same
original anchored constructor rejects a fork or a prefix older than the supplied
anchor. The same explicit reserve selector operates before the first event.

A new stop uses exactly `Stop -> Fence -> StopProgress`, through the existing
consuming terminal finalizer. No ordinary recovery fence spends the terminal
reserve first, and no original history is replayed again. Every event/byte limit
and source-admission law remains mandatory. The canonical image is compared
before the clock and again after terminal preflight, before cleanup/replacement.
Invalid intent or time does not clean staging. Persistence failure can leave a
visible replacement but returns no candidate receipt or anchor.

A matching already-drained stop confirms the identical canonical image and
returns an identical anchor without a clock, record or replacement, including
at full capacity. Staged bytes are never promoted. Pending or expired effects
remain liabilities under the original terminal reducer; no effect is resent.
No archive, successful stop or successful rehearsal provides production rights.

This is the base guarded profile only. Evaluated, predictive, mediated and
learned histories keep their existing separate admission rules. There is no
fallback to a weaker opener. Independent custody and anti-rollback remain host
assumptions. The private anchor contains original evidence; its export must be
separately synchronized outside the authority store. Retain the previous anchor
until that succeeds. This is not a remote-effect or OS-process stopping claim.

## Costs and verification

Event execution uses the existing cooperative cursor and attempted-event counts.
Initial read/decoding, guard checks, final encoding and synchronization remain
synchronous; one event can contain a complete decoder operation. No hard
wall-clock bound or extra numerical-work budget is claimed. The returned anchor
owns the already bounded original canonical buffer.

Seven new regression functions exercise actual identity/campaign-guarded stores:
original batch/replay parity under three poll quanta, exact retry and both return
forms; incomplete/cancelled and interrupted histories; full independent guard
and floor refusal; full-capacity reserved-tail completion; five native storage
barriers and visible-cut retry; and stale intent/time plus canonical replacement
before/inside the clock callback. Existing live recovery tests are unchanged.

Required commands are the targeted guarded tests and the unchanged xtask gate,
through `RCH_REQUIRE_REMOTE=1 rch exec`. This environment has no `rch` or Rust
toolchain; compilation, Rust tests, formatting qualification and Clippy have not
executed. No bead or full packet is closed. This implements a scoped capability
under FA-014, plan 16.4/16.5/16.9/25.4 and FI-A02; the broader admission and
production qualification requirements remain open. No dependency, runtime,
journal format, agent verb or live policy has changed.
