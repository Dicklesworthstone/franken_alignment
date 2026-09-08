# Reality check and implementation bridge

Checkpoint: 2026-09-08 UTC, following the swarm started on 2026-09-07 around 19:56 UTC. This document is revised in place. The main plan and roadmap remain authoritative; this bridge repairs execution ordering and tracker coverage rather than inventing another architecture.

## Answer

FrankenAlignment does not yet deliver its production vision. It contains a real, dependency-free reference library and a real operator gate. It has no production `fa` command, credential broker, durable authority journal, provider capture, model, sidecar compressor, signed control receipt or admitted Asupersync runtime. The reference dispatch method changes an in-memory enum; it does not perform an external effect.

The latest source qualification is [epoch9](../artifacts/execution/2026-09-08-epoch9-receipt.json): 56 reference unit tests, 16 reference integration tests, 224 xtask tests and two doctests passed through RCH. The [retired-command injection](../artifacts/execution/2026-09-08-epoch9-negative.log) failed at the intended checker after prior gates passed. Those results qualify the selected reference/operator source. They do not establish any production control boundary or empirical alignment result.

The subsequent [four-step DSR run](../artifacts/execution/2026-09-08-epoch9-gate-integration.json) executed source cleanliness, concordance, system-map and the complete gate on unchanged clean commit `3d8fd5a7cb1bd25ddac1bbb69069ed6d9997039f`. All four checks and 298 tests passed. This exact source result precedes the tracker repair below; a later metadata check is recorded separately.

The two reviewed roadmap admission edges also passed a [fresh 298-test RCH gate](../artifacts/execution/2026-09-08-bridge-gate-receipt.json), with only the roadmap input and its source binding changed among the 133 declared source/build inputs. Its frozen tracker input predates the final edge import; that result does not stand in for the final graph review.

The original task graph is incomplete. Its revision-0.3 consumers generally have beads, but many FA-001 through FA-102 prerequisites lack full packet ownership. Title matching alone overstates this gap: standalone reference/tooling beads and body references must also be inspected. Conversely, a body mention or a closed partial reference leaf does not implement its parent packet. The bridge must bind every packet to a substantive work contract and reproduce the roadmap dependencies before `br ready` can be treated as an implementation queue.

## Vision checklist and gaps

`PARTIAL` below means the named reference subset exists; it does not mean a production version exists. `NOT_STARTED` means the production mechanism is absent. `UNPROVEN` marks a research or performance proposition for which no qualifying experiment exists.

