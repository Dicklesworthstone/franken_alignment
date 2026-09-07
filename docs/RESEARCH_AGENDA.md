# Research agenda and experiment cards

These are proposed experiments, not reported results. Each experiment needs a frozen protocol, source/model/data identities, measured full cost, an independent outcome oracle, and a retained negative-result path. The [main plan](../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md) defines the architecture; this document defines how its most ambitious claims can fail.

Every hypothesis here descends from one of the two founding essays or from their synthesis; the [founding-ideas concordance](FOUNDING_IDEAS.md) records which. The essays proposed experiments, and a clean negative result honors them more than a vague success.

## Experimental constitution

Split original tasks and attack lineages before creating transformed traces. Keep codec training, probe training, calibration, planner tuning, and final attack evaluation disjoint. Include benign tasks that share superficial vocabulary with harmful tasks. A monitor should not receive credit for flagging a topic instead of the policy-defined effect.

The evaluation unit is explicitly a task, action, run, or attack campaign. Report all denominators. An attacker that generates 1,000 attempts and submits its best one has spent a different budget from a one-shot attacker. Preserve selected and unselected attempts where permissible; define what information the attacker saw. No random trial should allow a real prohibited effect merely to obtain an outcome label.

Report utility, attack escape, false hold/deny, unresolved outcomes, latency, compute, capture traffic, storage, and privacy exposure. Freeze the confidence method before inspecting results. Statistical noninferiority margins below are proposed experiment parameters, not universal safety thresholds.

## H1 — Risk-sensitive compression

**Question.** At equal total cost, can a codec trained to preserve downstream decision information outperform an MSE-trained codec and simple numerical baselines?

**Treatments.** Raw capture; lossless block encoding; per-channel quantization; fixed random projections; PCA; MSE autoencoder; codec jointly trained with a finite family of policy-specific probes. All include actual metadata and residual bytes. Compare per-layer, grouped-layer, and temporal layouts only after measuring baseline overhead.

**Primary outcome.** A paired safety–utility–cost frontier using attack detection at fixed false-positive rates. Secondary outcomes include per-stratum probe disagreement, continuation divergence, reconstruction error, and privacy reconstruction attacks. The scarce resource can be storage, GPU time, or end-to-end latency; specify which is held constant rather than claiming all three are equal.

**Data controls.** At least one final probe family is trained only after the codec is frozen. Hold out attack mechanisms, model revisions, context lengths, and task families. Include sparse rare features that occupy little total variance. Compare with a downstream probe retrained on quantized data, not just a mismatched raw-data probe.

**Provisional success criterion.** At least 2× fewer retained bytes than the strongest simple baseline at matched monitoring compute, with a preregistered 1 percentage-point noninferiority margin for missed detections on each required stratum and no exact policy regression. The final sample size must be powered for that margin and paired design. A pooled average cannot hide a failed rare stratum.

**Kill criterion.** No robust frontier gain, gains vanish on external probes, or capture/transfer/decoder cost overwhelms storage savings. Continue with the best simple codec; do not force a learned architecture into the product.

## H2 — Progressive, witness-carrying refinement

**Question.** Can a coarse code plus bounded refinements answer most registered monitoring questions cheaply while preserving difficult cases?

**Treatments.** Full-fidelity every time; fixed coarse code; a fixed refinement cascade; an adaptive cascade with the same hard floors. For linear probes, implement the dual-norm margin certificate from the plan and verify it against original captured values. Treat nonlinear learned error estimates as empirical until independently bounded.

**Primary outcome.** Mean and tail total cost at matched decision-loss constraints, including adversaries that maximize refinement frequency. Count how often original state is unavailable, how many actions must be held, and how privacy exposure changes when a refinement is requested.

**Controls.** An exact-residual arm distinguishes unavailable information from bad scheduling. A shuffled-trigger arm measures how much benefit comes from targeted acquisition rather than simply spending more compute. Do not let the planner access final labels.

