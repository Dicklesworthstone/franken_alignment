# Supervised source-completion path

`FileSupervisedDriver::complete_with_publication_source` connects the original
reviewed actor job to source-acquired atomic publication and settlement. The
caller supplies the concrete witness reader, trusted clock, committee/policy
callback, independent human permit and optional current credential permit.
The source/recipe must already be bound on the original durable attempt.

This is an additional supervised API, not a new actor command or another effect
state machine. Helper socket/process review, original action/control/input bindings,
automatic rights, human approval and the actor request book remain the existing
owners. The method accepts only a Ready job at a compatible native ledger cut;
foreign human/source/credential inputs refuse before acquisition or reservation.

When no automatic permit is retained, the method invokes the SAME private
publication-source provider and original sample/authorize path once. It then calls
`FileOversight::complete_publication_from_source`, which independently recaptures
before dispatch and first publication and settles the original receipt in the
same final effect cut. A normal completion therefore makes three independent
file acquisitions: authorization, dispatch, publication. No capture is relabeled
as fresh twice, and no second reservation is created merely to retry a read.

The returned `FileSourceCompletionReport` separates `authorization_reads` (at most
one) from the host's `completion.reads` (at most two), callback failure and final
result. A read identity is not installation/commit evidence. Only a successful
`completion.result` acknowledges the final publication together with settlement.

On success, the native request is already Confirmed or ConfirmedNotExecuted and
the driver closes the job. There is no additional publish/reconciliation step to
schedule; a later ordinary driver step is Idle and contacts no source for this job.
This avoids exposing a visible effect while leaving its original receipt accounting
for a later scheduler turn. The pre-read withdrawals and any automatic reservation
remain separate acknowledged operations; the entire method is not one transaction.

A returned pre-read error can leave the healthy original owner Authorized. The
driver then restores Ready with the SAME retained automatic permit. A retry makes
two real new acquisitions but does not authorize/reserve again. Once completion
begins, the driver retires its send phase before external callback code. A caught
unwind, quarantined owner or ambiguous final write cannot automatically redispatch.
Use the original cancellation/recovery paths as appropriate; never infer an outcome
from a storage error. Original late evidence can seal a staged effect and settle
its native nonexecution receipt without manufacturing an unknown-effect refund.

The callback supplies trusted committee/policy observations. This API does not
renew a separately configured native policy-source lease, authenticate producer
files, or make sequential observations an atomic cross-producer snapshot. Stronger
native checks still execute and can refuse. The existing paired concrete-file step
API remains available with its durable source refreshes and separate transitions;
this change does not silently switch that API to the new completion path.

`tests/file_driver_source_completion.rs` adds seven real helper-socket regression
tests: normal three-acquisition completion, unrelated versus dependency changes,
retained-permit retry after read loss, final read loss, caught callback unwind,
foreign human keys and a changed final policy snapshot despite identical witness
files. They check the canonical sink and the original actor request/rights state,
not a separate wrapper ledger. These new tests remain UNEXECUTED pending the
repository's required RCH verifier on the exact integrated revision.
