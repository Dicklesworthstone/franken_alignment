# Admission stop, endpoint fence, and outcome draining

Consumer: the supervising host shutting down an existing DeliveryBroker,
OversightBroker, ActorSupervisor or SupervisedDriver. This implements the
stop-before-effect distinction from plan sections 1.1 and 8 over the original
authority ledger, dispatcher fence and endpoint receipt protocol. No new
executor, dependency, effect ledger or actor-facing command is introduced.

## Three different observations

`request_stop(StopRequest)` permanently closes local admission. The exact
operation key, control predecessor and authority epoch bind idempotency. Exact
retries return the original StopReceipt; conflicting reuse of a key refuses.
Only undispatched reservations are refunded. Sent requests become or remain
Unknown, the dispatcher epoch advances, and the prior acknowledgment is stale.
All preflight checks precede the original authority's atomic fence operation.
This call performs no endpoint I/O and does not claim remote nonexecution.

`progress_stop` installs and acknowledges the CURRENT dispatcher fence before
resolving requests. Until this fence reaches the endpoint, a delayed envelope
can still execute despite local suspension. Failed fence writes leave the stop
latched, the fence unacknowledged, and unknown effects charged. An endpoint with
the same numeric resource IDs but a different issuer cannot satisfy the barrier.

After acknowledgment, the sweep queries every recoverable original dispatch.
An existing execution wins. Missing requests are atomically sealed at the
endpoint, even before their original action or human deadline. Only a terminal
receipt can release the original charge, once; no dispatch is resent and no
replacement key is minted. Per-attempt failures do not discard another item's
progress. Host clock observations remain a separate explicit prerequisite.

StopProgress::drained requires an acknowledged fence at the current dispatcher
epoch, no unresolved dispatches and no reservations. Historical executed charges
may remain nonzero. Expired retention and IrrecoverablyUnknown records remain
visible and prevent draining; the latter cannot disappear merely because normal
reconciliation excludes them. Restarting the dispatcher requires a new fence.

Proposal, review creation, permissive review application, authorization and the
shared dispatch path check the permanent stop. Source repair, policy changes,
actor resets and human approvals cannot reopen it. Missing source capture,
helper evidence or reviewer availability cannot block stopping or reconciliation.
These progress values are supervisor observations, never live capabilities.

## Actor intake and retained observations

ActorSupervisor::request_stop closes new mailbox admission in the same local
handoff as stopping the original broker. Queued requests become
CancelledBeforeDispatch without manufacturing ledger attempts. Accepted work is
projected from the original ledger. Dispatched work remains Unknown until a
terminal receipt arrives; it is never reported as cancelled-before-dispatch.
The mailbox stays live for polling, exact retries and late outcome receipts.
Original request keys, terminal tombstones and conflict checks remain intact.

Stopping through broker_mut stops the ledger immediately. The next supervisor
synchronize or accept_next handoff also closes intake and cancels its remaining
queue, without depending on a fresh evidence snapshot. Use the supervisor's
request_stop for the combined handoff rather than leaving that propagation to a
later call. Existing wire/peer sessions all retain the same closed ActorPort;
reconnecting a transport cannot reopen its original admission domain.

## Active and disconnected driver ownership

SupervisedDriver::request_stop stops the original supervisor first, then releases
its active review or retained permit and requests cleanup through the existing
HelperChildren owner. It does not wait for a helper reveal, human key or evidence
refresh. A projection error after a successful ledger stop cannot retain the
active job. Cleanup does not imply that a direct child has exited.

Driver progress_stop(now) also releases jobs stopped through a lower-level owner
and synchronizes mailbox closure BEFORE fallible clock or endpoint work. A clock
refusal cannot keep a stopped review running. Cleanup is polled on success and
error. DriverStopProgress reports review release and helper reaping separately
from effect draining; quiesced requires all three.

Normal reap_helpers, both connected and offline, independently observes the
original controller's stop latch and releases its stopped job before maintaining
children. Consequently ordinary step, receipt handling and a failed reconnect
cannot keep stopped helpers running merely because a clock or endpoint operation
refused. No hidden blocking wait, new reaper thread or replacement child is
introduced. Hosts still poll the retained child owner until actual reaping.
Stopping child processes alone is never an endpoint acknowledgment or refund.

OfflineDriver::request_stop retains the original stopped controller and mailbox
while the endpoint is absent. Reconnection moves those same owners back into a
driver, establishes a fresh fence and preserves admission closure. No reserved
permit is resurrected, no helper is rerun and no effect is resent. Endpoint files
and their surviving recovery key use the existing persistence protocol; the
control authority itself is not reconstructed from disk.

## Verification and limits

Five new suites contain twenty-three regression scenarios: ten in delivery_stop,
four in actor_stop, two in filesystem_stop, four in supervised_stop and three in
supervised_stop_processes. The fifth suite also contains one test-subprocess
entry point, not an independent fourth scenario; there are twenty-four new
Rust test functions in total. They cover both key profiles, delayed execution
before acknowledgment, mixed outcomes, exact retries, stale predecessors, foreign
endpoints, restart fences, retention loss, abandoned obligations, source loss,
queued/reserved cancellation, late receipts, lower-level stop propagation,
active-review shutdown, missing human keys and offline reservation cleanup.

File cases block an actual pending-file write, reopen the original endpoint and
distinguish executed from unexecuted outcomes without a duplicate publication or
refund. Process cases wait for child-published PID readiness before stopping,
require the same direct-child IDs to be reaped despite stale-clock failure or a
failed reconnect, and include a working process-review/publication control before
shutdown. Their fixed fixture verdicts are not model evaluations. A compile-fail
example prevents substituting StopReceipt for EndpointReceipt.

The existing listener test's root-level FilePublicationLimits import is supported
by re-exporting the same existing type. No dependency manifests, lockfiles,
production admission registries, licenses or historical execution logs changed.

The Rust code and tests have not been compiled or executed in this editing
environment, and no RCH qualification or production-gate claim is made. Existing
execution receipts do not validate these increments. This remains a bounded
reference controller with a local filesystem endpoint option; whole-process
authority recovery, hostile-process or descendant containment, and reversing
executed external effects are not provided. No br-managed task is closed by
these increments.
