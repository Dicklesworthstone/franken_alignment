# Resolve shutdown acknowledgments without allocating another visit

## Original consumer and boundary

`FileShutdownCoordinator::resolve_shutdown_visit(revision, visit)` completes a
previously admitted shutdown visit from fresh canonical evidence. `visit` is the
zero-based index in `report().visits`. This closes the finite-budget case where
the final allowed visit reached the domain but its coordinator completion was
lost. It serves the existing two-store shutdown/recovery path; it is not a new
permit, endpoint, general transaction importer or command-retry API.

The method accepts retained `Advance` and `RecoverStopped` visits. It reuses the
original canonical reader, including the exact registered profile, latest whole
history-prefix comparison, head budget, numerical-bootstrap restrictions and
native semantic replay. The canonical image must contain the original stop for
this campaign's operation. No domain owner, reviewer, effect key or new clock
observation is created. Reading a saved intent cannot initiate a missing stop.

For an `Entered` visit, the same slot becomes `Observed` only after coordinator
replacement is acknowledged. Revision increases by one; visit count is unchanged.
This preserves the existing `revision = visits + completed visits` encoding and
both legacy coordinator formats. No new wire version or event is introduced.
A later successful observation for the same domain blocks resolution of an older
slot, preserving the ordering of independently acknowledged comparison cuts.

For an already `Observed` visit, fresh evidence may repopulate the current-session
report without a write only when it is still the exact retained canonical cut.
Thus a coordinator completion that reached disk despite a lost acknowledgment can
be confirmed even at the visit ceiling. A newer image refuses this read-only path;
an ordinary new visit must retain that newer comparison floor. Every attempted
read still uses the bounded in-memory campaign attempt allowance.

## Refusal and uncertainty

A missing, rolled-back, substituted, unstopped, newer-than-recorded or conflicting
canonical image is not shutdown evidence. A returned read refusal leaves the
original durable visit untouched and withdraws only that domain's current-session
success. Other domains can still progress. The current-session report records the
failed read; it is not an additional durable visit or a synthesized receipt.
A caught unwind or failed coordinator replacement retains unavailability until
reopening. Reopen reads the actual pending-or-completed image, not the failed
call's assumed outcome.

A successful resolution means that the original matching stop was observed, not
that its drain completed. The native `StopProgress` and sweep keep unresolved
liabilities explicit. No publication is resent and no uncertainty is refunded.
The method neither grows exhausted byte budgets nor repairs a domain that never
stopped. It does not authenticate operator-owned files, kill a live process,
prove simultaneous fleet stopping or detect rollback of all independent records.

## Implementation and verification status

Added the source implementation and seven regression tests over actual domain and
coordinator files. The tests cover the final visit plus final domain event slots,
unstopped evidence and a matching stopped control, all five coordinator completion
barriers, exact-cut read-only refresh, disappearing/newer images, retained-prefix
rollback, conflicting stop identity, stale and missing indices, ordering after a
later successful visit, and stopped-but-undrained observations. Test policies are
synthetic fixtures, not helper inference or hardware power-cut qualification.

The required command was attempted in this session:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It exited 127 because `rch` is unavailable. Rust, Cargo and rustfmt are absent.
Compilation, formatting, Clippy and these Rust tests are **unexecuted**. Source
hash and lexical/delimiter/whitespace checks are not substitutes for those gates.
No production qualification, historical test count or Bead closure is changed.
