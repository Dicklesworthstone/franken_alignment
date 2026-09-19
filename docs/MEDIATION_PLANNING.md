# Minimum-cost mediation placement

`AuthorityGraph::plan_minimum_cut` supplies the missing selection step before the
existing independent cut checker. It supports the bounded reference portions of
FA-072/073/074 and plan 15.6-15.9; it does not qualify those full packets.

The operator supplies one explicit positive integer cost for every registered
enforcer. All actor nodes are roots and all sink nodes must be covered. Only
registered enforcers can be selected; processes, credentials, unregistered
Enforcer-shaped nodes and individual channels are not removable substitutes.

The planner constructs a sparse vertex-split residual graph and uses augmenting
paths, including reverse residual arcs, to find a minimum-cost candidate. The
uncuttable capacity is checked `sum(costs) + 1`, not a saturating sentinel. Every
parallel, reverse and self-loop channel is represented separately. Ordering is
deterministic by canonical graph IDs, without a lexicographic tie-breaking claim.
No dense vertex-by-vertex capacity matrix is allocated.

The complete residual layout is bounded before allocation; graph and residual
searches and augmenting arc work share an explicit ceiling. Exhaustion returns no
candidate. Reported counts are logical projection sizes, residual slots and work,
not allocator bytes, measured peak memory, latency or a production SLO.

A result is either a **plain editable CutProposal**, an actual path avoiding all
registered enforcers, or a list of originally unreachable sinks. Disconnection is
not credited as successful mediation. Missing, stale or incompletely represented
inventory refuses. The original independent fixed-point checker must still turn
a candidate into a VerifiedCut; the optimizer cannot mint one or any effect key.
The checker verifies coverage, not the cost objective or actual OS containment.
Inventory completeness, provenance and functioning enforcers remain assumptions.

## Verification status

Eleven Rust tests and a compile-fail example were added. The tests include serial
and joint cuts, multiple roots/sinks, parallel/reverse/self-loop edges, a flow
that requires undoing an earlier choice, real bypass witnesses, incomplete
inventory, integer overflow and exact/one-over work bounds. An independent
subset-enumeration oracle compares every nonempty five-vertex DAG under three
cost vectors; each returned candidate also goes through the original checker.

The required RCH test command was attempted but did not start: `rch: command not
found`, exit 127. Token/delimiter scanning passed; compilation, formatting, Clippy
and the Rust tests remain **UNEXECUTED**. No production activation, speedup claim
or bead closure follows from source alone.
