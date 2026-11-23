# Checked risk-theater reference projection

This is the bounded score/support kernel for FA-125 (plan §15.10; FI-A18 and
FI-A13). The consumer is a separate governance/evaluator incident path. The
kernel detects threshold crossings without gaining policy or effect authority.
Its source lives in `crates/fa-reference/src/governance_assurance.rs`; its public
regressions are in `crates/fa-reference/tests/risk_theater.rs`.

## Complete boundaries and exact support

Construct `RiskTheaterDetector` with an explicit positive threshold for each of
the six `RiskTheaterFamily` values. Missing or nonpositive configuration refuses
construction. Thresholds are immutable for that detector instance.

Call `apply_frontier` with a `ControlPlaneFrontier`. Sequences start at one and
advance contiguously, including boundaries with no changes. The caller must
declare the boundary closed and every delta must carry that sequence. The
reference caller's `closed` bit is an input assertion, not authenticated proof
that a native journal prefix is complete.

Each family/key identifies one live tuple: its nonnegative metric value and
full support address are exact identity. `metric_delta` retains its historical
field name but contains that tuple's **value**. Multiplicity is a nonzero signed
weight. An insertion adds weight; a retraction removes weight from the exact
existing value and address, never below zero. A changed value or address requires
an exact `-old, +new` pair. For example, retracting a value-12 tuple using value 5
refuses; it cannot delete the support while leaving score 7 behind.

Updates stage the entire batch and commit atomically. Every retained tuple's
multiplicity and contribution must fit i64, and every family's completed score
must fit i64. Intermediate family totals use checked i128 arithmetic. Support
capacity is checked at closure so an insert-before-delete replacement at the
limit remains possible. An error preserves the complete preceding snapshot.

`snapshot(sequence)` and `evaluate_findings(sequence)` require the exact current
closed sequence. A refused new boundary cannot be reported as an empty successful
query by accidentally reading an older snapshot. A snapshot contains all six
thresholds and scores, every weighted support tuple, and threshold-crossing
findings. Finding addresses are deduplicated; the complete weighted relation is
still retained in the snapshot, including zero-score and below-threshold tuples.

## Independent batch comparison

`recompute_risk_theater_batch_oracle` accepts the separately supplied complete
history prefix through a requested sequence. It groups the original deltas by
family/key, reconstructs each key from the beginning at every closed boundary,
and sums the resulting relation from scratch. It does not instantiate the
detector or call its incremental reducer. Checking earlier boundaries prevents
a later deletion from hiding an earlier overflow or capacity violation.

`verify_against_history` compares the whole incremental snapshot with that
reconstruction, not just alarms. It therefore detects a lost retraction even
when the stale tuple contributes zero and neither projection raises a finding.
It cannot detect an omission shared by both supplied inputs or authenticate
their independence.

| Resource | Explicit maximum |
| --- | ---: |
| Live support tuples | 512 |
| Deltas in one complete boundary | 256 |
| Bytes in each key, tenant or address ID | 256 |
| Oracle prefix frontiers | 1,024 |
| Oracle prefix deltas | 4,096 |

Input validation precedes retained cloning. Public address fields are rechecked
against the canonical ASCII component grammar. Exceeding a bound refuses the
operation; findings and supports are never silently truncated.

## Qualification and remaining integration

Fourteen authored regression functions cover all six families across 48 closed
frontiers, exact replacement and multiplicities, address identity, atomic
refusal, product and sum overflow, missing thresholds, gaps/replay/unclosed
input, bounds, deduplicated finding addresses, invalid historical prefixes and
lost retractions below alarm thresholds. These are test definitions; they have
not been executed for this change. The original local implementation passed
scoped Rust parsing/format and whitespace checks before the workspace became
unavailable. The recovered patch received source review; its final formatting
and execution still need the required gate. Remote-only RCH test and canonical
gate attempts could not start because the runner was unavailable.

FA-125 remains in progress. This repair does not implement the six native ledger
query predicates, authenticated journal ingestion/closure, governed threshold
changes, durable incident opening, or enforcement of evaluator/governance-only
access. `NotActorVisible` is a marker, not an authorization boundary. Neither a
reference finding nor successful oracle agreement grants policy authority.
The existing acceptance conditions and execution gate remain unchanged.
