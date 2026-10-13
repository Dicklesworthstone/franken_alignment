# Cooperative resolution of retained shutdown visits

FA-014 / plan 16.4, 16.5, 16.9 and 25.4: after a crash, a coordinator may
have only a retained Entered visit even though the domain already stopped. It
must inspect the original evidence, not automatically issue the command again.
The existing evidence-only resolver could only perform a synchronous full replay.
This increment adds event checkpoints to that actual resume path.

`FileShutdownCoordinator::begin_resolve_shutdown_visit(revision, visit)` freezes
one canonical image under the existing independent registration and newest
acknowledged whole-prefix checks. It uses the original numerical-bootstrap and
aggregate evidence-space admission. No domain writer lock is acquired: passive
resolution remains available beside a live or faulted domain owner. The cursor
borrows the coordinator, preventing overlapping operations through that owner.

`FileShutdownVisitResolution::advance(expected_events, max_events)` runs at most
the selected number of original Machine transitions. Progress reuses the original
replay-count/status type. Ready certifies only complete replay of the captured
image; it does not yet assert that the visit's stop identity or exact-cut rule
matches. No partial snapshot, stop observation, owner or approval escapes.
Original errors and unwinds latch before another poll can reuse changed state.

`finish` applies the SAME stop-operation and retained-cut checks as synchronous
resolution. A pending Entered visit is completed only through the original
coordinator replacement. An already Observed visit requires exactly its retained
cut and changes no durable bytes. A newer image is not imported as a quiet
refresh. Historical successful observations never substitute for this read.
The synchronous resolver and passive canonical inspection share this engine;
no second historical pass, alternate reducer, domain command or clock is added.

## Cancellation, interruption and acknowledgment

Explicit cancellation records only session-local negative evidence and withdraws
aggregate success. It changes neither canonical nor pending files, allocates no
new durable visit, and leaves the prior Entered or Observed result intact. Its
bounded in-memory attempt remains spent. Other members can still be visited.
Dropping or unwinding instead leaves the live coordinator unavailable until
reopen. An interrupted native transition cannot be cleared by finish or cancel.

Missing, foreign, incomplete or nonmatching evidence is a refusal, not a reason
to retry Stop/Fence/StopProgress. A stopped-but-undrained observation retains its
actual liabilities. Executed or unknown effects are not refunded by resolving
bookkeeping. A failed coordinator completion may leave a visible acknowledgment;
reopen and resolve that actual cut without changing the domain again.

The read is a captured historical image, not a simultaneous or continually fresh
domain observation. A writer can publish a valid successor while replay proceeds;
that successor does not alter the already captured bytes or result. The next
read must satisfy the ordinary retained-head rules. No domain staged file is
cleaned, and no current producer/helper/clock or stronger recovery profile is
silently substituted.

## Limits and verification

File acquisition/decoding, one original event, result construction and a final
coordinator write remain synchronous. Event counts do not measure CPU time,
heap usage or real-time responsiveness. Journal and retained-head bounds are
unchanged. This is not a parallel scheduler or distributed atomic shutdown.

Eight new native regression functions cover original-machine/quantum parity at
the durable visit ceiling; every cancellation cut for pending and observed visits;
drop and actual-event unwind; invalid polls and a stale-clock suffix; concurrent
writer progress; all five coordinator storage barriers; stopped/unstopped/wrong
operations; and independent prefix/head-space refusal. Existing tests and their
assertions are preserved. Synthetic fixtures are not production fault evidence.

Source-only status: the editing environment has no RCH or Rust toolchain.
Required targeted and full `RCH_REQUIRE_REMOTE=1 rch exec -- ...` verification
must execute on the exact integrated revision. Compilation, tests, formatting
qualification and Clippy are not claimed; FA-014 and its prerequisites remain
open. No dependency, runtime, authority or journal-format change is introduced.

## Checkpointed fleet resume

`resume_registered_pass_with_checkpoints(revision, clocks, max_events, callback)`
uses the existing full-roster/clock/capacity selector with resume semantics.
Eligible latest Entered/Observed visits take the passive cursor; other members
use the original new-recovery branch. A passive refusal never falls through to
new recovery. The checkpointed new-recovery API shares the same outer loop, and
its original domain mutation/cancellation branch remains unchanged.

Before each member, None progress precedes its current read or intent. Some
progress reports zero, each bounded quantum, and Ready before resolution or
native settlement. False ends the pass without hiding the remaining roster.
Cancelling a passive read yields session-only EvidenceRefused(Unavailable), with
no visit or file rewrite. Cancelling new recovery retains the original durable
Refused acknowledgment. Both leave later members NotVisited. Ordinary evidence
refusals allow later members to proceed; an unacknowledged coordinator write
stops the pass immediately. Callback unwinds are not converted into results.

Clocks are still bound to the complete roster, but resolution does not consume
their ticks. Reopening does not convert old success into current success or
replenish the durable visit allowance. Each fresh read uses an original bounded
session attempt, and the whole pass is admitted before any callback. A consumer
can enforce one cumulative elapsed deadline without changing source time.
There is no new CLI, implicit retry, background worker or parallel executor.

Seven additional test functions cover mixed eager/cooperative parity and exact
refresh at the visit ceiling; all 14 read checkpoints in a two-member fixture;
missing/unstopped evidence with a reachable new member; every completion barrier
before an unvisited tail; callback unwind after prior success; complete-roster,
clock, quantum and spent-attempt admission; and original two-key reserved/sent/
executed/expired receipt accounting. Existing batch/resume tests and admission
are unchanged. The combined increment adds 15 authored Rust tests; none has run
in this environment. Required target and full RCH attempts still stop before
Cargo at the absent launcher. Source/lexical/hash checks are not qualification.
