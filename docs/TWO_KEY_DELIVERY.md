# Two-key execution and delayed delivery

Implementation scope: the in-memory `fa-reference` publication protocol. This is the execution-boundary part of FA-121 / plan section 9.11, serving FI-A06, FI-A13 and FI-A16. It does not complete the human-member adapter, human authentication, durable co-signing or production effect mediation. The FA-121 beads remain open.

## Contract

A successful congress decision and a separately provisioned human approval are both required by the protected oversight broker. Consuming these keys at broker dispatch does not extend either validity window. First publication at the registered endpoint must occur before **both** the original action deadline and the human approval expiry. The interval is half-open: execution at the expiry tick is refused. An endpoint clock preceding the recorded approval time also refuses without claiming terminal nonexecution.

`PublicationRequest::deadline()` remains exactly the reviewed action's deadline. `execution_deadline()` returns the narrower bound. The request carries only the second key's request ID, reviewer ID, issue tick and expiry tick. No review input, witness, helper vote, actor checkpoint or argument to the reviewer is added to the endpoint projection. Copying this metadata does not mint a key or a dispatch envelope.

The broker retains the approval alongside its original action record. Recovery queries reproduce it exactly; receipts bind it through complete request equality; receipt and query admission compare it against the retained record. A restart cannot silently drop the shorter deadline or replace the original key identity.

An endpoint that has already published returns its retained executed receipt even after the keys expire. It never turns a real execution into a refundable failure. A first delivery after either deadline instead records `NotExecuted { DeadlineElapsed }` without changing payload, resource version or stream prefix. Only that terminal endpoint evidence permits the original resource charge to be returned, once. The human key stays consumed in either case. A missing status, elapsed clock or local cancellation alone never refunds an unknown effect.

## Resolving messages that never arrive

`PublicationEndpoint::resolve_expired(&StatusQuery)` is a conditional, endpoint-backed sealing operation. It is deliberately distinct from read-only `status`. It validates the exact endpoint brand, scope, resource, request binding, current fence and retained time window. An already recorded terminal outcome wins, including an execution whose acknowledgment was lost. Otherwise it refuses before the narrower execution deadline and, at or after that deadline, atomically records nonexecution and prevents a delayed copy from publishing later.

The broker consumes this receipt through its existing receipt/reconciliation path. Calling the endpoint does not itself alter broker accounting. Repeated receipts cannot refund twice, resurrect a human key or reset the endpoint's version. An elapsed retention interval still refuses; it is not evidence that a missing message never executed. Restarted dispatchers use fresh-fenced queries with the original second-key metadata, not fresh approvals or rewritten action deadlines.

This operation also serves the ordinary single-key profile using its original action deadline. It adds no scheduler, automatic retry, network transport, clock authentication or disk durability.

## Bounded recovery sweep

Both `DeliveryBroker` and `OversightBroker` expose `reconcile_pending(&mut PublicationEndpoint)`. The operation preflights the registered endpoint identity, confirmed dispatcher fence, matching endpoint epoch and observed endpoint clock before processing the bounded set of retained in-flight obligations. It retrieves terminal receipts, conditionally seals expired missing requests, and applies only those endpoint-backed outcomes through the existing accounting path. Still-live missing messages and expired retention remain charged and become or remain `Unknown`.

The result is a deterministic map from attempt ID to an individual `Result<EndpointStatus, Error>`. Callers must inspect each result. Per-attempt refusals do not erase already completed progress or prevent later attempts from being processed. Outer errors are preflight failures; this is not an all-or-nothing remote transaction. Successful terminal records drop out of subsequent sweeps. No operation resends an envelope, reruns a helper, requires a retained reviewer role, mints a replacement key, or restores authority from an actor checkpoint.

## Verification status

Added public controls cover first delivery immediately before, at and after human expiry; unchanged frozen-action bytes; exact receipt metadata; delayed acknowledgment after real execution; missing-status accounting; dispatcher restart and sealing; and the unchanged single-key profile. Constructor controls include zero IDs, empty/reversed windows and `u64::MAX` tick boundaries. A compile-fail example checks private metadata construction.

The expiry-reconciliation controls cover pre-expiry refusal, missing-message resolution at the exact deadline, one-time refunds, delayed copies, retained execution precedence, new-fence recovery, expired retention, and identically numbered foreign endpoint/broker brands. Sweep controls compose executed, expired and still-live requests in one broker; preserve residual liability across repeated sweeps; require fresh fencing; refuse foreign endpoints; retain expired-retention uncertainty; and reconcile despite unavailable helper inputs and a dropped reviewer role.

These additions have **not been compiled, executed, formatted by rustfmt or qualified through RCH in this editing environment**. It has no Rust toolchain or RCH access. Source review is not execution evidence. Existing dated execution receipts and test counts are unchanged and do not qualify this increment. No dependency, runtime or release admission is changed.
