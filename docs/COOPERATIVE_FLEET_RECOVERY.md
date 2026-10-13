# Cooperative registered-domain stop recovery

FA-014 / plan 16.4, 16.5, 16.9 and 25.4: a registered multi-domain shutdown
must preserve unknown effects, exact history custody and responsiveness while
reconstructing a long journal. This increment reuses the original stop-only
recovery cursor inside the existing write-ahead shutdown coordinator. It adds
no authority ledger, runtime, dependency or journal encoding.

## One replay, two acknowledgment boundaries

`FileShutdownCoordinator::begin_recover_and_drain(revision, domain, at)` first
persists the original RecoverStopped visit intent. Only then does it open the
registered domain under its original exclusive Store lock. Its return value
separates an acknowledged native refusal (inner Err) from an unacknowledged
coordinator write (outer Err). An absent or busy domain remains in the fixed plan.

The resulting `FileShutdownVisitRecovery` borrows the coordinator and retains
one private `FileStopRecovery`. Each `advance(expected_events, max_events)` runs
at most that many original transitions. Progress exposes event counts/status,
not a partially replayed observation, owner, helper, reviewer or effect key.
The registered profile, latest acknowledged WHOLE prefix, aggregate head space
and original numerical-bootstrap restriction are checked before any event runs.
Passive canonical inspection and registered recovery share that admission.

The existing synchronous `recover_and_drain` now uses the same implementation.
It no longer calls passive whole-history replay followed by another replay for
terminal execution. The latest original StopProgress result is retained during
the one replay so an already-drained reread still reports its actual sweep.

`finish` consumes the verified cursor through the existing fixed
Stop/Fence/StopProgress finalizer, then acknowledges the observation separately
in the coordinator. Native event/byte/reserve limits and total retained-head
space are checked before the domain is changed. Canonical identity is checked
again before terminal mutation or cleanup. Only the acknowledged native result
and its original encoded history are installed in the coordinator.

A failed coordinator completion can follow a successful domain stop. It must
not be interpreted as nonexecution, a refund or permission to resend. Reopen the
coordinator and use the existing evidence-resolution or explicit recovery path.
An exact drained domain reread appends no domain records and uses no new tick,
even at full capacity. Supplied `at` retains the registered clock-domain meaning;
replay does not renew that observation, a deadline or any resource allowance.

## Cancellation and failure

Explicit `cancel` drops domain reconstruction without canonical or pending-file
changes, and records Unavailable as the coordinator's negative visit completion.
This is operator cancellation, not proof that the domain stopped. The consumed
visit allowance is not refunded. Later members can still be visited after that
negative acknowledgment. Dropping or unwinding without cancellation instead
leaves the durable Entered visit and the live coordinator unavailable until
reopen. A later poll cannot reuse a failed or unwound native reconstruction.

The domain lock stays held across polls and terminal completion. The coordinator
borrow prevents overlapping visits in that owner. Independent domains are still
stopped at separate local cuts; no simultaneous fleet-stop, remote process halt,
cryptographic authentication or protection from jointly rolled-back custody is
claimed. Learned profiles requiring separate numerical recipes retain their
explicit recovery routes and existing admission failures.

Construction, file decoding, an individual original event, final encoding and
storage barriers remain synchronous. Event quanta allow caller deadline/stop
checks; they are not hard real-time bounds. Logical event counts do not measure
CPU work, allocation or elapsed performance. This removes duplicate historical
transition execution, not the required ordered writes or prefix comparisons.

## Verification scope

Eight new native test functions cover quantum/count parity with passive native
observations, cancellation at every event and Ready, drop/unwind custody, stale
polls and early completion, exact prefix and head-space admission, canonical
replacement before finish, full-capacity drained rereads, all five barriers in
BOTH stores, and original reserved/sent/executed/expired liabilities. Existing
test assertions and storage-fault seams remain intact. The fixtures use actual
original journals and two-key effects, not imported successful stop reports.

Source-only status: tests, Rust compilation, formatting and Clippy are UNEXECUTED.
Targeted coordinator tests and the unchanged full xtask verifier were attempted
with RCH_REQUIRE_REMOTE=1; both stopped at missing rch (exit 127) before Cargo.
The selected-file worktree is not a full checkout. Source/hash checks are not
execution qualification; FA-014 and its original prerequisites remain open.

## Whole-roster checkpointed passes

`recover_registered_pass_with_checkpoints(revision, clocks, max_events, callback)`
uses the same side-effect-free full-roster, exact clock binding, visit budget and
revision-growth admission as the existing batch/resume methods. It then drives
the original cooperative visit per member in stable order. No subset of the
registered denominator is accepted, and progress allocates no extra visits.

The callback receives only a domain ID and optional original event progress.
None means before intent/domain I/O; Some includes initial zero, each quantum and
Ready before native terminal completion. False ends the pass. Before entry, the
member stays NotVisited. After entry, cancellation records Unavailable without
changing the domain. Every later member remains explicitly NotVisited. The pass
predicates use only this pass, never an older successful coordinator observation.
The caller may enforce one cumulative elapsed deadline; a new member does not
implicitly renew any deadline, source time or allowance.

Native refusals are recorded and other reachable members may proceed. Failure
to acknowledge either native completion bookkeeping or cancellation ends the
pass with Unacknowledged. A successful domain stop may already be visible. A
callback unwind remains an unwind: an entered coordinator retains its poisoned
Entered intent and requires reopen. The existing resume operation still resolves
historical visits without automatically retrying a refused/not-yet-stopped
member. It is not converted into a mutating checkpointed recovery pass.

Seven further tests cover batch-result equivalence and quantum counts, every
checkpoint cancellation, complete-plan admission before callbacks, offline/busy
members alongside a reachable control, callback unwind after a completed member,
failed acknowledgment of both cancellation and native success, and full-capacity
clock-free domain rereads. The original batch/resume execution loop and prior
assertions are preserved. Together the increments author fifteen Rust tests.
Required target/full RCH launches remain blocked by the missing launcher; no
compiled/tested/format-qualified status follows from source and hash checks.
This API enables embedding-operator checkpoints, not a new fleet CLI, hard
real-time scheduler or distributed stop transaction. FA-014 remains open.