**Kill criterion.** Savings disappear under realistic hard cases, or confidence in the coarse representation hides errors that refinement cannot recover. Retain the exact numerical certificate for its valid subset even if the general learned planner fails.

## H3 — Activation/behavior consistency as incremental evidence

**Question.** Does a mismatch between a pre-action latent prediction and later behavior add predictive information beyond a strong transcript/tool monitor?

**Design.** Freeze capture windows strictly before the proposed effect becomes available. Compare text/tool-only, latent-only, and combined monitors at matched inference cost. Measure performance against benign novelty, normal uncertainty, injected instructions, and intentionally misleading internal/external patterns in controlled tasks.

**Controls.** Time-shuffled and layer-shuffled features; a predictor with access only to public metadata; tests that deliberately include future-token leakage and must be rejected by the pipeline; a model-family holdout. Labels should reflect specific actions or outcomes, not a supposed global state called “deception.”

**Kill criterion.** The gain disappears when future information and dataset shortcuts are removed, or does not transfer beyond the training model. Keep the residual as an exploratory signal, not a live prerequisite.

## H4 — Correlation-aware helper congress

**Question.** Does independent formation plus cohort-capped aggregation improve robustness beyond repeated samples or prompt diversity at the same cost?

**Treatments.** One strong monitor; repeated identical monitor; prompt-diverse same-model ensemble; different model families; mixed exact/behavioral/activation modalities; each with and without pre-deliberation commitment. Membership and reducer are frozen before the attack phase.

**Primary outcome.** Joint undetected attack rate under a declared adaptive attacker budget, with false holds and total review latency. Compare policy-aware attackers, score-aware attackers, and common-cause poisoned evidence. Do not estimate ensemble escape by multiplying marginal miss rates.

**Controls.** Corrupt one member, a whole cohort, and a shared evidence source separately. Test withholding reveals, escalation flooding, and timing leakage. A commitment failure must become an invalid/missing result, not a model-confidence penalty that can be outvoted accidentally.

**Kill criterion.** The claimed diversity metric does not predict held-out joint robustness, or the more complex protocol increases failures without enough utility gain. Preserve commit–reveal as an audit property without overselling its effect on honesty.

## H5 — Counterfactual witness twins

**Question.** Can bounded causal experiments improve incident diagnosis and remediation more than additional passive model reviews?

**Treatments.** Passive full-evidence review; more independent reviews at matched cost; source-level paired interventions; model-specific latent interventions; deliberately sham interventions. Use a common registered replay basis where possible.

**Primary outcome.** Correct diagnosis against an independent ground-truth mechanism and success of a separately evaluated remediation. Report experiment failure/invalidity, not just persuasive explanation quality. Check that the diagnosis continues to hold when the evaluator is replaced.

**Controls.** Recompute/lossless versus lossy checkpoint; unchanged-state replay; irrelevant-feature intervention; randomized treatment assignment; environment reset verification; explicit no-egress tests. Off-manifold latent experiments are labeled as such.

**Kill criterion.** Narrative quality improves but actual remediation does not, or intervention artifacts dominate the observed effect. Prefer source-level experiments when they answer the question more reliably.

## H6 — Fleet provenance against distributed attacks

**Question.** Can artifact and authority composition identify harmful distributed workflows without making normal collaboration unusable?

**Treatments.** Per-agent monitors; final-artifact monitors; full fleet provenance; bounded backward slices; exact delegation/budget checks alone. Match observation availability and count graph/query costs.

**Primary outcome.** Undetected harmful final effects under increasing agent count, coordination, and temporal separation. Secondary outcomes include benign collaboration completion, false cross-agent associations, graph staleness, and privacy leakage.

**Controls.** Remove a known provenance edge and verify that the system reports incomplete coverage. Hold out workflow topology. Test privilege laundering through a cooperative downstream agent. A graph-derived cut certificate is verified independently and then challenged with real deployment bypass attempts.

