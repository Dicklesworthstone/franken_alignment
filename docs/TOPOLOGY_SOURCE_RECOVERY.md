# Rebinding source custody after mediated recovery

## Implemented source boundary

`FileMediationObserver::bind_withdrawn_topology_source` binds a new process-local
source custodian to the exact original graph after coverage has been withdrawn.
Its consumer is a supervisor continuing publication after guarded recovery. An
ordinary source bind requires available coverage and cannot perform this handoff.
This explicit constructor instead requires unavailable coverage, no accepted cut,
no terminal stop, the original owner issuer/revision, and the independently
selected exact former graph. It accepts only distinct absolute source paths with
no parent traversal. It does no file read, certification, update or clock work.

The custodian starts above the largest topology-update operation in the original
acknowledged journal. That includes out-of-order update IDs and earlier withdrawals,
not only the most recent update. Exhausted ID space refuses before binding. This
prevents later automatic source-loss withdrawal from colliding with a retained
operation after restart. The original live-bind API remains unchanged.

Use the same source's existing `rebind` with an independently supplied newer graph
AND inventory generation. The original revoking update remains the sole authority
transition. The paths, checker allowance, source limits and work stay on this same
custodian; a corrected retry cannot silently change them. Neither binding nor
rebind certifies a cut. A subsequent actual file refresh must pass the original
checker, and `FileDriverTopology` must retain this source for first publication.
No caller-supplied certificate, new ledger, new journal encoding or executor exists.

## Cost and trust

Binding scans at most the original bounded event count to find the native update
floor and stores one bounded loaded topology. It does not replay or write history.
Rebind, refresh, source-loss quarantine and driver publication use their original
costs and persistence barriers. A refused or uncertain update returns no new source
eligibility; native faults remain faults, not permission to use the former cut.

This is a **new explicitly admitted source lifetime**, not recovery of an old
source's unpersisted read counters. Its zero work counts describe only subsequent
checks in this custodian. Neither this API nor the original source binder supplies
a durable deployment-wide observation quota. Independent input custody, trusted
paths, provenance, hardware durability and operating-system containment remain
separate requirements. A declared graph does not establish an actual OS perimeter.
No effect, permit, helper answer, evidence freshness or refund is created by binding.

## Verification status

Six authored native regressions exercise original anchored mediated recovery,
withdrawn-only behavior, explicit newer rebind and driver transfer, fresh operation
IDs on actual file loss, foreign/stale/mismatched binding, corrected rebind and exact
retry, all five original replacement faults, and exhausted operation space.
The targeted RCH launch in this editing environment stops before compilation because
`rch` is absent. Compilation, runtime tests, rustfmt, Clippy and the full gate remain
unexecuted. This advances the FA-014/FA-072/FA-073 integration without packet closure
or a production qualification claim.
