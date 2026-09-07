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
| ADR-015 | Treat the two founding essays as the normative spine, with a maintained concordance | Cite them once and let the mechanism catalogue stand alone | Every mechanism names its founding root; a semantic change that leaves the concordance stale is incomplete (FA-103) |
| ADR-016 | Type congress consequences, including rewind as containment | Let helper votes select free-form actions; treat pause as the only intervention | A lattice of registered consequences; `ResetToCheckpoint` never rewinds the ledger; reopen only with a counterexample to FA-INV-034 |
| ADR-017 | Keep the actor out of its own metacognitive loop in production | Let a production actor practice, branch and edit its own latents | Practice runs under `ExperimentCx`; promotion is a fresh proposal; reopen only with an independence proof that the loop cannot be captured |
| ADR-018 | Govern helper selection against risk theater | Optimize helper roster for throughput and low false alarms | Attack-conditioned metrics from the independent evaluator; andon budget exhaustion opens an incident; a helper that keeps catching real violations gains credibility |
| ADR-019 | Make control claims verifiable by outsiders through receipts and assurance profiles | Trust the operator's dashboard and reports | Independent verifier sharing no broker code; receipts disclose only authorized digests; reopen only if receipts prove unusable in practice |
| ADR-020 | Verify the identity of the guarded model continuously | Assume the served model equals the qualified model | Passports plus anchor-based liveness under the audit floor; substitution fences the run; reopen with a fingerprint-preserving attack result (H19) |
| ADR-021 | Bring humans and external parties into the congress under the same protocol | Treat human review as an unmeasured exception; keep all helpers in-house | Humans commit, are scored and are shielded from the actor like any member; providers see only committed views and never labels or authority |
| ADR-022 | Let the control plane monitor its own drift | Rely on governance vigilance alone | Standing risk-theater queries with immutable history visible to the evaluator and governance; thresholds governed, never actor-visible |
| ADR-023 | State the system as one tower with one epistemic type, one address scheme, one journal and one vocabulary | Separate CLI, API, dashboard and log vocabularies; bare booleans in responses | Every agent-facing value is a Knowledge variant (FA-INV-039); the system map is machine-checked so surfaces cannot drift; prose renders from predicate trees |
| ADR-024 | Make every mutation rehearsable in a branch and every error a plan | Dry-run flags with unspecified semantics; errors as prose | Rehearsal returns a labeled outcome and can never permit (FA-INV-040); errors carry retry safety, idempotency key and what would change them |

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

## Revision 0.3 constitutional decisions

1. Control is external and one-directional (F1); introspection is economical, versioned and replayable (F2). These two founding commitments sit above the eight bets.
2. The founding-ideas concordance is normative and machine-checked.
3. Consequences are typed; rewind is a containment action with an out-of-band incident counter.
4. Helper credibility is a ledger with lead-time credit and influence caps, never a false-alarm minimizer.
5. Elicitation runs only in capability-free branches; signatures carry lineage and must transfer beyond their honeypots.
6. The sidecar code is the default activation input to the congress; refinement is bought under budget.
7. Control spend is a measured ratio, not a slogan.
8. Purpose contexts are typestates; a forbidden authority conversion is a compile error.
9. Receipts and profiles make control claims verifiable without trusting the operator.
10. The guarded model's identity is verified, and authority widens only through governed transitions.
11. The system is one tower; every value crossing a boundary carries its epistemic status; every mutation is rehearsable; evidence and notes accrete, authority never does.