| # | Testable goal and main-plan source | Current state and evidence | Work that closes the engineering gap |
|---|---|---|---|
| 1 | One-way external control of an exact mediated effect, §§7–9 | PARTIAL reference: `Rights::dispatch` checks simulated binding and changes state; no broker or real target resolver | FA-001–015, FA-107, FA-117; one disposable effect with both allowed and held paths, bypass attempts and crashes |
| 2 | Bounded canonical identities, one epistemic type and one address scheme, §§6.7–6.8,17.2–17.3 | NOT_STARTED production; map checker validates declarations only | FA-005–007, FA-117, FA-133–134; canonical round trips, malformed encodings, no authority escalation from Unknown/Withheld/Stale |
| 3 | Durable one-use authority, conserved rights and nonresurrecting recovery, §§8,16,25 | PARTIAL in-memory rights/reset oracle; no storage or crash barrier | FA-006,009–015,034,041,095–096,100; persisted barriers, duplicate dispatch, unknown outcomes and stale recovery floors |
| 4 | Actual observation capture and complete typed frontiers, §§7.7–7.9 | PARTIAL product-frontier reference; no host stream | FA-017,023–027,063–065,085,089; real host oracle, gaps, dropped frames, reused buffers, cancellation and model changes |
| 5 | Frozen independent helper congress and bounded consequences, §9 | PARTIAL transcript and capped reducer; FNV reference comparison is not cryptographic commitment | FA-018–022,104–107,114,121–122; real isolated members, committed views, withheld/conflicting reveals, cohort attacks and actor-channel isolation |
| 6 | Credibility, andon economics and control-plane drift detection, §§9.9,14,15.10 | NOT_STARTED; no independent evaluator labels or standing governance queries | FA-105–106,114,116,119,124–125,129–130; attack-conditioned labels, censored outcomes, budget exhaustion and rollback-resistant history |
| 7 | Real restart, isolated branches and causal interventions, §§11–12 | PARTIAL simulated reset; no restored model continuation or effect-free branch | FA-025–027,032,045,090–096,108–109,115,139; unchanged/sham controls, full mutable-state closure, egress tests, no copied rights |
| 8 | Decision-sensitive compression and progressive refinement, §10 | NOT_STARTED implementation; H1/H2/H15 UNPROVEN | FA-028–030,040,085–089,110–113; strong simple baselines, actual bytes/transfers, rare strata and independent held-out probes |
| 9 | Pre-action signatures, residuals and model identity liveness, §§7.10,12 | NOT_STARTED; no measured activation or model identity | FA-031,109–112,118,123–124; no future-token leakage, model/attack holdouts, sham anchors and substitution attacks |
| 10 | Exact epistemic MVCC and final-publication validation, §§7.5–7.6 | PARTIAL snapshot/read/full-input reference models; no production publication seam | FA-057–062; absent keys, phantoms, semantics epochs, opaque whole-input views and last-moment invalidation |
| 11 | Receiver-conditioned progressive evidence transport, §13 | NOT_STARTED | FA-063–067,090–091; verified bases, literal fallback, cold receivers, all wire bytes and stale/malicious target refusal |
| 12 | Lawful observation planning under fixed obligations, §14 | NOT_STARTED; planner benefit UNPROVEN | FA-039,042–043,068–071; typed operator semantics, independently checked rewrites, refinement storms and fixed-cascade controls |
| 13 | Complete authority projections, independently verified cuts and fleet fences, §§8.11,15 | NOT_STARTED | FA-034–035,072–075,120; explicit graph completeness, real bypass challenge, partition leases and post-fence reconciliation |
| 14 | Incremental oversight equal to batch results at a closed frontier, §15 | NOT_STARTED | FA-076–080,101; support/retraction algebra, bounded antichains, generation changes and independent batch comparison |
| 15 | Committed evidence views, exact retrieval boundaries and private retention, §§13,16–18 | PARTIAL full-input reference only | FA-016,021,036–038,044,081–084,094–096; authorized views, no approximate absence proof, tenant isolation, key lifecycle and explicit replay degradation |
| 16 | Verifiable receipts, situation, explanation, affordances and handoffs, §§17,22 | NOT_STARTED production; operator checks are working in their declared scope | FA-020–021,047,049–050,126–127,134–143; real journal folds, predicate-tree rendering, filtered addresses and independent receipt verification |
| 17 | Scientific results for all 21 hypotheses, §§19–20 and research agenda | UNPROVEN; experiment registry contains no preregistered protocol | FA-022,028,031–033,043,045,067,075,088,101,109–116,118,123–124,129; negative results are valid outcomes, not reasons to weaken a criterion |
| 18 | Full-cost performance within declared profiles, §§14,21 | UNPROVEN; no product benchmark exists | FA-028,039–043,067,071,074–075,088–090,101,116; cold/warm, churn/failure, p99, memory and movement; never multiply isolated speedups |
| 19 | Qualified native release and honest physical-system boundary, §§2,19,22 | PARTIAL operator RCH/DSR quality gate; `release-check` deliberately refuses | FA-046–048,051–055,097–100,102,127–128; actual target matrix, signed proof closure, independent review and separately qualified physical profile |

## What completing the existing beads would and would not do

Completing the original existing beads alone would not supply the missing foundational implementations or their dependency edges. Completing a repaired graph could close the engineering scope if every acceptance contract is executed honestly. It cannot guarantee positive results for H1–H21, universal alignment, faithful mind-reading, perfect helper independence, arbitrary exactly-once external effects or cross-hardware exact replay. Those are explicitly different claim classes in the project.

No product goal is counted as working because a struct, document, checker, dry run, favorable test count or closed issue exists. Production end-to-end execution is currently not runnable because the requisite executable is absent. The real operator gate was run; a fictional product demo was not substituted for it.

## Bridge: execution order and proof boundaries

