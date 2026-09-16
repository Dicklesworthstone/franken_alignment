# Complete-tail witness candidate index

Source addition: September 15, 2026. Plan sections 7.6–7.8; FA-061; FA-INV-021. This is a bounded reference-model optimization over the existing witness validator, not an adapter, durable cache, or publication authority.

`witness::refinement::index::WitnessChangeIndex` derives changed-key sets from consecutive, complete `WitnessSnapshot` inputs. It compares versions and values and records insertions and removals. Callers cannot submit a purportedly complete list of changed keys. Snapshot completeness retains the existing `closed_marker` trust boundary: caller-supplied adapter assertions are not cryptographically authenticated by this model.

A judgment is captured from the index's actual immutable current snapshot and bound to a private process-local issuer. An equal revision number cannot substitute another snapshot or another index. Every validation takes an explicit target snapshot; a failed observation cannot silently validate the previous target instead. A different target, a foreign history, an evicted tail, or insufficient planning budget selects the original full exact scan.

The history requires consecutive revision numbers and nondecreasing control cuts in an unchanged semantic/domain/projection profile. Admission failure leaves the old history intact. Retention is bounded to at most 64 deltas; each stores no payloads and at most 512 changed keys under the existing snapshot limits. Constructing a delta temporarily retains at most one additional bounded delta. Derivation costs are reported separately: at most 512 ordered-map lookups and 2 MiB of charged comparison bytes per transition. These are logical limits, not measured latency or allocator overhead.

Candidate selection checks point and half-open-range intersections against the entire retained tail since capture. It has its own intersection-check budget and reports spent checks even when it falls back. Exhaustion discards every partial skip decision. Its fixed-size candidate mask needs no heap allocation; the history does allocate bounded map/set/deque nodes. Current basis and negative closing-frontier checks still run in the original budgeted cursor. A zero-work validation remains pending. Intersection is only a candidate: an ABA change is resolved by exact final-value comparison, not automatically called invalid.

## Usage

```rust,ignore
let mut history = WitnessChangeIndex::new(&initial, &initial_frontiers, 16)?;
let captured = history.capture(&initial_frontiers, requests)?;
let ingestion_cost = history.observe(&next, &next_frontiers)?;
let (index_report, mut validation) = history.begin_refinement(
    &captured, &next, &next_frontiers, 1024,
);
let result = validation.advance(RefinementBudget { steps: 64, value_bytes: 4096 });
// NeedsRefinement is pending, not authority. Retain the cursor and resume it.
// Even StillValid is historical semantic evidence, never a dispatch permit.
```

Import the index from `fa_reference::witness::refinement::index` and `RefinementBudget` from `fa_reference::witness::refinement`. The example illustrates the API; it is not an executed integration test.

The 11 unit tests include eager differentials for 50 mutations under three planning budgets, payload skipping with a positive cost control, missing and evicted history, partial planning exhaustion, ABA restoration, unknown/summary closure refusal, equal-numbered substitution, foreign index identities, changed closing markers, semantic/domain/cut changes, and capacity/overflow/zero-work boundaries.

## Verification boundary

Compilation, rustfmt, Clippy, Rust tests and doctests were not executed: the editing environment lacks Rust and RCH. The existing refinement file was reconstructed with its exact Git blob identity before the small integration patch. No Beads were closed or edited; no qualified test count, shared integration-owned status, dependency admission, or production gate was changed. This does not complete the full FA-061 packet. Frozen-source RCH verification, independent review, mutation testing and cold/churn/allocation measurements remain required.
