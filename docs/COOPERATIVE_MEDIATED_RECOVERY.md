# Cooperative recovery of the original mediated owner

`FileOversight::begin_open_mediated_guarded` returns `FileMediatedRecovery`.
The anchored variant, `begin_open_mediated_guarded_anchored`, requires the
independently retained exact history prefix before any event replay.

The cursor retains the original writer lock and one canonical image. Call
`advance(progress().replayed_events, quantum)` to execute at most `quantum`
original journal transitions. Stale cursors and invalid quanta do no work.
The existing guarded cursor latches transition and final-validation failures
or unwinds. There is no exported inner cursor, machine, owner or role.

Initial and current topology, pre-fence availability, optional prediction and
evaluation, every original guard, effective policy, credential epoch and external
floors are still checked. Existing reserve selection is available only before
replay and cannot create or refill a reserve. A valid anchor never excuses an
invalid suffix. Learned-generation recovery retains its distinct recipe-bound
entry points; this API does not import or resume learned state.

`Ready` is not permission or persistence. `finish()` rechecks the complete
requirements and consumes the original single-fence replacement path without a
second history replay. Only an acknowledged replacement returns the original
owner and separately held mediated roles. The fence withdraws earlier graph
coverage and approvals. Old topology custody cannot bind the new issuer; a newer
graph/inventory and fresh native cut are required before new mediated work.
Pending effects remain original reconciliation obligations, never resent effects.

Cancellation or dropping the cursor preserves canonical and staging bytes and
releases the lock. A changed canonical image is rejected before cleanup or
replacement. The existing synchronous `open_mediated_guarded` now consumes the
same cursor; it is not an alternate recovery implementation.

File reading, decoding, configuration/anchor checks and final persistence are
still synchronous. One original event can include numerical work. Event quanta
are not wall-clock bounds, throughput measurements or proof of containment.
This extends the FA-072/FA-073 mediation/recovery work (plan 15.2/15.6/17.1,
FI-A02/FI-A06); it does not claim deployment or production qualification.

Eight actual-journal regression functions and two compile-fail examples are
authored for bounded replay, exact binding, cancellation, old-role rejection,
anchored forks and invalid suffixes, interruption, canonical replacement races,
and all five native storage barriers. Rust compilation, tests, formatting and
Clippy have not run: the required remote-only `rch` launcher is absent in the
editing environment. No bead is closed on authored tests alone.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::guarded::recovery::mediated
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Reconcile old deliveries in the same acknowledged recovery cut

`FileMediatedRecovery::finish_reconciled(clock)` consumes Ready into the original
Fence -> Time -> Sweep replacement. It returns `ReconciledMediatedOwner` with
`owner`, the separately held `roles`, and every original reconciliation `outcome`.
No intermediate owner, second replay, source acquisition, approval or resend is
introduced. An unresolved native result remains explicit rather than becoming a
success because the recovery returned an owner.

The complete mediated requirements are checked before the single trusted-clock
call. Both original canonical-image checks remain, including the check after
clock acquisition. All three events require ordinary capacity; this cannot spend
the terminal-only reserve. Failure or unwind before acknowledgment returns no
candidate owner, role or settlement. A successful recovery still leaves topology
coverage withdrawn and requires fresh qualification before new work.

Six further actual-journal regression functions reuse the original two-key
publication fixture. They compare all three delivery dispositions across anchored
and unanchored replay and three event quanta against original Machine transitions,
encoded bytes and per-attempt outcomes. Additional controls cover consumed old
approvals, incomplete/mismatching/capacity preflight, stale and unwinding clocks,
canonical changes during the final clock, and all five storage barriers with
post-failure recovery. Fourteen regression functions and two compile-fail examples
are now authored in total, NOT executed. The second targeted and full RCH launches
also stop before compilation with exit 127 because `rch` is absent.
