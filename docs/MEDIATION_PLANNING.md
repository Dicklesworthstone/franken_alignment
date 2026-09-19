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

## Durable supervisor integration

`FileMediationObserver::plan_and_certify` checks its owner, journal revision and
authority epoch before planning the currently available graph. It passes only an
untrusted partition to the original durable `certify` path. The native checker
has its own budget; inspect the returned `checked` result, not merely whether a
candidate exists. The journal retains the original graph/gates/partition/check
inputs in the unchanged format. Replay independently checks coverage; it does
not re-run the optimizer or turn advisory costs into an optimality certificate.

`update_and_plan` commits a topology update **before** planning. The original
update clears old cuts, advances the revocation floor, withdraws old human keys
and cancels only undispatched work. The subsequent solve/check is a separate
ordered local cut. A solver error, missing inventory, bypass or failed certificate
write cannot retain the old permissive topology. `TopologyPlanningReceipt` keeps
the acknowledged topology receipt separate from the fallible certification.
An exact retry does not repeat its fence; an older operation cannot plan against
a newer graph, and a withdrawn graph cannot be reinstated by optimization.

A bad advisory cost vector on an unchanged immutable graph does not invalidate
its earlier independently checked cut. That is not a topology observation. Use
the update path for actual changed or lost topology evidence. Already dispatched
liability and its original cut remain historical; any later nonexecution refund
still requires the original endpoint's accepted receipt. Storage errors expose
no candidate certificate and quarantine the owner until exclusive recovery.

These are supervisor methods, not actor commands or a new authority ledger. No
additional dependencies, journal tags, effect permits or weakened gates are added.

## Verification status

Eleven planner tests and a compile-fail example were added. The tests include
serial and joint cuts, multiple roots/sinks, parallel/reverse/self-loop edges, a
flow that requires undoing an earlier choice, actual bypass witnesses, incomplete
inventory, integer overflow and exact/one-over work bounds. An independent
subset-enumeration oracle compares every nonempty five-vertex DAG under three
cost vectors; each returned candidate also goes through the original checker.

Ten durable integration tests were added using the existing real-file owner and
original two-key review, dispatch, publication and receipt operations. They cover
an allowed publication with the selected cut, fresh work after topology change,
post-dispatch bypass sealing without an early refund, both work ceilings, stale
roles and retries, coverage loss, recovery, and all five deterministic storage
barriers for both certification and topology publication. Injected barriers are
not hardware power-loss tests.

The required RCH commands were attempted but did not start: `rch: command not
found`, exit 127. Token/delimiter scanning passed; compilation, formatting, Clippy
and all 21 new Rust tests remain **UNEXECUTED**. No production activation, speedup
claim or bead closure follows from source alone.
