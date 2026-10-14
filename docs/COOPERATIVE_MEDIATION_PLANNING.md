# Cooperative placement of the original registered enforcers

## Capability and actual consumer

`AuthorityGraph::begin_minimum_cut` returns a sealed `CutPlanningCursor` for the
same complete authority projection, registered enforcer prices and immutable
lifetime limits as `plan_minimum_cut`. This serves the existing mediation
planner and its `FileMediationObserver` consumer (FA-072/FA-073; plan 15.6-15.9,
18; founding FI-A02/FI-A06/FI-A17). It adds no authority, effect type, journal
format, dependency, executor or deployment profile.

The synchronous sparse solver is unchanged and remains an independent execution
oracle. The cursor uses its original projection and residual representation,
including every actor root, protected sink and labeled parallel channel. It
resumes breadth-first searches at the next edge and preserves predecessor paths
through bottleneck selection and forward/reverse augmentation. Polling does not
restart previously completed work. Prices, provenance, graph identity and limits
cannot change within a task.

## Work, termination and verification

Each `advance(max_edge_visits)` spends at most its edge allowance AND the
remaining original lifetime allowance. The total work fields have the same
meaning and value as the synchronous solver. A forward/reverse residual update
reserves two visits together; `minimum_edge_visits` reports this next atomic
requirement. Repeated one-visit polls cannot half-apply that update. Zero performs
no work; callers must eventually provide the advertised minimum to make progress.

Exhaustion returns a sticky error with retained work available from `progress`.
An unwind marks the task Interrupted before it could resume a partially changed
network. No partial cut or incumbent escapes either failure. `Ready` retains a
complete plain proposal until consuming `finish`; it does not certify coverage.
The original independent `verify_cut` must still check the resulting proposal.
Bypass paths and unreachable sinks remain different, nonpermitting outcomes.

The initial sparse projection, residual allocation, queue and phase bookkeeping,
path/result materialization and one pair update are synchronous bounded work.
Counts are logical graph operations, not CPU time, heap measurements, physical
I/O or strict cancellation latency. No speedup, containment or authenticity
claim follows. A declared complete graph can still omit a real-world effect path.

## Source verification status

Eight new public regression functions compare exact outcomes and work against
the unchanged original solver, including all 1,023 nonempty forward-edge subsets
of five nodes under three price vectors and three polling quanta. Independent
exhaustive gate subsets check optimal costs, and the native independent checker
checks candidate coverage. Other controls cover all roots/sinks, parallel/reverse/
self-loop channels, pair-update boundaries, every insufficient lifetime allowance,
frozen prices/provenance, cancellation, invalid inputs and non-vacuity. Two
compile-fail examples prohibit cloning the task and elevating its output to a
VerifiedCut. Existing assertions and independent algorithms are not weakened.

These tests and Rust compilation are UNEXECUTED in the preparation environment:
`rch`, `cargo`, `rustc` and `rustfmt` are absent. The required remote-only test
launch stopped before compilation. Selected-source blob and whitespace checks
are not runtime, format, Clippy, performance or deployment qualification. No
bead is closed and prior execution receipts do not qualify this code.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test mediation_cooperative_planning
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
