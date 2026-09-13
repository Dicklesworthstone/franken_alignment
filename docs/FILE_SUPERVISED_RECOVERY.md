# Durable driver recovery, cancellation and direct-child custody

Consumer: FileSupervisedDriver operating the original FileOversight through its
existing actor gateway. This extends FILE_SUPERVISED_DRIVER.md with query-only
recovery and the actual executable-worker path from FILE_HELPER_PROCESSES_REFERENCE.md.
No new journal event, alternate reducer, authentication scheme or runtime is added.

## Recovery never recreates a send path

The journal remains authoritative, not a persisted copy of the driver's phase.
After FileOversight::open performs its original recovery fence, move that owner
into a new driver. Existing ports and tickets do not retain the old lock; a new
port reacquires request visibility by exact submission retry. Neither replay nor
that retry provides an automatic permit, human approval or active helper round.

Every driving job also pins the exact original writable-owner brand. Replacing
the trusted gateway host cannot redirect an existing job by matching numeric IDs.

resume_reconciliation accepts only an original Dispatching/Unknown request. The
resulting job contains no helper input, worker, permit or review predecessor.
The next normal driver step observes an explicit current tick and invokes only
the original outcome query/reconciler. An executed receipt retains its charge;
missing/expired status remains uncertain. There is no provider callback, helper
rerun, replacement approval or publish operation on this path. A successful query
ends that local job even if unresolved; further reconciliation is explicit.

Undispatched work cancelled by reopening cannot be resumed this way. Fresh work
requires a new external key and the original full-input review and independent
human approval. Old role/key brands still refuse. The original human context can
also be used only once: an expired human request cannot be renewed by choosing a
new key number under the same review. Cancellation followed by new reviewed work
is distinct from reviving the old request, and the driver preserves that rule.

Before an attempted publication, the driver already changes to reconciliation.
Storage failure cannot make a later driver turn publish again. Reopening after a
failed staging write restores the actual canonical history, including its unknown
charge, not a guessed nonexecution. Endpoint sealing is still the only refund
path for an original dispatched-but-unexecuted effect. An externally committed
fence or unknown disposition also sends a pending driver job directly to queries.

cancel_active delegates to original request cancellation. It releases only
undispatched reservations. A sent request becomes query-only locally; this is not
proof of nonexecution and does not delete its key. request_stop and progress_stop
use the original durable stop and endpoint sweep. Invalid preflight preserves a
healthy job; a committed stop or ambiguous storage closes local driving without
returning a candidate refund. Pending-obligation sweeps do not require helper data
or the human role and remain available independently of active review work.

## Real executable helpers, one retained cohort

start_process_review takes the same FileDriverLaunch with operator-supplied
HelperProgram values. It calls the existing begin_helper_processes, whose leased
round is committed before spawn and whose post-launch clock/cutoff checks precede
input delivery. Process errors report original admission and failure diagnostics;
any partially started child owner is retained by the DRIVER for inspection/reaping,
not silently discarded with the error. No manual-vote fallback is introduced.

Socket-backed and process-backed jobs use the same review/dispatch/publication
step implementation. Completion, rejection, original cancellation, stop and storage
failure retire their worker set. The original child owner requests termination
and is polled without blocking waits or new threads. Actual exits are reported by
PID; requesting termination is not called reaping. Unreaped driver-owned children
block another helper launch, including a socket launch. Direct privileged host
operations remain the trusted operator's responsibility, not a second sandbox.

reap_helpers runs before and after normal steps and after lifecycle operations.
It examines original request dispositions before fallible clocks/providers, so
cancelled or faulted work cannot keep owned helpers running solely because the next
operation fails. A cancelled job still produces its original Stopped event; child
cleanup does not manufacture a congress decision or erase an unknown effect.
Idle means no active driving job, not that retiring children have all exited.

release explicitly returns the SAME FileActorSupervisor plus any retained
HelperChildren. It withdraws helper I/O, requests termination and preserves both
ownership duties without modifying ledger outcomes. Dropping a driver/child owner
is only the existing best-effort cleanup, not a reaping or shutdown guarantee.
Endpoint draining, direct-child exit and hostile-process containment are distinct.

## Source coverage and remaining scope

Seven lifecycle scenarios cover all-owner recreation after review/reservation/
dispatch/publication; actual staging-file failure; fencing before publication;
pre- and post-dispatch cancellation; invalid/valid stopping; and expiration at
first endpoint execution with acknowledgment-gated refund; and rejection of a
replacement host with matching numeric request IDs. Recovery is followed
by fresh reviewed/co-signed publication as a positive control. These recreate the
owner objects; they are not an executed process-kill or power-loss experiment.

Five additional process scenarios and their child entry point exercise real
inherited-socket review through publication, PID-confirmed cancellation cleanup,
storage-failure cleanup, partial spawn custody, and explicit ownership handoff.
The parent waits for atomically written PID readiness before its live-child tests;
child fixtures use the original HelperClient and synthetic fixed verdicts.

Together with the first driver increment there are twenty scenarios across
twenty-one Rust test functions and one compile-fail example. The initial new expiry
fixture was corrected to respect the original context-deduplication rule; the
existing human gate and its assertions were not altered. No older test body,
journal format, dependency manifest, license, qualification or Beads state changed.

Rust compilation, formatting, Clippy and these tests have NOT run. The required
RCH invocation could not start because rch is absent. Source hash/diff inspection
is not executable Rust validation. This is still the bounded operator-controlled
Unix file-publication profile, not authenticated storage, anti-rollback recovery,
full hosted-decoder persistence, trained-helper qualification, independent process
watchdog service or descendant/hostile-process containment. Journal writes and
replay are synchronous and not bounded by the socket I/O counters' latency.
