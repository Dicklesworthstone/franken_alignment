# Observe original helper credibility and Andon accounting

The native Tower `ledger` read exposes the original durable owner's helper
credibility and false-stop accounting to an independently authorized operator.
The counters come from acknowledged native reviews and independent evaluator
assessments. The caller cannot supply a score, label, replacement ledger or
claimed calibration result.

This is the L8 governance observation described by plan sections 9.9, 15.10,
17.8 and 22.1, serving FA-105/049/134 and founding ideas FI-A06/A14/A17. It does
not add another accounting owner, change review requirements or qualify a
deployment. A calibration incident remains an observation requiring independent
governance; it never silently weakens the committee or grants publication.

## Select a separate finite disclosure grant

The trusted host calls
`FileActorSupervisor<FileOversight>::governance_ledger_port(queries)` with
1–1024 queries, retains the returned `FileSupervisorGovernanceLedgerGrant`,
and installs the returned weak port through
`PublicationTower::set_governance_ledger(Some(port))`.

Actor intake, precheck, inspection and explanation capabilities cannot create
this grant. Installation requires the exact original owner and complete scope,
not merely equal numeric tenant/run identifiers. A failed replacement removes
the previous port. Dropping or explicitly revoking the noncloneable grant
withdraws every cloned port. Dropping, replacing or faulting the original owner
cannot recover an earlier Known result.

Each entered query consumes one shared query before fallible accounting work.
Identical reads and reconnects do not replenish it. The read uses one immutable
borrow of the original acknowledged owner; it acquires no source, clock, model,
helper, evaluator role, publication key or replay worker. It writes no journal
event and preserves the actor gateway's admission snapshot. A conflicting
borrow or missing owner returns Unknown, never a stale saved answer.

## Use the registered Ledger verb

The exact run mount accepts the existing six-field Tower request:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "governance-observation-1",
  "address": "fa://1/run/3@41",
  "verb": "ledger",
  "arguments": {},
  "idempotency_key": null
}
```

The caller must possess the independent operator role and the host must have
installed the separate native grant. Tenant, run, registration generation and
argument checks precede native spending. The immutable snapshot facade remains
unsupported for this live read. Operator capability discovery reports the
mounted grant and its remaining allowance without spending that allowance.

The response retains the canonical sixteen-field Tower envelope and uses result
kind `governance-ledger`, schema `fa.governance-ledger/1`, and disclosure
`recorded_governance_accounting`. A sealed linked result binds the original
registration, exact query address, operation ID and idempotency key. Editing
public envelope fields cannot relabel the native observation during encoding.

## Interpret the original acknowledged cut

The outer Known result and both subsystem sections share one basis: complete
scope, journal revision, control sequence, authority epoch and recorded time.
Those coordinates retain different meanings. Recorded time is historical; the
read does not claim a fresh clock, source frontier or current effect eligibility.
The generic envelope does not turn native journal coordinates into global Tower
coordinates or a measured computation cost.

The credibility section includes the exact declared evaluation domain, stratum
and period; minimum independent-origin counts; precision and recall floors;
false-positive ceiling; original ledger and policy revisions; retained, scoped,
pending and censored case counts; benign and violation origin denominators;
per-member confusion and coverage counters; qualification failures; and lifetime
false stops with the original false-stop budget and incident flag. Repeated
cases from one origin do not become independent evidence. Missing and abstaining
origin counters may overlap. These counts reflect the original labels and do
not independently establish label truth or population calibration.

The Andon section includes the original deployment/evaluator identity and every
original accounting window: its ID, interval, false-stop allowance, per-stop
cost, charged false stops, total cost and any calibration incident's original
opening round and contributing-round count. Costs retain their full `u128`
range. Every integer, including costs and fraction components, is encoded as a
canonical decimal string rather than a potentially lossy JSON number.

An uninstalled subsystem returns Absent over its named original-owner domain
and acknowledged basis. A configured ledger with no cases is Known with zero
counts. Revoked disclosure returns Withheld; lost or faulted custody returns
Unknown; an exhausted allowance returns BudgetExhausted. None permits an effect.
The disclosure omits helper questions, profile bytes, individual labels, raw
evidence and publication payloads.

## Preserve complete finite results

The existing native inventory bounds remain unchanged. Serialization returns
the complete selected summary or refuses within the mount's existing response
ceiling. It never truncates members, windows or failures and never refunds the
query when the complete response does not fit. A separately chosen larger
`PublicationLimits` allowance can accommodate a larger original inventory; it
cannot replenish the original grant or enlarge native ledger limits.

## Verification boundary

Native and Tower regression sources perform actual reference reviews and
independent assessments, including censored-label resolution, same-origin
deduplication, two false-hold charges exceeding `u64` total cost, and a later
permitted publication requiring the original independent keys. Causal cases
cover actor/foreign scope, owner replacement, grant revocation, conflicting
borrows, exhausted queries, poisoned storage, recovery, sealed-provenance edits
and a complete response exceeding its byte allowance.

Scoped syntax/format checks and independent source review are separate from
compilation and execution. Required RCH native tests and canonical qualification
cannot start in the current workspace because `rch` is unavailable. No runtime,
Clippy, empirical detector-quality or production qualification is claimed.
