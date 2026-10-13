# Anchored cooperative investigation of stored proposals

The original guarded proposal investigator now has an explicit independently
anchored, read-only cursor. This is L7 diagnosis, not recovery or live authority.

```rust,ignore
let mut read = FileOversight::begin_read_proposal_investigation_anchored(
    directory, &profile, &requirements, &retained_anchor, attempt, scope,
)?;
loop {
    check_operator_deadline()?;
    let progress = read.progress();
    if progress.status == FileProposalReadStatus::Ready { break; }
    read.advance(progress.replayed_events, 1)?;
}
let investigation = read.finish()?;
let diagnosis = investigation.run(&interventions)?;
```

`read_proposal_investigation_anchored` is a synchronous convenience consumer of
the same cursor. Existing eager and live-owner investigation APIs remain intact.
Both paths share the original first-creation selector and original machine; an
external request ID is not assumed to equal its allocated attempt ID. The
historical admission policy, input snapshot, compiled witnesses and original
Reviewing/Denied disposition remain separate from the ending journal snapshot.
Later outcomes or policy changes do not rewrite the historical question.

The input anchor must be independently retained. Canonical decode and exact
prefix validation precede every event execution, as does the original numerical
configuration admission. A valid equal-counter fork or rollback is refused.
Complete suffix replay and ending guard, policy, credential and recovery-floor
checks precede Ready and finish. No partial proposal, mutable machine or live
role is exported. Failed original transitions or caught unwinds permanently
latch the read; a correctable stale poll performs no work. Missing attempts
remain missing rather than acquiring a fictional proposal.

No writer lock is taken; the read can run beside the original live owner. The
captured canonical image can be older than disk by the time finish returns.
Its journal snapshot identifies the inspected image, NOT current eligibility,
latest-state truth or a fresh durability acknowledgment. It does not authenticate
origin or prevent joint rollback of the journal and independently held anchor.
Drop/cancel, including at Ready, never writes a journal, fences a dispatcher,
cleans staging, refreshes source evidence or refunds an unresolved effect.

Each advance bounds original transition attempts. Initial bounded file intake,
canonical decoding, prefix comparison and final guard checks are synchronous;
one event can include substantial computation. Attempt counts are not elapsed
time, physical bytes or numerical-operation metrics. Historical replay work is
separate from the subsequent native bounded repair-search budget. Hosts should
check cancellation before opening, between events and at Ready.

Eight new regression functions use original denied proposals and compare eager
and event-bounded reads, native counterfactual results, all cancellation and
unwind cuts, unchanged journal/staging bytes, invalid ending requirements,
valid equal-counter forks and rollback, invalid suffixes, missing attempts,
and concurrent writer progress. Positive controls require the original denied
payload to be repaired hypothetically while the actual attempt remains denied.
No successful counterfactual becomes a permit or predicts helper approval.

These are unqualified sources. Required targeted and full-gate RCH dispatches
must execute before qualification; there is no local compiler fallback. This
implements plan 11.3, 12.5, 17.7 and 17.9 experiment/evidence semantics and serves
FI-A16 and FI-I11. No new dependency, runtime, journal format or authority path.
