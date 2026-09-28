# On-demand executable learned-sidecar review

## Original consumer and source boundary

`helpers::learned_sockets::processes::sequence` consumes the existing
`FileLearnedSidecar` and `LearnedWorkerSchedule`, then uses the original
`FileLearnedProcessRound` for every actual launch, wire transition and witnessed
completion. This implements the pending refinement-sequence increment against
the current upstream launch API; it does not replace upstream single-round
process ownership or its tests. It serves plan 9.2 and 10.15 in L3/L4.

`begin_learned_process_review` fixes every program roster, round identity,
window, per-round helper ceiling and total poll allowance before first Begin.
The original program preflight checks every scheduled executable and directory.
The complete schedule's direct-child start count must fit an explicit allowance,
with a ceiling of 256. Each configured program retains its original bounded
arguments/environment. Future programs are not spawned eagerly.

## Acknowledgment, cleanup, then the next review

Only the first roster starts initially. Every actual protocol pass delegates
to the current original process round and its learned-source checks. A complete
abstention may buy one original residual through the existing witnessed finish,
only when a future registered window is usable. Earlier votes cannot approve
those richer bytes. The result and its disclosure costs remain acknowledged
history even if later admission or spawning fails.

After a refinement the sequence returns `Draining` and yields. The next Begin
and spawn cannot occur until every preceding direct child has been reaped by
the original nonblocking child owner. Only that same sidecar moves into the
successor; no planner or disclosure budget is recreated. Source currentness and
the fixed window are checked again before launch. At most one unreaped roster
is retained. One invocation never sends the successor's first evidence byte.

Each scheduled round has an inspection record, including unstarted rounds.
Records retain original admission status, socket progress, first wire failures,
actual child states and acknowledged completion. A partial successor spawn
retains its real child owner; previous outcomes cannot be rolled back. Process
exit is not a ballot, a completed round is not either publication key, and
cancellation does not cancel or refund an external effect.

`reap` is cleanup-only and consumes no protocol allowance. It remains available
after success, cancellation, storage/source failure or poll exhaustion. Clock
unwinds latch failure and request shutdown; original constructor-unwind cleanup
remains the launcher's bounded best effort, not guaranteed reaping. There is no
worker replacement, rescheduling, vote override or executable-owner escape.

## Verification and compatibility

Seven regression functions and a separate executable child entrypoint are
ported from the saved sequence patch. They exercise original inference, fitted
compression, source-bound sidecars, real Unix child processes, original wire,
canonical journals, residual refinement and two-key publication. Cases include
successor launch only after reaping, full inventory and exact start limits,
source/window loss after acknowledged refinement, actual spawn failure,
cancellation, exhausted polls, stale/foreign calls and clock unwinding. A real
partial two-member spawn is paired with a successful two-member control.
One compile-fail example forbids program replacement.

The existing upstream single-round API, original launcher/reaper/reducers,
numerical implementation, journal/wire formats, and all upstream test bodies
are unchanged. This module is not a native CLI, executable attestation,
process sandbox, descendant supervisor, persistent network accounting system
or global inference escrow. Synthetic ballots are protocol controls, not
empirical detector measurements. Original synchronous launch/replay/fsync costs
are not turned into a wall-clock guarantee by a bounded poll count.

All new Rust checks remain UNEXECUTED. Fresh targeted and full remote-only RCH
attempts stop before compilation because `rch` is unavailable (exit 127).
Selected-source integrity and whitespace checks do not establish compilation,
runtime correctness or production qualification. No Bead or gate is closed.
