# Changelog

## 0.3 design revision · 2026-09-06

Put the two founding essays back at the center. Added the normative founding-ideas concordance (`docs/FOUNDING_IDEAS.md`, `registry/founding_concordance.json`) mapping 38 essay ideas, 6 syntheses, 8 deliberately non-literal imports and 8 engineering additions (raised to 9 syntheses and 25 engineering additions by the later batches below) to plan sections, invariants, hypotheses and packets; packet FA-103 keeps it complete. Recorded the chronology showing the essays precede most cited literature.

Plan: two founding commitments (F1 external one-directional control, F2 economical versioned replayable introspection) above the eight bets, each bet naming its founding root; new subsections for typed consequence classes (§9.8), the helper credibility economy with lead-time credit, influence caps and the andon budget (§9.9), escalation reports and quarantined post-round learning (§9.10), the manifold premise as hypothesis (§10.14), the sidecar-to-congress pipeline (§10.15), rewind as containment (§11.10), elicitation campaigns and activation signature libraries (§12.6), and surprise as a masking detector (§12.7); weight-exfiltration and guardrail-removal threat rows; three new rejected alternatives; control-overhead SLO; gates G2–G6 extended.

Registries: invariants FA-INV-031 through FA-INV-034; hypotheses H15 through H18 with research cards; packets FA-103 through FA-116; `SLO-CONTROL-OVERHEAD`; `F-ALIGN-FEEDBACK` source with head-commit pins; founding-idea tags on existing invariants and packets; ADR-015 through ADR-018; threat-model rows for helper hunting, activation masking, honeypot recognition, repetition after rewind and compliant-helper selection.

Execution: the 20 reference tests were executed on one operator host and passed; the complete local gate passed under `nightly-2026-08-31` after the documented `cargo fmt --all` preparation (the only source change), with all logs retained under `artifacts/execution/`. The host's rolling nightly lacked rustfmt/clippy binaries, recorded as a host defect. Registry statuses and `IMPLEMENTATION_STATUS.md` updated accordingly; the revision 0.2 preparation history is preserved verbatim.

Second batch (same day): plan §6.6 typestate purpose contexts, §7.10 model passports and identity liveness, §8.10 graduated autonomy, §8.11 fleet fence with acknowledgment frontier, §9.11 human members and two-key effects, §9.12 multi-party congress, §12.8 self-report consistency, §14.11 live canaries, §15.10 risk-theater detector, §17.9 control receipts, §17.10 assurance profiles, §19.9 concrete formal anchors, §20 preregistration ledger, §22.7 policy replay campaigns, §22.8 attack-capsule interchange; invariants FA-INV-035 through FA-INV-038; hypotheses H19 through H21 with cards; packets FA-117 through FA-131; `registry/experiments.json`; ADR-019 through ADR-022; threat-model rows for model substitution, operator misrepresentation, canary misuse, authority creep and non-propagating halts; fixed the duplicate §7.5 numbering (now §7.5 through §7.9).

Agent-legibility batch (same day): the system stated as one tower of nine layers with five rules (plan §6.7, §6.8); the `Knowledge` epistemic wrapper (§17.3); one address scheme, one verb vocabulary and one journal replacing the two inconsistent command lists (§17.2, §17.8); the situation report as the primary operational surface (§22.1); explain trees, `why-held`, typed affordances, `next`/`propose` with decision cards, rehearsal mode, annotations and handoffs, policy disclosure and actor precheck (§17.8); playbooks and what accretes (§17.11, §17.12); two rejected alternatives; invariants FA-INV-039 and FA-INV-040; packets FA-132 through FA-143; new `docs/SYSTEM_MAP.md`, `docs/AGENT_GUIDE.md`, `registry/system_map.json`, `registry/vocabulary.json`; ADR-023 and ADR-024; README section "Driving it as an agent"; AGENTS.md legibility rule.

Workspace: `.beads/` initialized with the revision 0.3 packets decomposed into self-documenting tasks with tests and logging (no cycles); AGENTS.md records the `br`/`bv` workflow and the concordance rule. README aligned with the plan (founding-ideas section, matching bets, plan §6.2 crate families, corrected claim boundaries).

## 0.2 design revision · 2026-09-06

Deepened the source review across the eight original ecosystem projects plus FrankenNetworkX and Doodlestein Self Releaser. Recorded fixed refs, implementation scopes and mismatches rather than treating README features as integrated guarantees.

Revised the architecture with epistemic MVCC, refinable judgment witnesses, product frontiers, an observation operator algebra, native progressive ATP/receiver-base transfer, authority graph compilation, sparse-kernel qualification, incremental supports/retractions, committed evidence views, sensitivity-aware compression, structural sharing, lawful trace quotient and nonresurrecting recovery.

Made the pure-Rust/nightly/closed-transitive-universe and operator-local release requirements constitutional. Added dependency blockers, a DSR required-quality fragment and an explicit blocked production release gate. Removed the active Python reference and hosted-runner workflow. Added a dependency-free Rust reference and local-gate source; compilation/tests were not run during preparation and no former results are carried over.

Expanded the roadmap to 102 packets and the invariant registry to 30 planned obligations. Corrected the source premise: the two founding repos are complete; there is no missing post.

## 0.1 initial design · 2026-09-06

Introduced the first combined external-helper/introspective-compression architecture, source review, research program, 52 work packets and Python reference. Those historical reference results apply only to the original file set, not this Rust rewrite.
