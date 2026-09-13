# Durable reset during active execution and process recovery

Consumer: FileSupervisedDriver using the original full-input, mandatory-human-key
file owner. This extends FILE_ACTOR_CONTAINMENT.md without another reset reducer,
effect sink, worker launcher, actor protocol or journal format.

The driver's reset_actor operation checks its active job's exact original owner,
then calls that host's durable reset. Existing maintenance runs after success and
failure. A committed reset cancels only original undispatched work, so its helper
sockets are closed and any original owned children are requested to stop before
future clock/provider calls. The driver preserves its next Stopped event. A stale
predecessor refusal leaves a healthy review intact. Storage ambiguity retires
workers but returns no candidate reset receipt, restored-state claim or refund.

Exact retries return historical receipts. Their old cancelled-attempt list is NOT
an instruction to cancel the active job. Retrying an acknowledged reset while a
new review is active keeps that newer review usable. Actor request keys and
redacted original ledger outcomes persist; new work uses a new key, advanced epoch,
complete current inputs and original two-key authorization. The independent human
role is neither restored nor acquired by reset. Actor memory never contains current
policy or revocation authority, and a later policy is not reverted by restoration.

For dispatched effects, reset preserves their original obligation and driver phase.
Legacy publication retains its dispatch-before-reset ordering. A configured
first-publication guard remains enabled and independently seals the now-invalid
basis rather than publishing it. Neither branch refunds a charged effect before
its original endpoint outcome is acknowledged. Historical execution always wins.
Reconciliation continues without helper evidence or another human key.

The original interrupted-source admission latch permits the restrictive reset
operation but is NOT cleared by it. State restore cannot launder an unavailable
source into fresh evidence or reopen effect admission. The added unit test injects
that actual latch directly and checks reset followed by refused proposal; it is
not presented as a new evidence-file or provider-authentication test.

Three new storage campaigns reuse the existing five failure barriers: stage,
write, file synchronization, rename and directory synchronization. They cover
state replacement, checkpoint capture and coupled reset/human withdrawal, with
reserved versus dispatched obligations in the last case. Before rename, recovery
must find no new operation; after successful rename and failed directory sync,
it must find the exact operation despite absent acknowledgment. The old owner
refuses state/receipt reads. Reopening never rebases a missing old-epoch operation
or increments an acknowledged reset twice. These are fault-injection scenarios,
not hardware power-cut qualification.

Five driver scenarios cover cancelled socket review and permitted fresh work;
historical retry during a new review; rejected reset with original publication;
actual reservation cleanup; legacy versus guarded dispatched outcomes; and keeping
new policy after restoring older actor memory. Some functions exercise paired arms.
The original driver/helper/human/endpoint assertions are unchanged.

The process scenario launches the actual test executable, waits for a PID-bound
marker written after reset has durably returned, then kills and waits for that
owned process. It reconstructs a new file authority with no surviving controller,
checkpoint or approval objects. Dispatched-unexecuted and already-published arms
must both recover actor memory and exactly one incident, preserve charges, forbid
resend and support fresh reviewed/co-signed publication afterward. The fixture's
cache/sampler bytes and helper verdicts are explicit synthetic inputs. This test
source does not establish numerical decoder continuation or hostile-process control.

Across both increments there are eighteen scenarios in nineteen Rust test functions,
including the process entry point, and one compile-fail example. The older storage
tests gain only a module declaration; all their bodies remain intact. Source and
blob checks are not Rust execution. The required RCH command returned 127 because
rch is unavailable; compilation, formatting, Clippy, all Rust tests and revision-
bound qualification remain unexecuted. No dependency, Beads status, acceptance
threshold or historical execution result changed.

This is the existing synchronous, operator-controlled Unix publication profile.
Authenticating actor-state observations, anti-rollback storage, an independent
watchdog, hostile-process/descendant containment and migrating the actual hosted
numerical decoder into the durable owner remain separate work. Restored memory
is never proof that an external effect was undone.
