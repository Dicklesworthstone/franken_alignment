# Admission stop, endpoint fence, and outcome draining

Consumer: the supervising host shutting down an existing DeliveryBroker or
OversightBroker. This implements the stop-before-effect distinction from plan
sections 1.1 and 8 over the original authority ledger, dispatcher fence and
endpoint receipt protocol. No new executor, dependency or authority is added.

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
These observations are supervisor data, not new actor commands or authority.

## Verification and limits

`delivery_stop.rs` adds ten regression functions covering both key profiles,
delayed execution before acknowledgment, mixed outcomes, exact retries, stale
predecessors, foreign endpoints, restart fences, retention loss, abandoned
obligations and source loss. A compile-fail example prevents StopReceipt from
being substituted for EndpointReceipt. The existing listener test's root-level
FilePublicationLimits import is supported by re-exporting the same existing type.

The Rust code and tests have not been compiled or executed in this editing
environment, and no RCH qualification or production-gate claim is made. Existing
execution receipts do not validate this increment. This remains a bounded
reference controller with a local filesystem endpoint option; whole-process
authority recovery, hostile-process containment and reversing executed external
effects are not provided. No br-managed task is closed by this increment.
