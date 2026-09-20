# Shared-budget fleet funding

## Implemented scope

`delivery::fleet::funding::FundingPool` adds bounded in-memory delegation to the
original `DeliveryBroker` (plan sections 8.5 and 15.1). `FleetCoordinator` still
owns fence propagation, not money. The pool owns its child brokers instead of
creating a second permit, policy, congress, outcome or rights algorithm.

A supervisor initializes one trusted parent budget, then `fund_domain` debits
exactly `ControllerConfig.total` before exposing a limited `FundedDomain` borrow.
Allocation IDs are the child authority IDs, tenant-bound and never reused. The
pool cannot be cloned, restored from an inspection, topped up, or used to remove
an independently spendable broker. A rejected allocation does not debit money.

The inspectable conservation law is:

```
parent unallocated + child available excluding returns + reserved + charged = total
```

`collect_returned` requires a permanent original child stop and returns only
original available rights not previously collected. Undispatched reservations can
return on stop. Sent, unknown, irrecoverable and executed effects remain charged;
only an original endpoint nonexecution receipt can release a sent charge. A local
stop is NOT endpoint acknowledgment: a delayed envelope may still execute. The
child ledger and all attempt/receipt tombstones remain retained after collection.
A stopped child's raw ledger includes returned historical availability; use the
pool inspection for the non-duplicated aggregate.

## Usage

Fund each child with its normal `ControllerConfig` and `PublicationEndpoint`.
Through `pool.domain(authority)`, observe time, confirm the endpoint fence, propose,
review, authorize and dispatch using the existing APIs. To reclaim, request the
original permanent stop, collect available units, then use `progress_stop` with
the original endpoint. Collect newly available units after receipt reconciliation.
No inspection or return receipt is a permit, funding token or restart image.

## Coordinated shutdown and original fleet enforcement

`request_stop_all(PoolStopRequest)` closes the parent before visiting any child.
A child's stop failure cannot leave proposal, positive review, authorization,
dispatch, fleet enrollment or new funding open through the limited borrow. The
parent remains closed with no resume operation. Exact retries preserve the stop
identity, including after funding-return revisions change; already-stopped
children retain their own original stop receipts. Stopping itself consumes no
funding revision, so an exhausted accounting counter cannot keep admission open.

`progress_stop_all(&mut endpoints)` visits every allocated child, not just the
supplied endpoint keys. It separately reports stop, endpoint and collection
results. Missing/foreign endpoints and original per-attempt errors remain visible.
Successful settlement in one domain is never rolled back because another domain
failed. Unused rights can be collected after a successful local stop despite
endpoint failure; sent charges still require original terminal evidence. The
`drained()` observation requires every domain to finish endpoint settlement and
collection. An executed charge can remain after a successful drain.

`FundedDomain::join_fleet` and `install_fleet_fence` delegate to the original fleet
implementation. Its issuer, scope, predecessor, shared-clock lease and revocation
checks remain in the actual delivery broker. A fleet fence acknowledgment or
lease expiry alone never satisfies the pool's permanent-stop collection rule.
The original `fleet_observation` remains available through the read-only broker.

The parent revision tracks funding/returns, not child control or endpoint I/O.
Inspect `admission_closed` separately. A returned sweep is historical telemetry,
not an imported authority token or a durable recovery image.

## Verification and limits

Ten authored unit tests exercise shared reservations, atomic admission failures,
stopped-child reallocation, delayed execution, sealed nonexecution, duplicate
receipts, retention loss, cross-domain capability substitution and integer limits.
Ten additional tests cover multi-domain stop/drain, missing and foreign endpoints,
original stop overflow, exact stop retries, accounting-revision exhaustion, empty
closure, fleet-fence integration, shared-clock lease expiry and positive-review
refusal after a failed child stop. Two compile-fail
examples cover cloning and mutable-inner escape. These Rust tests
are **UNEXECUTED**: `RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-08 run
--locked -p xtask -- check` exited 127 because `rch` is unavailable. Rust, Cargo and
rustfmt are also unavailable. Static lexical/delimiter and whitespace screens are
not compilation, formatting, Clippy or runtime qualification.

This is not durable parent escrow, cross-process allocation, authenticated fleet
funding, global bootstrap uniqueness, or production admission. An independently
created pool is a separate trusted bootstrap, not money from this pool. Existing
production/runtime admission and independent review obligations remain open; no
bead is closed by this increment and no dependency or workspace member is added.