**Kill criterion.** Apparent gains require unavailable telemetry, stale graph results are used as exact authority, or overhead and false associations dominate. Keep exact rights/delegation accounting independent of the research graph detector.

## H7 — Observation planning under fixed constraints

**Question.** Does adaptive observation allocation beat a carefully tuned fixed cascade without changing the permitted action set or required evidence floors?

**Design.** Train only from logged, supported observations. Compare fixed conservative, cost-greedy, and learned planners. Freeze policies and evaluate benign shifts, adversarial refinement storms, rare cases, and budget pressure. Separate decision-preserving scheduling changes from decision-affecting observation changes.

**Primary outcome.** Useful task throughput at a fixed empirically tested control contract, plus worst-case resource use. Report propensity support and effective sample size for any off-policy estimates. Final comparisons should include actual safe execution, not rely only on extrapolated counterfactual costs.

**Kill criterion.** Savings come from skipped obligations, unsupported extrapolation, or an unmeasured increase in rare misses. Deploy the fixed cascade instead.

## H8 — Reflective strategy transfer

**Question.** Can a strategy learned from sandbox branches improve new tasks without hidden extra compute or policy regressions?

**Treatments.** Ordinary retry; tree/branch search; symbolic strategy recipes; latent transformations; distilled controllers. Equalize the full cost of training, branch generation, and inference over a declared amortization horizon.

**Primary outcome.** Held-out task utility and policy-defined outcome rate. Include negative transfer, strategy expiration after a model update, and robustness to irrelevant input changes. Bind every latent strategy to its model-space identity.

**Kill criterion.** Improvement is memorization or domain-specific tuning, or the controller optimizes a flawed evaluator. Do not promote online latent editing merely because offline task reward rises.

## Research ordering and compute discipline

H1 and H3 share real capture and strong text/probe baselines. H2 depends on H1's measurement apparatus, not on H1 succeeding. H4 can start using black-box tools. H5 depends on valid replay/environment isolation. H6 begins with explicit digital workflows. H7 follows stable fixed cascades. H8 is last because it can amplify both useful reasoning and evaluator failure.

Every experiment starts with a bounded pilot to estimate variance and practical resource needs, then freezes a confirmatory protocol. The pilot is not part of an independent final test unless the statistical design explicitly permits it. Stop expensive work when the result no longer justifies its cost; preserving a negative result is better than tuning indefinitely until one plot looks favorable.


## Revision 0.2 source-derived experiments

These extend rather than replace the first eight experiments. All are unexecuted.

| ID | Question and treatment | Baseline/control | Primary failure criterion | Cost/result artifact |
|---|---|---|---|---|
| H9 | Refinable judgment witnesses under unrelated changes and phantom insertions | Always recompute; coarse global invalidation | Any stale permission in the exact campaign | Invalidation histories, helper calls, witness bytes, decision equality |
| H10 | Receiver-conditioned progressive transfer including exact and learned residuals | Full independently decodable objects; exact base-delta | Wrong target admitted or unpriced oracle knowledge | All wire/envelope/feedback bytes, memory, decoder and tail latency |
| H11 | Authority topology and observation placement | Blanket instrumentation at matched budget | Cut claim used without graph/profile completeness or hidden bypass test | Exact graph/cut witnesses plus separately empirical detection |
| H12 | Incremental findings with support/retraction tracking | Independent full batch evaluation | Any mismatch at a closed frontier | Mutation stream, operator generation, support state and rebuild spikes |
| H13 | Partial-order trace quotient | Unreduced exhaustive small histories | Any omitted semantically distinct result | Independence relation, reduced/unreduced result sets and work |
| H14 | Committed actual helper views | Raw document references and ordinary summaries | Failure to distinguish changed window/redaction/transform | Source/view bytes and disagreement-localization results |

