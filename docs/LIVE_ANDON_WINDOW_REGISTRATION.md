# Register an original Andon window on a live owner

An independently retained supervisor can delegate one exact accounting-window
registration to its original running file-backed publication owner. This connects
the existing durable `FileOversight::register_andon_window` transition to finite
custody that a separate operator can use. It preserves the same native evaluator,
accounting identity, journal and immutable-window laws.

A useful consequence is delayed-label recovery without replacing the owner.
If an original independently assessed benign hold cannot be charged because its
occurrence window is missing, the supervisor may select that exact missing window,
register it through the bounded grant, then retry the original assessment through
its independently retained evaluator. Registration itself supplies no assessment.

Plan §§9.9, 15.10, 16 and 17; FI-A08, FI-A13 and FI-A18. This is a bounded native
reference capability. It neither closes calibration incidents nor makes missing
risk-theater source histories complete.

## Independently provision the exact budget

The supervisor selects the complete `AndonBudget` and retains the noncloneable
custodian separately from the cloneable weak port:

```rust,ignore
let (port, custodian) = supervisor.andon_window_port(
    selected_budget,
    8,       // cumulative original planning queries
    4096,    // complete acknowledged journal plus proposed event
)?;
let observation = port.next(901)?;
```

These example allowances illustrate API shape; they are not recommended
calibration or deployment thresholds. The original `AndonBudget` contains
`window`, `start`, `end`, `false_stops_allowed` and `cost_per_stop`.
Its window identifier is nonzero and its elapsed start precedes its end. Zero
allowance and zero price retain their native meanings. Full-width unsigned values
remain exact.

Provisioning freezes that budget and the original native accounting binding.
It performs no registration, source acquisition, clock observation, label entry
or actor-snapshot consumption. Conflicting existing identifiers and overlaps
remain visible refusals at planning and registration, never replacement policy.

Weak port clones share the custodian's planning allowance and one entered
operation. Dropping or revoking the custodian withdraws access, including cached
result disclosure. Equal numeric scopes do not establish the original owner;
the port is also bound to its original live owner and issuer identity.

## Plan, select, and register at the exact cut

A `Knowledge::Known` plan contains the original scope, accounting identity,
journal revision, control sequence, authority epoch, evaluation revision, congress
and policy generations, and recorded elapsed time. Its exact selected budget,
already-registered flag and explicit block reason accompany that basis.

`FileAndonWindowPlan::request()` returns an action only when its native preview
permits registration. The request carries an operation, complete expected owner
basis and `expected_budget`. That budget is an assertion of the independently
provisioned selection; it cannot choose or replace a grant's window. A request
issued by one grant cannot register a different budget through another grant on
the same owner, even when the owner cut is unchanged.

Select the returned request explicitly and pass it to `port.register(request)`.
The port rechecks the complete predecessor, exact budget, original immutable
window/overlap/capacity laws, original source admission and native numerical gates
before entering mutation. A stale or cheap pre-entry refusal preserves the
operation. Planning consumes its finite query and reserves neither the
predecessor nor future eligibility.

The native law permits a genuinely missing window for an earlier occurrence.
It does not impose a new clock read or silently change an existing period's
allowance or price. Native occurrence ticks, confirmation ticks, control sequence
and journal revision remain distinct coordinates.

## Preserve original accounting, authority and uncertainty

A successful new registration advances the original journal revision exactly
once. Control sequence, authority epoch, evaluation revision, congress generation,
publication policy generation, recorded time and the complete effect/control
snapshot remain unchanged. Registration cannot refund a false stop or an Unknown
effect, change a helper weight, grant an effect key or close an incident.

An exact already-present window returns `registered: false` without another
journal event, numerical replay or actor-snapshot withdrawal. The current
predecessor and selected budget still have to match. This path remains available
under the original no-event law even where a new event would require unavailable
work or admission.

Entered work spends the one operation before native replay or storage can fail.
The retained entered sentinel prevents an unwind or uncertain result from being
treated as a fresh registration opportunity. A returned historical result can be
retried only with that exact operation, predecessor and budget while the same
healthy owner and independent custodian remain available. A lost acknowledgment
does not prove that the journal was unchanged.

## Numerical and source boundaries

A new registration on a native or learned numerical history requires the explicit
`andon_window_port_with_numerical_budget` constructor. Its separate cumulative
event allowance covers the complete original history plus the proposed event,
and is reserved before entry. Failed entered work does not refund it. Original
immutable recipe limits continue to own numerical work; event counts do not
establish a CPU-time limit.

The preview uses the original source, consistency and decoder admission checks.
An interrupted source or paused native decoder therefore cannot spend mutation
or numerical allowance merely by attempting registration. Correcting the original
admission condition permits a fresh plan under the same remaining grant.
Action-predictor custody is not supplied by a numerical replay allowance; new
registrations on owners that require it remain explicitly unsupported.

## Recover the exact extended inventory

Window registration is recorded in the existing original journal format. Retain
the actual acknowledged history independently and extend the independently
selected `FileAndonRecoveryContract` / operator `current_windows` inventory with
the exact returned budget. The original bootstrap windows remain unchanged.

Guarded recovery recomputes accounting from original applied reviews and
independent assessments. An incomplete or altered current-window inventory
refuses; a new empty ledger or the registration receipt cannot replace that
source history. Recovery issues its own fresh evaluator custody and does not
resurrect a live registration grant.

## Verification boundary

Fifteen public regression definitions cover original delayed-label recovery,
complete predecessor and budget fidelity, exact no-ops, finite custody, native
source and numerical admission, storage failure and exact guarded recovery.
All fifteen passed on the combined source at `b0866b44` plus this capability,
using standard LLVM `nightly-2026-09-08` and the original public integration
harness. Scoped Rust formatting also passed.

The full canonical source-snapshot gate and remote RCH qualification remain open.
No production activation, empirical helper calibration, cryptographic evaluator
authentication or complete native risk-theater feed is established by this
reference adapter.

