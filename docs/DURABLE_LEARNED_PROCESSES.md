# Managed direct children for durable learned-sidecar review

`helpers::learned_sockets::processes` connects the original executable launcher
and child reaper to `FileLearnedSocketRound`. This serves the existing plan 9.2
and 10.15 consumers: external helpers receive the actual learned sidecar, and
accepted commitments/reveals and refinement results use the original journal.
No new numerical evaluator, reducer, wire format or effect authority is added.

`begin_learned_process_round` takes operator-selected programs, a frozen round
and limits, and a sealed current sidecar. It derives the attempt and input
revision from that sidecar. The original process launcher records Begin before
spawning. A later failed spawn or expired post-launch deadline leaves that
round consumed; it cannot be taken over by manual votes. A failed Begin starts
no process. An ordinary partial-launch failure returns every started direct
child with termination requested and an explicit NotStarted/Committed/Unknown
admission classification. No helper-input bytes are sent during construction.

The live owner keeps both the original source-bound socket round and its child
collection. Pumping delegates to the existing learned-source checks and durable
protocol barriers. An OS exit status never supplies a vote: already-buffered
wire bytes still need parsing, and an exit without a reveal remains missing.
Completed, failed and cancelled workers receive termination requests through
the original reaper. Completion still needs the existing automatic and human
keys, exact current-input checks and final publication validation.

The host must call `reap` until `all_reaped` before discarding the owner, even
after protocol failure. Cleanup requires no functioning journal, current source,
clock callback, action permit or available review budget. `into_children`
closes protocol sockets first and transfers only cleanup responsibility, not
a sidecar, pool or ballot interface. Cancellation does not cancel an action,
refund unknown effects or erase acknowledged review outcomes. During a live
pump/finish unwind, a guard closes the protocol and requests child termination;
the original child collection stays available to the caller for later reaping.

The original launcher performs synchronous OS setup and fresh post-launch clock
validation. It uses explicit executable paths/arguments/environment and private
inherited sockets, not a shell inserted by this adapter. Program-image identity,
process sandboxing, descendants, ambient descriptors, private diagnostic output
and clock authenticity remain supervisor obligations. Constructor unwinds and
Drop retain the original launcher's bounded best-effort termination behavior;
Drop is not a reaping guarantee or a hidden blocking/reaper-thread facility.
Nonblocking socket and child polls do not bound journal fsync or spawn latency.

Eight regression test functions and one separately launched test entrypoint
exercise real direct processes, the original HelperClient, learned numerics,
checked packets, canonical files, congress and two-key publication. Tests cover
exact packet delivery and reaping; source-loss termination; stale/foreign calls;
post-launch deadline failure; ambiguous Begin storage failure; actual partial
spawn failure; successful exit without a vote; callback unwinding; and a real
occupied-staging-file failure after the child sends its commitment. The synthetic
ballots are protocol controls, not evidence of helper-model safety or independence.
One compile-fail example prevents worker replacement through the managed owner.

All new Rust tests, compilation, rustfmt and Clippy remain UNEXECUTED. Required
remote-only RCH commands cannot start in this environment because `rch` is
absent. Selected-source/hash/whitespace checks do not establish runtime
correctness, a complete checkout, production qualification or bead completion.