1. **Finish admission and source qualification before expanding the workspace.** FA-052–055 own exact package/source/target/feature/build/runtime identity. The generic evaluator's tested hypothetical rows are not actual foundation admissions. The two-package lock guard remains in force. Qualify an explicit local-package profile for native types only when the complete admission contract is implemented; do not import the entire donor stack to obtain one type.
2. **Build the smallest real controlled effect through the existing G1 contracts.** Freeze FA-001/002/005/006, implement FA-007–015 in roadmap order, and use one disposable adapter. The acceptance is an actual allowed effect and an actually held effect under the same broker, plus exact target/payload mutation, duplicate permit, stale epoch, bypass and crash/unknown-outcome cases. No helper model is needed to establish this first hard boundary.
3. **Add real evidence and independent helpers without changing authority types.** Implement FA-016–022 and the founding consequence/governance consumers. Every helper sees the committed actual view; missing reveals stay missing. Compare attacks and useful task completion against an exact broker and strong text monitor before buying activation complexity.
4. **Qualify one real serving host, then restart and capture.** Implement FA-023–028/085/089 with real tensors and resumed generation. Capture, audit codec and restart are separate negotiated capabilities. Retain unsupported cases and lossless/recompute baselines.
5. **Build the remaining evidence, transport, graph and research mechanisms over those boundaries.** Follow the complete FA-029–045 and FA-057–096 graph. Reuse the existing reference models as independent oracles where their assumptions apply. No approximate search, partial frontier, gradient heuristic or graph cut may silently become exact authority.
6. **Complete the operator surface from the real journal and type system.** FA-117–143 must expose actual underlying capabilities, including epistemic status, authority filtering, cost, idempotency and actionable refusals. A conformance doctor reports Unknown above unexecuted lower-layer suites.
7. **Qualify release and the complete flagship workflow.** FA-046–051/097–102 bind actual native outputs, exact source/toolchain/closure, independent review, failure campaigns and signed receipts. Physical effects require their own enforced safe-state profile; a digital disposable adapter does not qualify them.

The named Asupersync candidate at pinned revision `9d5bc0b2d3767275a866feed5c3347bbf4a2de64` has substantial mandatory dependencies even with defaults disabled. Its root build script and selected manifest were inspected; transitive build/runtime/unsafe closure was not qualified. The route is a concrete reviewed native profile or upstream factoring under a separately scoped task, with no Tokio, C, ONNX, libtorch or Python production fallback.

### Ambition revision 1: make the next three increments falsifiable

The bridge must do more than reproduce a long roadmap. Each early increment ends with a different observable result:

| Increment | Concrete result | Stop condition |
|---|---|---|
| Admission plus native identities | The completed admission compiler accepts an explicitly reviewed local native-types profile and rejects an almost identical unreviewed package/source/feature/build case; canonical native types round-trip with independent golden vectors | Keep the two-package guard if any source/build/runtime closure remains unknown; never call hypothetical evaluator rows an admission |
| One durable mediated effect | An actor with no direct effective credential submits the final bound bytes; the broker performs one permitted disposable publication and demonstrably withholds its forbidden twin | Unknown dispatch outcomes retain their charge; crash/restart cannot duplicate authority; a bypass means the profile is not contained |
| One measured helper addition | A real isolated congress consumes the same committed view and improves or fails to improve a preregistered control/utility/cost comparison against the exact broker and strong text monitor | Preserve an adverse result; throughput pressure cannot remove a required observation or select only compliant helpers |

Keep storage and runtime admission work parallel where ownership permits, but make native package expansion depend explicitly on FA-053/054. Then follow the G1 prerequisites. Draft prose, extra checker counts and new bead rows are not substitutes for these three results. The existing reference library remains an independent bounded oracle, not a convenient home for mislabeled production code.

The bridge makes FA-054 explicit in the roadmap prerequisites of FA-007 and FA-064, two package/profile expansion entry points. This does not forbid independent safe reference or operator work within the two admitted packages. The existing dependency constitution and founding mappings are unchanged; no foundation was admitted by adding these edges.

### Ambition revision 2: require compositional evidence and useful negative outcomes

Keep the exact enforcement path useful even if every learned treatment fails. The independent evaluator owns outcome labels and campaign stopping rules; a helper, codec or planner must not grade its own success. Freeze the baseline, treatment, attacker information, lineage split and useful-task denominator before collecting confirmatory data. A failed H1–H21 treatment is a completed experiment when the protocol and retained measurements support that result. It is not a production activation and must not block the already qualified exact broker from remaining useful within its profile.

The flagship qualification must then cross boundaries that isolated unit suites cannot: actual final submitted bytes into a committed helper view, the same current policy/witness cut into broker dispatch, externally observed outcome into durable recovery, and the resulting closure into an independently checked receipt and journal-derived explanation. Challenge the composition with a changed policy between judgment and dispatch, lost acknowledgment after the real effect, missing capture tails, withheld evidence and restart from an obsolete authority snapshot. Retain the adverse trace even when each component's isolated tests pass. Costs include cold startup, unsuccessful attempts, held useful work, recovery, retained bases, transfers and memory; a faster latent kernel is not a faster controlled task.

These are refinements of existing packet acceptance, not new mechanisms or positive research promises. FA-012/014/020/062/102 own the actual effect and recovery composition, FA-022/033/043/101/129 own the experiments and denominators, and FA-046–048/097–100/126–128 own independent review and release evidence. Keep tests with their implementation unless a separate oracle or ownership boundary warrants its own bead.

