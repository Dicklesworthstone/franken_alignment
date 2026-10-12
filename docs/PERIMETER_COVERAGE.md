# Scoped logical perimeter coverage and least-cost cuts

`fa_reference::perimeter::coverage` implements a bounded, non-vacuous audit of
barriers in the existing `Graph` model. Its `synthesis` module selects minimum-cost
eligible barrier sets. These APIs support the authority-topology and effect-perimeter
work in plan §§4.4, 6 and 15. They do not activate a production adapter, authenticate
an inventory, install barriers or confer effect authority.

## Audit and binding

A caller declares one scope, projection, semantic epoch and topology generation,
complete directed graph, source and sink sets, and eligible barrier sites. The
constructor refuses an incomplete declaration. Sources and sinks cannot be
barrier candidates: removing the actor or protected resource is not credited as
installing a mediator. Completeness and eligibility remain caller assumptions.

`audit` returns each sink separately: already unreachable, mediated with an
original path establishing non-vacuity, or bypassed with a concrete surviving
shortest path. Sorted adjacency and source IDs fix ties. Unreachable sinks are
not included in the active-sink denominator. With no active sinks, certification
refuses instead of turning absent traffic into a positive coverage claim.

`certify` additionally consumes the existing independent `Graph::cut_disconnects`
oracle. `verify` binds the complete normalized graph and declaration, not only a
generation number or short hash, and re-executes the checks. The opaque result
cannot be converted into a permit. It is a logical certificate, not cryptographic
attestation, a portable live-state format, or evidence of complete host mediation.

The original graph limits remain: 128 nodes and 16,384 supplied edges. An audit
performs two breadth-first traversals and retains at most 128 paths of at most
128 nodes. It adds no dependency or executor and makes no latency/RSS claim.

## Minimum-cost synthesis

`CoverageProblem::minimum_cost_cut(costs, budget)` requires exactly one declared
`u64` cost per eligible candidate, including unselected sites. It returns:

- `AlreadyUnreachable`: no active sink exists; no coverage certificate is issued.
- `Impossible`: even all eligible sites leave a concrete source-to-sink bypass.
- `Planned`: a minimum-cost barrier proposal and independently checked coverage
  certificate, with the entire cost inventory and actual solver work retained.

The compiler splits each original vertex into input/output vertices. Only an
eligible vertex's internal edge can have finite cut cost. Original edges and
protected endpoints receive capacity strictly greater than the sum of all
candidate costs. A deterministic augmenting-path solver finds the residual source
side; its selected sites are then checked by the original, independent graph cut
oracle. Cost sums and residuals use `u128`: zero costs and multiple `u64::MAX`
costs are supported without a saturating sentinel. Equal-cost alternatives follow
sorted traversal, not minimum cardinality or a promise to remove redundant free
sites. No running deployment is changed.

`verify_cut_plan` binds all original topology fields and ALL candidate costs,
then recomputes the optimum. Changing an unselected price invalidates an old
cost plan even when its selected barriers would remain optimal. A graph-only
certificate establishes coverage, not current prices or minimum cost.

The explicit `CutBudget` bounds allocated residual cells, inspected residual cells
and completed augmentations. Hard caps are 66,564 `u128` cells, 50,000,000 scans
and 4,096 augmentations. These are residual-solver counts, not total process memory,
FLOPs, wall-clock time or a cancellation deadline. Ordinary topology audits retain
their separate fixed graph bounds. The result records their completed count and
the independent cut check separately. A budget error retains completed work in
`CutFailure` and exports no incumbent, partial certificate or optimality claim.
A zero augmentation allowance can admit a free cut, but the complete residual
search and independent coverage check are still mandatory.

## Example

```rust
use fa_reference::perimeter::coverage::CoverageProblem;
use fa_reference::perimeter::coverage::synthesis::{CutBudget, CutSynthesis};
use std::collections::BTreeMap;

fn propose_barriers(problem: &CoverageProblem, prices: &BTreeMap<usize, u64>) {
    let result = problem.minimum_cost_cut(prices, CutBudget {
        residual_cells: 66_564,
        residual_scans: 1_000_000,
        augmentations: 1_024,
    });
    match result {
        Ok(CutSynthesis::Planned(plan)) => {
            // A graph proposal only; this does NOT install gates or mint rights.
            println!("sites={:?}, declared cost={}", plan.barriers(), plan.total_cost());
        }
        Ok(CutSynthesis::Impossible { audit, .. }) => {
            println!("uncovered paths: {:?}", audit.sinks());
        }
        Ok(CutSynthesis::AlreadyUnreachable { .. }) => {
            println!("no active sinks; no positive coverage claim");
        }
        Err(failure) => {
            eprintln!("no plan: {:?}; work={:?}", failure.error, failure.work);
        }
    }
}
```

## Source qualification — October 12, 2026

Eight public audit tests cover paired cut/bypass cases, cycles, multiple sources,
vacuity, zero-edge paths, endpoint exclusion, incomplete declarations, full binding,
deterministic witnesses and all 4,096 directed graphs on four nodes against the
original cut oracle. Thirteen public synthesis tests add shared-versus-separate
barriers, free and maximum-width costs, immutable cost binding, exact and one-less
budgets, multi-source/multi-sink cycles, self loops and the maximum graph size.
The synthesis exhaustive test compares all 4,096 four-node directed graphs under
nine cost vectors (36,864 cases) against independent subset enumeration. Two
compile-fail examples exclude conversion of certificates or plans into permits.

These Rust tests are authored, NOT executed in this session. The prescribed
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test
perimeter_coverage` stopped before compilation because `rch` is absent (exit 127).
The synthesis test and full-workspace commands were also attempted; both stopped
with exit 127 before compilation for the same missing launcher. No Rust parse, formatting, compilation, Clippy, test or workspace pass is claimed.

A separately executed Python model matched a brute-force subset oracle in 36,864
exhaustive cases (9,216 planned, 9,216 unreachable, 18,432 impossible) and 3,000
seeded cases (152 planned, 789 unreachable, 2,059 impossible). Every planned case
also passed exact and one-less residual scan/augmentation limits. A 128-node chain
returned a cost-seven cut using 67,338 residual scans and one augmentation. These
checks support the algorithm design only; they do not execute or qualify the Rust
implementation. No production, live-perimeter or bead-close claim is made.

Operator verification remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test perimeter_coverage
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test perimeter_cut_synthesis
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
