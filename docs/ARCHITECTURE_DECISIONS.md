# Initial architecture decisions

All decisions describe the prospective design. They are open to improvement through explicit counterexamples and revised contracts, not silent implementation drift.

| ADR | Decision | Alternatives considered | Consequence / reopening condition |
|---|---|---|---|
| ADR-001 | Center the effect gate, not an all-purpose safety classifier | Dashboard only; one model with direct authority | Exact scoped enforcement can ship before introspection research; reopen only with a stronger explicit authority model |
| ADR-002 | Separate observation, interpretation, and authorization | Let helper votes directly execute tools | Smaller trusted core and auditable policy; helper sophistication remains replaceable |
| ADR-003 | Separate audit traces from restart checkpoints | One generic cognitive-state object | More types, fewer false replay claims; merge only after a host-specific equivalence proof/contract |
| ADR-004 | Keep one authoritative compact control ledger | Independent truth in database, graph, and object store | Derived systems are replaceable; changing ledger backend requires semantic and recovery equivalence |
| ADR-005 | Use narrow optional Franken integrations | Mandatory full-stack deployment | Minimal tool protection stays economical; add dependencies only for demonstrated benefit |
| ADR-006 | Use explicit unknown outcomes and one-shot permits | Retry on timeout and claim exactly once | May sacrifice availability; prevents unaccounted repeated effects |
| ADR-007 | Preserve independent votes before deliberation | Unstructured group chat with a final consensus | Commitments preserve historical independence of disclosure, not model honesty |
| ADR-008 | Measure joint failures, not multiply miss rates | Assume different architectures are independent | More expensive evaluation; scientifically credible ensemble claims |
| ADR-009 | Prefer risk/decision-sensitive codec evaluation | Optimize average MSE alone | Rare-event tests and external probes become release evidence |
| ADR-010 | Keep causal/latent intervention offline by default | Automatically edit the live actor's internal state | Metacognition can be researched without unvalidated production changes |
| ADR-011 | Classify claims and optimizations explicitly | One “verified” badge; one risk score | More metadata; prevents statistical/benchmark evidence from laundering into exact permission |
| ADR-012 | Publish a test-only independent reference now | Empty Rust workspace or fake implementation stubs | Runnable semantics without a misleading product quickstart |
| ADR-013 | Keep privacy and evidence retention in the control model | Treat storage/deletion as an operational afterthought | Explicit replay degradation and admission backpressure when retention promises cannot be funded |
| ADR-014 | Preserve the inspected custom license convention | Silently label derivative planning work unqualified MIT | Full rider retained and clearly labeled; any license change is the owner's explicit decision |

## How to amend a decision

Name the affected invariant, supply a minimal counterexample or measured problem, propose the replacement semantics, identify compatibility and migration effects, and add an independent test. A performance argument must include the operation/durability cost it changes. A research argument must state the held-out experiment that would distinguish the alternatives.

The design can change radically while its honesty remains stable: no evidence becomes stronger merely because a document was edited.


## Revision 0.2 constitutional decisions

1. Pure Rust and a transitive closed universe supersede any permissive prototype/runtime implication in the first draft.
2. Version judgments through exact read witnesses; do not rebase opaque explanations.
3. Use native ATP as an evidence frontier transport, not a generic byte pipe.
4. Separate authority, causal, incident and failure-cohort graph projections.
5. Maintain findings by registered deltas with exact batch fallback.
6. Commit actual evidence views and treat search as discovery unless exact completeness is established.
7. Share immutable experiment state without sharing rights; reduce traces only under sound independence.
8. Operator-local DSR/Cargo evidence, not hosted CI, controls releases.

The main plan and the dependency/local-release constitutions are normative for these decisions.