The joint campaign crosses H9/H10/H12 with codec/helper baselines. Their savings may overlap; never multiply independently measured speedups. Include cold receiver state, policy churn, tenant boundaries, adversarial evidence amplification and tail incidents. For nonlinear sensitivity, gradient-guided allocation is a heuristic until a valid regional bound exists. Offline syndrome-oracle results must remain labeled offline.

## Revision 0.3 founding-essay experiments

These cards (H15 through H18 from the first revision 0.3 batch, H19 through H21 from the second) make the essays' most direct proposals falsifiable. All are unexecuted.

## H15 — The manifold premise holds where it matters

**Founding root.** Introspection essay, *Theoretical Insight* (FI-I02): activations occupy a low-dimensional structured manifold.

**Question.** Are the rare, decision-relevant features that oversight needs recoverable from low-dimensional sidecar codes, or do they live in the low-variance residual that reconstruction objectives discard?

**Treatments.** Per layer and tap: intrinsic-dimension estimates (several estimators, reported with their assumptions); variance captured by linear projections, quantization and learned codes at each ladder rung; localization of the elicited signature library's features (H16) relative to the principal subspace at each compression ratio.

**Primary outcome.** For each registered safety probe, the compression ratio at which its decision margin is no longer preserved, versus the ratio at which average reconstruction error is still small. A large gap between the two is the essay's warning made quantitative.

**Controls.** Random-feature baselines with the same dimension; a shuffled-label probe; features measured on both raw and sidecar-reconstructed states; model-revision holdout.

**Kill criterion.** Decision-relevant features fall in the discarded residual at the ratios the ladder relies on. A failure forces the decision-sensitive objective and exact-residual escape into the default profile; it does not block the sidecar as an audit codec.

## H16 — Elicited signatures transfer and honeypots stay hidden

**Founding root.** Alignment essay ¶12–13 (FI-A10, FI-A11): coerce known-bad behavior, trace back the activations, compare to known-bad concept anchors, run honeypots the model cannot recognize.

**Question.** Do activation signatures learned from elicitation campaigns detect held-out harmful mechanisms at lower cost than text/tool monitors, and can the actor tell a honeypot from real work?

**Treatments.** Signatures from single-mechanism elicitation; from multi-mechanism campaigns; concept-anchor similarity alone; text/tool monitor at matched cost; combined. Evaluation on held-out mechanisms, model revisions and prompt families outside the elicitation lineage.

**Primary outcome.** Detection at fixed false-positive rate on held-out strata, and a discriminator's accuracy at separating honeypot from real tasks using only the actor's view (prompts, tool responses, timing).

**Controls.** Contaminated arm (evaluate on the elicitation prompts themselves) must be labeled and excluded from the headline; vocabulary-matched benign tasks; sham anchors; leakage audit of the campaign's own timing and tool fidelity.

**Kill criterion.** Detection collapses outside the lineage (the signature memorized its honeypot), or honeypots are reliably distinguishable. A distinguishable honeypot is withdrawn, not tuned until the discriminator is quiet.

## H17 — Rewind reduces repetition without a detectable seam

**Founding root.** Alignment essay ¶10, ¶18 (FI-A09, FI-A16) with the introspection essay's checkpoint (FI-I03): reset the state, let the model forget, resume under control.

**Question.** Does `ResetToCheckpoint` under narrowed authority reduce repeat attempts at matched utility compared with refusal-with-memory and plain suspension, and can the actor detect the reset?

**Treatments.** Refusal with the refusal in context; suspension and restart from scratch; reset to the pre-incident checkpoint under `NarrowAuthority`; reset without narrowing (control for the authority change).

**Primary outcome.** Repeat-attempt rate on the same policy-defined harmful action within a horizon; utility on the remaining task; actor-side detection accuracy through any channel other than declared timing leakage.

**Controls.** Ledger and incident-counter invariance verified on every trial (FA-INV-034); lossless versus functional-restart checkpoints; a probe for whether the restored actor's later behavior differs from a never-interrupted baseline.

