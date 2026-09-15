# Credential mediation in FileSupervisedDriver

`FileSupervisedDriver` now carries the durable credential guard through the same
state machine used by callback evidence and registered-file evidence. There is no
parallel execution loop and no credential-aware shortcut around helper review,
human approval, resource reservation or first-publication evidence revalidation.

The existing `step_with_evidence` and `step_from_file` APIs retain their original
behavior. On a credential-guarded authority they can progress review and dispatch,
but a valid first publication refuses rather than falling back to an uncredentialed
effect. As before, the driver retires its send phase before the provider is called;
a failed publication becomes reconciliation-only and is never automatically retried.

`step_with_evidence_and_credential` and `step_from_file_with_credential` use the
same private `step_with_provider` state machine with one additional borrowed
`FileCredentialPermit`. The permit is ignored in Review and Ready phases. It is
consulted only when the job reaches its first publication transition. Consequently
presenting a credential cannot mint an automatic permit, satisfy a human review,
change helper evidence, reserve units, refresh a source or advance a dispatch.

At publication, a credential-guarded host invokes
`publish_checked_with_credential`; a missing capability uses the legacy checked
path, which may still seal a restrictive result but cannot execute valid evidence.
Historical endpoint receipts, expired/nonexecution outcomes and subsequent
reconciliation do not require the credential. This prevents credential loss from
blocking correct accounting of an already admitted effect.

The registered-file variant preserves the existing source behavior. It performs
its normal durable source capture/revalidation before the publication transaction,
and only then presents the process-local credential capability. Source freshness
therefore cannot be replaced by a credential, and a fresh credential cannot renew
or repair stale evidence.

Regression source pairs otherwise identical reviewed/two-key dispatched actions:
one publishes with a credential bound to the live owner; one uses the legacy
method and is refused without execution; one receives a credential from another
durable owner and is refused by issuer binding. A separate control confirms that
reconciliation after a terminal receipt needs no credential. The registered-file
case exercises durable source refresh plus credentialed first publication.

The required RCH runner is unavailable in this environment. Rust compilation,
formatting, Clippy, tests and doctests remain unexecuted, so these source scenarios
are not execution qualification. No Beads or production-gate status is changed.