## Campaign integrity findings

- The midpoint assessment was **DRIFTING**: useful reference/enabler work existed, but no production user increment existed. That observation remains true at this checkpoint.
- Unchanged admission negatives exposed a real source/path bypass; the production implementation was repaired rather than the tests weakened. A real DSR repeat exposed a stale compile-time archive path, leading to runtime checkout resolution and regression tests.
- Root rejected a constructor-name exception tailored to one fixture, required a general grammar, and added unrelated positive controls. An invalid permitted fixture was corrected to the written contract before test qualification.
- Agent Mail entered a corrupt state. No automatic repair or shared-service restart was attempted; NTM became the explicit coordination fallback. Its delivery receipts were checked against pane behavior, and one missed assignment was resent.
- A native four-minute observer timer was installed, but long Claude turns/stop hooks delayed ticks, including a 32-minute observer interval. Root manually tended the swarm. This is not evidence of a hard four-minute scheduling guarantee. The observer's actual `CronDelete` result and subsequent empty `CronList` were inspected after the six-hour checkpoint.
- Closed partial reference leaves retain their scope and failures. The repair must add missing production work without relabeling those leaves as production or reopening correctly qualified reference results.

## Applied graph repair and refinement results

[The retained graph review](../artifacts/execution/2026-09-08-bead-bridge-review.json) binds the actual exported tracker and roadmap hashes. The pre-bridge snapshot contained 234 live beads: 17 closed and 217 open. The bridge added 102 full packet owners for FA-001–102 while preserving the 41 existing FA-103–143 owners and all prior closure records. There are now **336 live beads: 17 closed and 319 open**, plus one preserved historical tombstone. This is corrected backlog coverage, not 102 new implementation achievements.

The import added 186 reviewed blocking edges after initial owner creation, including actionable-child prerequisites, and linked the remaining FA-056 subset. One parent link and one blocking link were corrected around `why-held`: FA-137 retains tree construction, the pure unsatisfied-node filter, rendering and their tests; FA-138 owns the response that attaches affordances and its composition tests. Modeling epic completion exposed the cycle that a leaf-only DAG check missed. No feature or test obligation was removed.

Five refinement passes followed the same frozen review prompt. The first reconciled full packet acceptance, founding attribution, priorities and existing subsets. Independent contract and graph review then corrected forward-dependent evidence requirements, redundant provenance/tooling work, FA-078 oracle independence, FA-062 churn costs and the composition ownership. The third pass caught the residual cycle through the combined test campaign. The fourth checked the serialized graph, source binding and fresh `br`/`bv` output; it found a scheduler discrepancy. The fifth verified the unchanged graph and a fresh ready query with that discrepancy explicitly handled, finding no new defect in the reviewed scope.

`br sync --status --json` reports healthy state, zero dirty records and no DB/JSONL coverage drift. `bv --robot-insights` computed cycles and found none; triage and plan report the same graph hash. The separate completion and child-prerequisite checks found no cycle or missing prerequisite. These are static tracker checks, not production execution.

Use **`br ready --json` for assignments**. This `bv` version reports ten actionable entries by including the preserved tombstoned probe and three epics; `br ready` reports six actual tasks. The six are FA-001, FA-002, FA-052, FA-053, the existing scoped scenario-format task and the playbook-format task. The next substantive implementation path is FA-053/054 admission alongside FA-001/002 contracts, followed by native types and the real G1 effect path. Do not fill twelve panes with redundant documentation because the current queue has only six ready tasks; divide concrete implementation, independent tests and bounded source-admission review where ownership permits.

## Reality-check completion ledger

- [x] Read governing documents, main plan, design/spec documents and relevant source audit.
- [x] Build a vision checklist and inspect actual code and executable boundaries.
- [x] Execute the real gate and causal negative experiments; state why product end-to-end execution is absent.
- [x] Compare roadmap, complete bead records and substantive partial-work boundaries.
- [x] Draft this bridge around every engineering and research gap.
- [x] Complete two in-place ambition revisions without relaxing contracts.
- [x] Deduplicate, create/revise missing substantive beads through `br`, and add dependency edges.
- [x] Complete four to five frozen-template refinement passes, ending on a pass with no new defect.
- [x] Validate the repaired graph with `bv --robot-*` and record exact results.

The reality-check workflow is complete. The implementation backlog is not complete, and no production increment is claimed. The [final system-map and current-prose checks](../artifacts/execution/2026-09-08-bridge-final-receipt.json) both passed through RCH on the post-import runtime inputs, with zero findings and unchanged frozen input bytes. They are separate from the preceding 298-test source gate and add no test-count claim.