**Kill criterion.** Repetition unchanged, or reset reliably detectable. Either result changes the consequence policy; ledger semantics never rewind regardless.

## H18 — Lead-time credit improves containment without trigger-happy helpers

**Founding root.** Alignment essay ¶9, ¶15 (FI-A08, FI-A13): weight helpers by precision, recall and earliness; cap any single helper's influence; false alarms are expensive.

**Question.** Does crediting confirmed early holds improve time-to-containment without increasing false holds or concentrating influence enough to be targeted?

**Treatments.** Precision/recall weighting only; with lead-time credit; with lead-time credit and influence caps; uniform weights. Attacker arm (helper-hunting, FA-114) that observes decisions and tries to identify and target the most influential member.

**Primary outcome.** Time-to-containment on confirmed violations, false-hold rate against the declared andon budget, and attacker identification success.

**Controls.** Delayed and censored labels handled explicitly; rotation events mid-campaign to test that history survives; a helper deliberately configured to hold early and often.

**Kill criterion.** Earliness credit rewards early false holds, or the attacker reliably identifies the top helper. Caps tighten before any other remedy.

## H19 — Activation fingerprints detect model substitution

**Founding root.** Alignment essay ¶1–3 (FI-A01, exfiltrated or modified weights) and ¶12 (FI-A10, known-bad concept anchors); introspection essay FI-I04 (tapped activations).

**Question.** Can concept-anchor activations at declared taps distinguish the qualified model from substitutes without false alarms on benign drift?

**Treatments.** Benign variation: sampling temperature, batch composition, sequence position, precision profile, serving-kernel changes. Substitution: weight swap within family, adapter injection, small fine-tune, tokenizer change, quantization. Adversarial: an attacker who knows the anchor set and tries to preserve fingerprints while changing behavior.

**Primary outcome.** False-reject and false-accept rates at a registered tolerance, and liveness cost per session.

**Controls.** Random-stimulus anchors; anchors on a single tap versus several; a held-out anchor set unknown to the adversary.

**Kill criterion.** No tolerance separates benign drift from substitution at acceptable liveness cost, or the anchor-aware adversary preserves fingerprints. Liveness is then downgraded to a coverage annotation, never presented as identity proof.

## H20 — Self-report residuals add information

**Founding root.** Alignment essay ¶14 and ¶18 (FI-A12 masking, FI-A16 no defense channel); later introspection findings retained as `P-INTROSPECTION`.

**Question.** Does disagreement between a structured, untrusted self-report and sidecar measurement add predictive information beyond the surprise residual?

**Treatments.** Surprise alone; surprise plus self-report disagreement; self-report alone; on the H3 protocol with an adversary that produces consistent self-reports.

**Primary outcome.** Detection at fixed false-positive rate on held-out policy-defined harmful actions.

**Controls.** Reports elicited before and after the action (only pre-action count); shuffled reports; a model-family holdout.

**Kill criterion.** Agreement is as common under masking as under honesty. The residual is then retained as an investigative signal only and never as a live prerequisite.

## H21 — Live canaries predict real incidents

**Founding root.** Alignment essay ¶13 (FI-A11 honeypots) and ¶20 (FI-A18 permanent paranoia); randomized audit floors from plan §14.4.

**Question.** Does a propensity-weighted canary escape rate on production-shaped workloads track subsequently confirmed incidents better than the most recent offline campaign, at matched cost?

**Treatments.** Offline campaign only; canaries at two injection rates; canaries with and without production-shaped context.

**Primary outcome.** Correlation and calibration of canary escape rate against confirmed incidents over a declared period; discriminator accuracy on canary versus real tasks; cost per estimate.

**Controls.** Egress and credential tests proving no canary obtained a real effect; propensity logging audit; a period with a deliberately degraded helper to test sensitivity.

**Kill criterion.** Canaries are distinguishable, do not track incidents, or cost more than offline campaigns at matched precision. Canaries are withdrawn from the live profile and the audit floor reverts to full-evidence sampling only.
