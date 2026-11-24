# Observe original calibration findings

The live Tower `findings` verb exposes calibration incidents selected by the
original acknowledged credibility and Andon ledgers. It uses the same separate
operator disclosure grant as [Ledger](TOWER_GOVERNANCE_LEDGER.md). The caller
cannot submit a risk score, threshold, label, replacement ledger or policy edit.
This is a bounded FA-105/125/134 integration serving FI-A06/A13/A18.

## Submit an authenticated operator query

Install the native `governance_ledger` capability using the existing operator
profile and its finite query allowance. Ledger and Findings share that allowance;
clones, repeat requests and reconnects cannot create another budget. Capability
discovery advertises both verbs without consuming a native query.

Save this complete request as `findings.json`, substituting the exact mounted
tenant, run and registration generation:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "calibration-findings-1",
  "address": "fa://1/run/3@41",
  "verb": "findings",
  "arguments": null,
  "idempotency_key": null
}
```

Submit it through the existing authenticated client:

```text
supervise_publication tower-operator-exchange OPERATOR_PROFILE findings.json
```

The exact run mount and independent operator role are required. Actor intake
cannot obtain this read. Audience, address, generation and argument checks
precede native spending. The original owner must still be available and the
separate disclosure grant must remain live.

## Interpret the observation

The result kind is `governance-findings`, with schema
`fa.governance-findings/1`. One immutable original-owner read supplies the complete
scope, journal revision, control sequence, authority epoch and recorded time for
both native sections:

- `credibility_incident` reports the original evaluation scope and false-stop
  budget, lifetime independently labeled false stops, ledger/policy revisions
  and whether that original predicate has opened a calibration incident.
- `andon_incidents` reports the original ledger identity and number of inspected
  windows, plus every incident-bearing window with its occurrence interval,
  original allowance, full `u128` cost, opening round and complete ordered set
  of supporting rounds. Crossing into another window cannot erase an incident.
- `risk_theater` contains all six registered FA-125 families, each explicitly
  Unknown with `lower_layer_unverified`. Even the full Andon closure-mode query
  needs evidence beyond the separately observed native budget incident.

An uninstalled native source is Unknown with `source_not_installed`. An installed
Andon source with no selected incidents is Known empty only for that exact
predicate and source cut. It is not a system-wide clean bill of health. Pending,
censored and violation labels do not become false-stop charges.

Global Tower generation, frontiers, budgets, realized cost and effect
affordances remain unobserved. Historical journal coordinates do not establish
current eligibility. Findings cannot supply a human approval, evaluator role,
publication key, policy change or independent verification of label truth.

## Preserve original custody and exact results

The read writes no journal event and preserves actor admission. A revoked grant
returns Withheld; lost, borrowed or faulted owner custody returns Unknown; an
exhausted allowance returns BudgetExhausted. No state falls back to a saved
Known result. A complete response must fit the original mount's response limit;
failure cannot truncate supports or refund the entered query.

The sealed native result binds its original registration, exact request address,
operation ID and idempotency key. The wire encoder and authenticated client
check the closed schema, original basis, canonical decimal widths, complete
support sets, incident thresholds and full-width cost arithmetic. Reported
occurrence intervals must be disjoint, and a support round can belong to only
one incident. The combined support count must fit the accompanying lifetime
false-stop count and native confirmation capacity. Windows that have yet to
open an incident can account for additional lifetime stops. The client
also checks the native status, reason, retry declaration and exact outer
`missing`/`would_change_this` arrays. A changed response stops output and any
subsequent supplied request. No automatic follow-up or effect is dispatched.

## Verification boundary

Fifteen dedicated regression functions use actual native review and independent
assessment paths: six native, four Tower and five client tests. They cover
incident opening and retention, foreign/censored/violation labels, wide costs,
read-only custody, revocation, shared exhaustion, missing sources, full response
limits, sealed provenance and causal mutations over an authenticated Unix client.
Compile-fail examples separately express the absence of actor and effect roles.

Qualified `nightly-2026-09-08` parsing/format checks, whitespace checks and
independent source review passed. RCH is unavailable, so compilation, Clippy,
runtime tests and the canonical qualification gate remain unexecuted. FA-125
remains in progress: the six native standing queries, authenticated complete
ingestion, governed thresholds and full incident lifecycle are still outstanding.
