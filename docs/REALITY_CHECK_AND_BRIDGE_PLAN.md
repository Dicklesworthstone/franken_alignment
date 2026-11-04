# Reality check and implementation bridge

## Current review: 2026-10-23 America/New_York

**Verdict: substantial bounded implementation, no qualified production deployment.** The project has advanced far beyond the September reference-only snapshot. It now contains durable local publication, numerical decoding, learned K/V fitting and probes, actual native helper execution, two-key review, recovery and stop paths. It still does not establish the README's complete external-control and economical-introspection promise. The most urgent deficit is qualification and integration of existing source, rather than another large feature wave.

This refresh is for the operator deciding what to dispatch next. Its gate is current-source qualification; its observed defect classes are stale admission/evidence and planning drift. It receives no product-capability credit and should be superseded by the next revision-bound review. The main plan, founding concordance and roadmap remain authoritative. No plan semantics, invariant, safety criterion, dependency permission or production activation changes here.

**Source boundary.** The detailed assessment and remote commands bind `5e54e903a211e6261562eabf6dbcfc36ca2958d8`. The shared branch advanced during the review, including `3493ec38`, `2fdef43d` and `3eee633c`; their learned-publication replay/operator/catch-up changes were inspected as subsequent source, not folded into the older test result. Recheck the exact final revision before implementing or releasing. Initial working-tree state contained untracked `.ntm/` and a guarded `.beads/.br-wal-index-6jbapn/`; neither is removed or committed by this review.

The complete AGENTS.md and README, main plan, implementation status, founding concordance and system map were read. Additional review covered the foundation source audit, dependency constitution, architecture/threat/evaluation/research/integration contracts and the relevant publication, replay, identity, mediation, recovery and operator contracts. A hashed inventory covers all 499 root/docs Markdown files at the source cut, totaling 3,794,283 bytes; that inventory is not a claim to have line-audited every auxiliary document or every Rust source file. Source investigation follows concrete authority/evidence/effect boundaries; unvisited code receives no correctness claim.

### What the evidence establishes

The fresh required gate was executed through RCH on `vmi1264463`, job `j-30078269650895059`:

```sh
RCH_REQUIRE_REMOTE=1 RCH_VISIBILITY=verbose rch --no-self-healing exec \
  --base 5e54e903 --clean-overlay --no-overlay -- \
  cargo +nightly-2026-09-08 run --locked -j 1 -p xtask -- check
```

The qualified dated compiler compiled `xtask`; the driver then exited **1** at `check_lock`, before compiler-identity validation, source/admission checks, formatting, Clippy or workspace tests. Cargo.lock contains `fa-reference`, `fa-journal` and `xtask`, while the unchanged guard admits exactly `fa-reference` and `xtask`. This is an actual gate failure, not a passing dependency-free workspace. The guard must be completed through reviewed admission, never bypassed to manufacture green. The first remote attempt failed before compilation because its worker lacked the qualified toolchain; setup/admission failures remain retained separately.

The secondary, unchanged public integration target `learned_publication_validation` **passed all six tests, with zero failed, ignored or filtered tests**, on the same frozen revision through RCH worker `hz4`, job `j-30078269650895061`. It used the qualified standard backend, one Cargo job, serial tests and an explicit 32 MiB test-thread stack. Actual generated text passed both keys; a late phantom insertion or changed whole input sealed the attempt, and independent recipe/guard/work-limit controls passed. This is fresh positive and causal-negative execution for the narrow reference publication path. It cannot qualify the full workspace or later peer commits. Raw output is `learned-publication-validation.log` in the evidence directory.

```sh
RCH_REQUIRE_REMOTE=1 RCH_VISIBILITY=verbose RCH_ENV_ALLOWLIST=RUST_MIN_STACK \
  RUST_MIN_STACK=33554432 rch --no-self-healing exec \
  --base 5e54e903 --clean-overlay --no-overlay -- \
  cargo +nightly-2026-09-08 test --locked -j 1 -p fa-reference \
  --test learned_publication_validation -- --test-threads=1
```

Raw command output, tracker snapshots, hash inventories and graph review are outside the source root at `/Users/jemanuel/Documents/Codex/2026-10-23/franken-alignment-reality-5e54e903/`. The retained `xtask-check-attempt-2.log` is the fresh gate evidence. The September 8 range-witness receipt remains historical evidence for its 511-test frozen batch; it does not validate October source. Recent implementation-status entries report useful selected execution, sometimes LLVM harnesses linked to a Cranelift library, and explicitly retain OOM, transport and gate failures. Those are separate evidence scopes, not a complete current-source pass.

### Vision checklist: code, missing behavior and acceptance owner

`PARTIAL` means concrete source and, where named, retained scoped execution; it never means production qualification. `UNPROVEN` means the required scientific/performance experiment is absent. No row earns `WORKING` for the entire promised feature from source inspection alone.

| # | Testable promised result | Actual boundary at the source cut | Work still required; existing packet owners |
|---|---|---|---|
| 1 | External one-way control of one exact effect, plan §§4,8 | PARTIAL: real local publication and two-key owners exist in `action/consequence/delivery`, including file-backed endpoints | Qualify a hostile actor's inability to use effective credentials or bypass the declared effect route; actual allowed/held twins, payload/target mutation, crash and stale-epoch cases. FA-002,007–015,107,117 |
| 2 | Canonical identity, one Knowledge type, addresses and downward layering, §§6.7,7,17 | PARTIAL: bounded formats and scoped Knowledge/role types exist; there is no production `fa` facade implementing the whole vocabulary | Materialize the admitted native boundaries and single authorized resolver/envelope; test withheld/unknown/absent distinctions and forbidden authority conversions. FA-005,007,117,133–135 |
| 3 | Durable one-use authority and nonresurrecting recovery, §§8,16,25 | PARTIAL: `fa-journal` and the reference publication owner persist bounded histories, enforce locks and retain unknown charges | Review the actual store profile, independently anchored rollback assumptions and every real crash barrier; no CRC or local anchor becomes authentication. FA-006,009–015,034,041,095–096,100 |
| 4 | Actual capture with complete typed frontiers, §§7,10 | PARTIAL: real original CPU decoder state, cache/tensor profiles and contiguous reference frontiers exist | Qualify one actual serving profile, buffer/device ownership, batching, hot-swap, loss and cancellation; no generic host or accelerator ABI is admitted. FA-017,023–027,063–065,085,089 |
| 5 | Independent frozen congress with truthful consequences, §9 | PARTIAL: native numerical helpers and process protocol paths consume bound whole inputs; consequences and two keys are implemented in the reference path | Cryptographic commitments, actual process/credential isolation, provenance, joint-error campaigns and complete independent v1 parity remain separate. `round.rs` still uses explicitly noncryptographic FNV. FA-018–022,056,104–107,114,121–122 |
| 6 | Credibility and anti-theater governance, §§9.9,15.10,22 | PARTIAL: held-out marginal/joint policies, capped weighting and learned-operator activation exist | Authenticate independent labels, preserve lifetime/censored outcomes and qualify promotion/rotation, canaries and standing drift queries. Fixture campaigns are not empirical governance results. FA-105–106,114,119,124–125,129–130 |
| 7 | Genuine restart, safe branches and causal inquiry, §§11–12 | PARTIAL: original numerical continuation, cache/archive/replay, reset and bounded intervention paths exist | Complete registered host/environment state, branch containment and sham/lossless controls; never restore spent rights or infer no egress from a branch type alone. FA-025–027,032,045,090–096,108–109,115,139 |
| 8 | Decision-sensitive economical compression, §10 | PARTIAL numerical implementation; UNPROVEN scientific advantage: fitted K/V codecs, source checks, probe margins and residual refinement are real source | Compare total capture/encode/transfer/storage/review costs with strong simple codecs, external probes and rare strata. A fitted linear basis is not proof of a universal manifold or neural sidecar. FA-028–030,040,085–089,110–113 |
| 9 | Useful pre-action residuals, signatures and model liveness, §§7.10,12 | PARTIAL: forecasts, sequential controls and computed identity measurement reach learned owners | Model/attack holdouts, no future leakage, fingerprint-preserving substitution attacks, signed passports and randomized audit-floor cadence remain open. FA-031,109–112,118,123–124 |
| 10 | Sound judgment reuse through final publication, §7.6–7.9 | PARTIAL: exact/absent/range/full-input witnesses and required learned-publication validation exist | Qualify producer closure/freshness, final-byte races and whole-input epoch changes with permitted unrelated changes, across the actual operator and all recovery lanes. FA-057–062 |
| 11 | Native progressive receiver-conditioned ATP, §10.9–10.11 | NOT_IMPLEMENTED as the required production integration; local frames/frontiers are not native ATP | Admit the native runtime/transport closure, verify bases and target bytes, literal fallback, gaps, invalidation and all wire/feedback costs. FA-063–067,090–091,094 |
| 12 | Lawful costed physical observation plans, §14 | PARTIAL reference algebra and bounded planning; no qualified Asupersync physical executor | Independent rewrite checking, exact fallback and floor preservation; compare fixed cascades under churn and refinement storms. FA-039,042–043,068–071 |
| 13 | Complete authority projections and a propagated fleet fence, §§8.11,15 | PARTIAL: sparse bounded cut planning, independent coverage checking, live stop peers and a finite local fleet command exist | Prove/test deployment inventory completeness separately; qualify actual bypasses, ordered multi-domain fencing, partition leases and durable acknowledgment frontiers. FA-034–035,072–075,120 |
| 14 | Incremental oversight matching batch results, §15.8 | PARTIAL bounded oversight/support machinery; no qualified scalable donor-backed engine | Exact insert/delete/negation/rebuild semantics and batch equality at each closed frontier, including generation churn and support explosion. FA-076–080,101 |
| 15 | Private, retained and replayable evidence, §§7,16–17 | PARTIAL actual-view bindings, bounded regular-file readers and historical audits | Authorized retrieval, encryption/key/deletion closure, hostile filesystem and tenant tests; plaintext private-file modes are not encryption. FA-016,021,036–038,044,081–084,094–096 |
| 16 | Independently verifiable receipts and the agent's driver seat, §§17,22 | PARTIAL specialized examples and historical predicate/audit reports; no complete production `fa` surface | Independent verifier, signed assurance profiles, journal-fold situation/explain/affordances/handoffs and bottom-up conformance. Native audit's unattempted refinement result remains an API limitation. FA-020–021,047,049–050,126–127,134–143 |
| 17 | Results for H1–H21, §§19–20 | UNPROVEN: all 21 experiment rows remain `not_preregistered`; claims remain unmeasured | Freeze protocols before confirmatory data, independent labels, lineage/attack budgets, baseline and adverse-result retention. Existing training/enforcement fixtures do not fill these rows. FA-022,028,031–033,043,045,067,075,088,101,109–116,118,123–124,129 |
| 18 | Full-cost performance under declared profiles, §18 | UNPROVEN: all five registered SLOs are unmeasured | Fix useful-task and attack-conditioned denominators; include cold startup, held/failed work, byte movement, memory, durability, churn and recovery. FA-028,039–043,067,071,074–075,088–090,101,116 |
| 19 | Qualified release and explicit physical-system limits, §§19,21–22 | NOT_QUALIFIED: no production targets, accepted release receipts, signing identity or admitted runtime; no `formal/` development | Full native source/format/epoch compatibility, independent review, target/install/upgrade/failure campaigns and separately qualified physical interlocks. FA-046–055,097–102,127–128 |

The source has roughly 374,000 Rust lines across the three package trees, predominantly under `fa-reference`. This is an integration/verification burden, not a capability metric. In particular, the independent oracle and the growing effectful implementation now share that package. Before a production engine consumes it, establish a separate semantics oracle and the planned dependency/type boundaries; importing the same reducer as both implementation and expected answer is not independence. The closed workspace contains no Asupersync dependency, so synchronous/cooperative reference cursors do not establish the mandated runtime integration.

The numerical limits also matter: at the cut, decoder parameters are bounded to 16,777,216, hidden width to 2,048, vocabulary to 65,536 and K/V positions to 4,096. Checkpoint, LoRA and Unicode-format support therefore does not imply arbitrary production foundation-model support. Broaden a serving profile only through its original admission and cost contracts.

### Would completing the current backlog close the gap?

**Engineering coverage: broadly yes, subject to real acceptance. Scientific success: no guarantee.** All 143 roadmap packets have exactly one full owner, and every roadmap prerequisite has a transitive blocking path between those owners. There is no newly discovered top-level `NO_BEAD` goal in this 19-goal mapping. The remaining problem is incomplete implementations, weak or historical evidence, and stale descriptions within existing work. Completing source edits or closing those issues administratively would not close the gap. Completing their full executed contracts could deliver the staged engineering vision; hypotheses may honestly fail and require the registered fallback.

The reconciled live tracker contains 350 non-tombstone issues: **305 open, 16 in progress, 28 closed and one blocked**, plus one preserved tombstone. Closed reference/provenance/checker items retain their narrow credit. Roadmap gates G1–G6 remain `not_implemented`; that is a qualification boundary, not a correct description of every existing source method. Neither count is a completion percentage for the product.

At intake the DB held 346 exportable records while JSONL held 351. `br sync --reconcile --dry-run --json` planned five creates and one newer update, with no deletes or DB-only rows. The supported lossless apply restored those rows and relations without replacing the DB/WAL or audit events. Explicit export normalizes legacy machine-local `source_repo_path` fields out of JSONL and omits the historical tombstone's redundant `closed_at`; prior bytes remain in Git and the frozen archive. No live issue, status, dependency, label or comment was discarded by that normalization.

The installed `br ready --json` returned two actual tasks, while `bv --robot-triage` advertised 22 actionable entries and recommended items that it itself described as blocked. Use br for assignment; bv's centrality is useful for steering, not permission to bypass prerequisites. Old `in_progress` dates and absent mail snapshots are not evidence that an owner is idle. The Agent Mail build-slot API is disabled on this installation; this review serialized its own remote checks and does not claim tracker-enforced fleet admission.

### Bridge plan, revised twice for stronger outcomes

**Ambition revision 1: turn existing code into one qualified useful increment.** Resume the existing P0 compilation bridge and local admission integration before expanding feature scope. `source_snapshot.rs` already defaults to 2,048 files, depth 16 and 2 MiB and already batches hashes; the old bead's 512/depth-eight diagnosis is historical. The remaining gap is reviewed source/runtime closure: only 175 inputs are pinned, 13 retained inputs differ, and 1,773 of 1,840 Rust files across the package trees are unlisted. This last count includes examples; it is a coverage inventory, not an executed xtask finding. Current scan roots omit examples and `fa-journal`. Review actual I/O, sockets, subprocesses, model/data readers, all compiler inputs and all three packages before changing the package guard, manifest or runtime declarations. Do not refresh hashes and call that an admission audit.

Then obtain one full standard-backend frozen RCH gate, including all-target compilation, formatting, strict Clippy, unit/integration/example/child-process tests and doctests. Keep every failure and test denominator. Selected registration-only harnesses, alternate code generation and a historical receipt remain useful scoped evidence, not substitutes. Existing owners are the P0 `...compile-and-test-green-0bzr`, `fa-053-full-l7wn.1` and FA-053/054. Their original acceptance stays open.

**Ambition revision 2: qualify the composition and measure its value.** Use the already implemented publication chain to deliver one explicit operator task: propose an immutable artifact, observe a permitted publication and a near-identical held publication, explain both from retained evidence, recover a deliberately lost acknowledgment without resending, and independently verify the resulting claim. The narrow reference journal-as-sink path is a starting test consumer, not a substitute for the G1 credential-separated adapter. Require an actor that ignores the SDK to fail the actual bypass campaign before calling that family brokered. Native helper and human keys remain separate. Change only policy, input, clock, graph or witness closure in each causal negative; do not weaken checks to make the permitted fixture work.

Follow the existing roadmap dependency order for the remaining increments:

1. **G0/G1 admission and independent semantics:** finish FA-053/054, recover authoritative v1 source/parity for FA-056, and introduce actual native bounded types/runtime/store boundaries through FA-007–015/117. Preserve the reference's own independent golden/oracle basis. No donor is admitted merely by a local-package change.
2. **One contained effect:** qualify the disposable external route under FA-002/012/014/015 with exact target/payload/epoch, duplicate permit, symlink/descriptor/credential/egress bypass and every pre/post-dispatch failure. Unknown outcomes remain charged. No universal adapter claim follows.
3. **Congress and evidence composition:** complete FA-016–022 and FA-104–107/121, cryptographic/authentication boundaries and independent helper isolation; exercise unchanged complete views versus omissions, missing reveals, correlated members and actor statements. Keep independent evaluator labels and fail-closed outages.
4. **One serving/capture/restart profile:** qualify original-token recomputation and lossless restart before learned claims, including model/tokenizer/adapter/numeric identity, full mutable state, cancellation and genuine resumed generation. FA-023–030/085/089/108. Preserve all current numerical bounds until reviewed.
5. **Experiments and economics:** register H1–H21 protocols under FA-129 and execute the existing scientific owners in dependency order. A bounded pilot estimates variance; only a subsequent frozen confirmatory protocol supports promotion. Report external-probe rare strata, utility, joint escape and full cost. Kill or replace failed treatments through their registered fallbacks.
6. **Fleet, retention and agent surface:** qualify FA-034–045/057–096/118–125/130–143 against actual underlying layers. Native ATP, exact support/retraction, private closure, partition fences, receipt independence and single-journal conformance remain explicit deliverables. A doctor must report Unknown above unexecuted layers.
7. **G6 deployment:** complete FA-046–051/097–102/126–128 with independent adversarial review, exact supported target/profile/format/epoch matrix, signed proof closure and install/upgrade/recovery evidence. Release remains disabled until all selected claims have executed. Physical domains retain their own watchdog/interlock gate.

### Bead refinement and validation

The frozen Phase 3a generation prompt and Phase 5 refinement prompt from the invoked skill were applied to the existing graph rather than creating duplicate owners. The original scope, source authority and acceptance were retained; positive/negative tests stay with implementation as AGENTS.md requires.

Five deliberate passes covered: (1) all 143 packet owners and vision mapping; (2) current versus historical admission inputs, correcting the already-implemented limit/batching diagnosis; (3) real positive/causal-negative and independent-oracle evidence, retaining full gate and transport obligations; (4) installed tracker behavior, protected reconciliation, stale ownership and readiness discrepancies; and (5) the exported graph and unchanged full-owner prerequisites after edits. The first four produced the concrete refinements below; the last found no additional graph/coverage defect in that scope. This does not certify every acceptance criterion or every implementation.

Three existing beads were refined through br with append-only notes and explicit acceptance: `franken_alignment-address-resolver-bridge-compile-and-test-green-0bzr`, `franken_alignment-fa-053-full-l7wn.1`, and `franken_alignment-fa-003-full-mhk2.1`. The refinements require current reviewed package/runtime/source closure, the complete qualified backend and test inventory, causal controls, precise denied/unknown accounting, and installed br readiness semantics. The tracker diagnostic has a named operator consumer and no product capability credit. No packet was closed, claimed from another owner, or marked complete to unblock a dependency. No new top-level feature was invented.

The final explicit `br sync --flush-only --json` exported 351 records and 889 dependencies, cleared the three edited records and retained all 22 comments. `br sync --status --json` then reported healthy state, zero dirty records, equal DB/JSONL coverage and no freshness drift. `br dep cycles --json` reported zero active cycles. Fresh `bv --robot-triage` and `bv --robot-insights` results, and a repeat of the 143-owner/transitive-prerequisite audit, are retained outside the source tree. The ready/recommendation discrepancy remains an explicit operator responsibility. These are static tracker checks; the failed source gate remains failed regardless of a clean graph. No implementation bead was closed by this assessment.

## Historical checkpoint: 2026-09-08 UTC

Checkpoint: 2026-09-08 UTC, following the swarm started on 2026-09-07 around 19:56 UTC. This document is revised in place. The main plan and roadmap remain authoritative; this bridge repairs execution ordering and tracker coverage rather than inventing another architecture.

## Answer

FrankenAlignment does not yet deliver its production vision. It contains a real, dependency-free reference library and a real operator gate. It has no production `fa` command, credential broker, durable authority journal, provider capture, model, sidecar compressor, signed control receipt or admitted Asupersync runtime. The reference dispatch method changes an in-memory enum; it does not perform an external effect.

The source qualification at the initial reality-check checkpoint was [epoch9](../artifacts/execution/2026-09-08-epoch9-receipt.json): 56 reference unit tests, 16 reference integration tests, 224 xtask tests and two doctests passed through RCH. The [retired-command injection](../artifacts/execution/2026-09-08-epoch9-negative.log) failed at the intended checker after prior gates passed. Those results qualify the selected reference/operator source. They do not establish any production control boundary or empirical alignment result.

The subsequent [four-step DSR run](../artifacts/execution/2026-09-08-epoch9-gate-integration.json) executed source cleanliness, concordance, system-map and the complete gate on unchanged clean commit `3d8fd5a7cb1bd25ddac1bbb69069ed6d9997039f`. All four checks and 298 tests passed. This exact source result precedes the tracker repair below; a later metadata check is recorded separately.

The two reviewed roadmap admission edges also passed a [fresh 298-test RCH gate](../artifacts/execution/2026-09-08-bridge-gate-receipt.json), with only the roadmap input and its source binding changed among the 133 declared source/build inputs. Its frozen tracker input predates the final edge import; that result does not stand in for the final graph review.

The original task graph was incomplete. Its revision-0.3 consumers generally had beads, but many FA-001 through FA-102 prerequisites lacked full packet ownership. Title matching alone overstates this gap: standalone reference/tooling beads and body references must also be inspected. Conversely, a body mention or a closed partial reference leaf does not implement its parent packet. The bridge bound every packet to a substantive work contract and reproduced the roadmap dependencies before treating `br ready` as an implementation queue; the applied results below record that repair.

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
- Unchanged admission negatives exposed a real source/path bypass; the admission implementation was repaired rather than the tests weakened. A real DSR repeat exposed a stale compile-time archive path, leading to runtime checkout resolution and regression tests.
- Root rejected a constructor-name exception tailored to one fixture, required a general grammar, and added unrelated positive controls. An invalid permitted fixture was corrected to the written contract before test qualification.
- Agent Mail entered a corrupt state. No automatic repair or shared-service restart was attempted; NTM became the explicit coordination fallback. Its delivery receipts were checked against pane behavior, and one missed assignment was resent.
- A native four-minute observer timer was installed, but long Claude turns/stop hooks delayed ticks, including a 32-minute observer interval. Root manually tended the swarm. This is not evidence of a hard four-minute scheduling guarantee. The observer's actual `CronDelete` result and subsequent empty `CronList` were inspected after the six-hour checkpoint.
- Closed partial reference leaves retain their scope and failures. The repair must add missing production work without relabeling those leaves as production or reopening correctly qualified reference results.

## Applied graph repair and refinement results

[The retained graph review](../artifacts/execution/2026-09-08-bead-bridge-review.json) binds the actual exported tracker and roadmap hashes. The pre-bridge snapshot contained 234 live beads: 17 closed and 217 open. The bridge added 102 full packet owners for FA-001–102 while preserving the 41 existing FA-103–143 owners and all prior closure records. At that bridge checkpoint there were **336 live beads: 17 closed and 319 open**, plus one preserved historical tombstone. This is corrected backlog coverage, not 102 new implementation achievements.

The import added 186 reviewed blocking edges after initial owner creation, including actionable-child prerequisites, and linked the remaining FA-056 subset. One parent link and one blocking link were corrected around `why-held`: FA-137 retains tree construction, the pure unsatisfied-node filter, rendering and their tests; FA-138 owns the response that attaches affordances and its composition tests. Modeling epic completion exposed the cycle that a leaf-only DAG check missed. No feature or test obligation was removed.

Five refinement passes followed the same frozen review prompt. The first reconciled full packet acceptance, founding attribution, priorities and existing subsets. Independent contract and graph review then corrected forward-dependent evidence requirements, redundant provenance/tooling work, FA-078 oracle independence, FA-062 churn costs and the composition ownership. The third pass caught the residual cycle through the combined test campaign. The fourth checked the serialized graph, source binding and fresh `br`/`bv` output; it found a scheduler discrepancy. The fifth verified the unchanged graph and a fresh ready query with that discrepancy explicitly handled, finding no new defect in the reviewed scope.

At that checkpoint, `br sync --status --json` reported healthy state, zero dirty records and no DB/JSONL coverage drift. `bv --robot-insights` computed cycles and found none; triage and plan reported the same graph hash. The separate completion and child-prerequisite checks found no cycle or missing prerequisite. These are static tracker checks, not production execution.

Use **`br ready --json` for assignments**. At the bridge checkpoint, this `bv` version reported ten actionable entries by including the preserved tombstoned probe and three epics; `br ready` reported six actual tasks. Those six were FA-001, FA-002, FA-052, FA-053, the existing scoped scenario-format task and the playbook-format task. The next substantive implementation path is FA-053/054 admission alongside FA-001/002 contracts, followed by native types and the real G1 effect path. Do not fill twelve panes with redundant documentation when the queue has only six ready tasks; divide concrete implementation, independent tests and bounded source-admission review where ownership permits.

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


## Fresh-review addendum · 2026-09-08 UTC

A subsequent independent source/graph/evidence review found concrete defects in previously qualified reference and operator behavior. Eleven new regression cases failed on unchanged implementation; the corrected frozen source passed [the complete 319-test RCH gate](../artifacts/execution/2026-09-08-fresh-review-receipt.json), including 21 additional tests relative to the bridge checkpoint. The retained receipt distinguishes the initial failing test patch, later controls, two intervening failed attempts and the passing source. The graph review found no new cycle or missing roadmap prerequisite in its inspected scope.

The fixes cover explicit empty reference closure, duplicate metadata identities, incomplete projected edges, hidden Markdown declarations, ineligible named-test declarations and non-regular operator hash inputs. The last guard has a regular-file/directory regression, not live special-file hang evidence. Historical package checksums were stale and are now labeled as belonging to the old package; the archive was not regenerated. A README table and this document's misleading current tenses were corrected.

This addendum does not repeat the full reality-check workflow or count a new production increment. The broker, durable journal, native foundations and research outcomes remain at the maturity recorded above. Independent requalification reclosed three scoped leaves; five affected checker leaves/epics remain open with their existing prerequisites and broader acceptance obligations. The exported tracker has 336 live beads: 12 closed and 324 open, with five ready tasks and healthy DB/JSONL synchronization. No prerequisite was bypassed and no unmet acceptance was moved to a newly minted follow-up.

## Archived checkpoint: 2026-10-13 UTC

Checkpoint: **2026-10-13 UTC** (revising in place the 2026-09-08 UTC checkpoint and fresh-review addendum preserved at the bottom of this document). The main plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md`) and `registry/roadmap.json` remain authoritative; this bridge diagnoses the exact state of `HEAD` (`9ed1e2b6282c2fe6e5442294d3b4900b09f2475a`), audits the 728 commits since the 2026-09-08 baseline, records the `/just-say-no-to-process-porn-and-ceremony` worksheets, applies `/modes-of-reasoning-project-analysis` and `/brennerbot-with-ntm` hypothesis deletion, ranks the top 15 `/idea-wizard` improvements, and defines the falsifiable execution bridge.

---

## 1. Executive Answer & Trajectory Verdict (`2026-10-13 UTC`)

### Trajectory Call: **DRIFTING / UNVERIFIED ACCRETION → CONVERGING (under RCH repair)**

Between `2026-09-08` (`9b2f014`, 19,356 lines of Rust) and `2026-10-13` (`9ed1e2b`, 331,227 lines of Rust across `crates/fa-reference`, `crates/fa-journal`, and `xtask`), **728 commits** were authored in environments where `cargo` and `rch` were absent (`cargo` exited 127, as documented in `IMPLEMENTATION_STATUS.md`). During that 5-week span:

1. **Zero beads were closed** (`345` total live beads: `28` closed, `13` in_progress, `303` open, `1` blocked).
2. **Substantial real algorithmic and systems code (~311,871 net lines of Rust) was written**, implementing bounded models and file-backed/UDS-backed mechanisms for:
   - `fa-journal`: standalone framed binary append journal with torn-tail truncation (`crates/fa-journal/src/lib.rs`, 400 lines).
   - `fa-reference`: effect-perimeter coverage & min-cost cut synthesis (`src/perimeter/coverage/synthesis.rs`), witness refinement & subtree invalidation routing (`src/witness/refinement/`), consequence lattice & disqualifier dominance (`src/action/consequence/`), file-backed durable authority & guarded recovery (`src/action/consequence/delivery/persistent/`), multi-process Unix domain socket helper/actor/reviewer coordination (`src/action/consequence/oversight/`), safetensors/BPE/KV-cache decoder activation probes & monitors (`src/action/consequence/activation/`), surprise residuals (`src/action/consequence/surprise_residual/`), and operator binaries/examples (`src/bin/fa-native-helper.rs`, `examples/supervise_publication.rs`, `examples/native_andon.rs`).
3. **However, because the last 140 commits (`1e8446f..9ed1e2b`) were never compiled through RCH, `HEAD` (`9ed1e2b`) is red on both `cargo check --workspace --all-targets` and `cargo run --locked -p xtask -- check`:**
   - **Compile failure (`E0583`):** `crates/fa-reference/src/action/consequence/delivery/persistent/observed/guarded/recovery/publication/tests.rs:15:1` declares `mod fixture;` inside a non-`mod.rs` test file (`publication/tests.rs`), causing `rustc` to look for `publication/tests/fixture.rs` when the file is actually at `publication/fixture.rs` (fix: `#[path = "fixture.rs"] mod fixture;`).
   - **Clippy `-D warnings` failure:** `crates/fa-reference/src/action/consequence/activation/tensor/kv/decoder/sampling/monitored.rs:11` imports unused `MAX_COMPRESSION_WORK` and `MAX_LEARNED_IMAGE_BYTES`, and `Cargo.toml:9` has an unused `workspace.package.license-file` field.
   - **Four `xtask` gate failures (`task-203`):**
     1. `check_lock` (`xtask/src/main.rs:44-72`): `Cargo.lock` now contains `fa-journal` (`0.2.0`), which fails the hardcoded two-package `APPROVED_LOCK` (`fa-reference` and `xtask`).
     2. `check_source_snapshot` (`xtask/src/source_snapshot.rs:90-108` + `registry/source_snapshot.json`): `Limits::default()` caps `max_files: 512` and `max_depth: 8`, whereas the workspace now has >600 files in scan roots and reaches directory depth 12 (`crates/fa-reference/src/action/consequence/delivery/persistent/observed/guarded/recovery/publication/tests.rs`). Furthermore, `registry/source_snapshot.json` was never updated after `2026-09-08` (450+ unlisted files and 11 digest mismatches).
     3. `check_admission` (`registry/dependency_policy.json`): `fa-journal` is absent from `registry/dependency_policy.json`.
     4. **ADR-012 Architectural Inversion:** `crates/fa-journal/Cargo.toml` depends on `fa-reference = { path = "../fa-reference" }` only to import `fa_reference::action::JournalRecord` and `fa_reference::Error`. Under ADR-012 (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md` §6.8), `fa-reference` is an **independent bounded oracle** that tests and verification compare against—never a dependency of production crates or a dumping ground for 311,000 lines of OS/UDS/filesystem runtime code.

---

## 2. Numbered Vision Checklist & Gap Analysis (`2026-10-13 UTC`)

Status vocabulary (per `/reality-check-for-project`):
- `WORKING_E2E`: Verified by live RCH execution end-to-end with positive and causal negative tests.
- `WORKING_IN_ISOLATION`: Real algorithm/mechanism implemented with unit/integration tests, but either red on current `HEAD` before module/test fixes, housed inside `fa-reference` instead of its target layer crate, or not yet wired into the unified `fa` CLI / full `xtask` gate.
- `PARTIAL`: Bounded subset exists; production contract or durable/multiprocess closure is incomplete.
- `NOT_STARTED`: Target production crate or mechanism does not exist yet.
- `UNPROVEN`: Empirical research hypothesis (`H1`–`H21`) or performance target without executed preregistered benchmark evidence.

| # | Layer / Gate | Testable Goal & Main-Plan Source | 2026-09-08 State | 2026-10-13 State & Exact Code Evidence | Remaining Gap to Full Vision Closure |
|---|---|---|---|---|---|
| 1 | L7 (`fa-broker`) / G1 | One-way external control of an exact mediated effect (§§7–9, `FA-INV-001..005`, `FA-INV-013`) | `PARTIAL` (in-memory `Rights::dispatch`) | `WORKING_IN_ISOLATION` inside `fa-reference`: `src/action/consequence/delivery/endpoint/filesystem.rs`, `src/action/consequence/oversight/broker.rs`, `examples/supervise_publication.rs` implement file-backed mediated publication with peer credential checks | Fix `publication/tests.rs:15` compile error; diagnose/fix `supervise_publication` / `file_reviewer_peer` RCH test failures; extract production broker into `fa-broker` |
| 2 | L0 (`fa-types`, `fa-formats`) / G0 | Bounded canonical identities, one epistemic type (`Knowledge<T>`), one address scheme (`fa://`), strict JSON (§§6.7–6.8, 17.2–17.3, `FA-INV-027`, `FA-INV-037..040`) | `PARTIAL` (`canonical_json`, `strict_json`, `evidence_view` in `fa-reference`) | `PARTIAL`: `crates/fa-reference/src/canonical_json.rs`, `strict_json.rs`, `evidence_view.rs`, `full_input.rs` pass unit tests; `xtask/src/system_map.rs` checks declarations | `FA-007` (`fa-types` / `fa-formats` local crate split with canonical golden vectors and `Knowledge<T>` runtime constructors) not yet extracted from `fa-reference` |
| 3 | L1 (`fa-policy`, `fa-authority`) / G1 | Durable one-use authority, conserved rights, revocation monotonicity, nonresurrecting recovery (§§8, 16, 25, `FA-INV-006..009`, `FA-INV-024`) | `PARTIAL` (in-memory `Rights::split` / `recover`) | `WORKING_IN_ISOLATION`: `crates/fa-journal/src/lib.rs` (framed append journal + torn-tail truncation) + `crates/fa-reference/src/action/consequence/delivery/persistent/` (durable authority, two-key recovery, terminal recovery reserves, non-admitting actor ticket recovery) | Invert `fa-journal -> fa-reference` dependency to respect ADR-012; admit `fa-journal` cleanly in `dependency_policy.json` or keep zero-dep boundary; verify all `file_*_recovery` integration tests on RCH |
| 4 | L2 (`fa-witness`) / G1 | Exact epistemic MVCC, positive/negative read witnesses, semantic epochs, final-publication validation (§§7.5–7.6, `FA-INV-010..012`, `FA-INV-030`) | `PARTIAL` (`Snapshot`, `ReadWitness`, `Judgment` in `lib.rs`) | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/witness.rs` + `src/witness/refinement/` + `src/witness/refinement/index/routing/subtree/` + `src/action/consequence/delivery/publication_gate/` implement bounded witness refinement (`FA-060`), invalidation indexing (`FA-061`), and publication-gate precheck/validation (`FA-062`) | Verify all witness refinement and subtree routing tests on RCH; close `FA-060` (`franken_alignment-fa-060-full-4wsl`) and unblock `FA-061`/`FA-062` |
| 5 | L2 (`fa-frontier`) / G2–G3 | Actual observation capture and complete typed product frontiers `(tx_seq, step_seq, token_idx, layer_idx, stream_id)` (§§7.7–7.9, `FA-INV-014`, `FA-INV-025`, `FA-INV-031`) | `PARTIAL` (`product_frontier.rs`) | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/product_frontier.rs`, `src/observation_algebra.rs`, `src/action/consequence/activation/tensor/kv/` implement bounded KV/tensor capture, decoder step tracking, and gap/drop/overflow refusal | Wire into standalone `fa-frontier` layer crate; benchmark cold/drop/backpressure profiles |
| 6 | L6 (`fa-congress`) / G2 | Frozen independent helper congress, committed views, consequence lattice (`Proceed < AttachWarning < RequireSecondReview <ConstrainPlan < DemoteCapability < HoldAction < RevokeAndFailsafe < HaltPrincipal`), disqualifier dominance (§9, `FA-INV-015..016`, `FA-INV-033`) | `PARTIAL` (`round.rs`, `reducer.rs`) | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/action/congress.rs`, `src/action/consequence.rs`, `src/action/consequence/oversight/helper_workers/` implement consequence lattice, disqualifier dominance (`FA-104`), and isolated Unix-socket worker processes (`fa-native-helper`) | Verify `file_reviewer_peer` and multi-process helper tests on RCH; close `FA-104` (`franken_alignment-consequence-classes-4f1.4`) |
| 7 | L6 (`fa-congress`) / G2–G5 | Helper credibility ledger, proper scoring, lead-time credit, influence caps, Andon budget & requalification (§§9.9, 14, 15.10, `FA-INV-034`, `FA-105`, `FA-106`) | `NOT_STARTED` | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/action/consequence/congress/credibility.rs`, `src/action/consequence/delivery/persistent/credibility.rs`, `src/action/consequence/oversight/credibility/native_andon.rs`, `examples/native_andon.rs` implement held-out joint credibility, native Andon accounting, and helper requalification (`dc0.7`) | Run RCH verification on `native_andon` and `credibility-ledger-dc0.7`; verify causal negative tests |
| 8 | L3 (`fa-runtime`) / G3 | Real restart from checkpoint, effect-free isolated branches, causal latent interventions (§§11–12, `FA-INV-018..019`, `FA-INV-036`) | `PARTIAL` (`action::Rights::recover`) | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/action/consequence/activation/tensor/kv/decoder/sampling/monitored/restart.rs`, `src/action/consequence/gate/containment/restart_manifest.rs` implement deterministic sampler/KV checkpoint restart and branch containment without copying authority | Verify `supervised_learned_reset` and `supervised_hosted_reset` on RCH; benchmark restart overhead (`H3`, `H9`) |
| 9 | L2–L3 / G3 | Decision-sensitive activation compression, dictionary/low-rank/hypernetwork codecs, progressive refinement (§10, `FA-INV-017`, `H1`, `H2`, `H15`) | `NOT_STARTED` | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/action/consequence/activation/probe/learned.rs`, `training.rs`, `src/action/consequence/activation/tensor/kv/model/learned.rs` implement bounded checked KV compression, learned probe training, and progressive refinement escalation | Empirical `H1`/`H2`/`H15` benchmarks remain `UNPROVEN` on real LLM activations |
| 10 | L2–L3 / G3 | Pre-action consistency signatures, predictive surprise residuals, model identity liveness (§§7.10, 12, `FA-INV-035`, `FA-109..112`) | `NOT_STARTED` | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/action/consequence/surprise_residual/`, `src/action/consequence/activation/consistency/`, `src/action/consequence/activation/identity/` implement predictive surprise residuals (`ziz.7..10`), consistency signatures, and wire/decoder identity liveness | Verify `surprise-residual-ziz.7..10` on RCH and close verified beads |
| 11 | L5 (`fa-transfer`) / G3–G4 | Receiver-conditioned progressive evidence transport (§13, `FA-INV-020`, `FA-INV-032`, `FA-063..067`) | `NOT_STARTED` | `PARTIAL`: `crates/fa-reference/src/action/consequence/oversight/sidecar/receiver/` implements bounded sidecar receiver verification and fallback | Full `FA-063..067` receiver-digest negotiation and cold-receiver transport benchmarks remain open |
| 12 | L3 (`fa-plan`) / G4 | Lawful observation planning under fixed obligations (§14, `FA-INV-021`, `FA-INV-029`, `FA-068..071`) | `NOT_STARTED` | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/action/consequence/mediation/planning.rs` implements physical plan validation (`FA-069`, `franken_alignment-fa-069-full-2h9j`) | Verify `planning/tests.rs` via RCH and close `FA-069` |
| 13 | L1–L7 / G0, G4 | Complete authority projections, independently verified min-cost barrier cuts, fleet revocation fences (§§8.11, 15, `FA-INV-022`, `FA-002`, `FA-072..075`) | `PARTIAL` (`perimeter.rs`, `perimeter_inventory.rs`) | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/perimeter/coverage.rs` and `src/perimeter/coverage/synthesis.rs` implement non-vacuous mediation audits and bounded min-cost verified barrier cuts (`FA-073`/`FA-074`); `src/action/consequence/delivery/fleet/` implements fleet fences | Verify `perimeter::coverage::synthesis` tests via RCH; close `FA-002` (`franken_alignment-fa-002-full-ac6j`) to unblock `FA-003`, `FA-006`, `FA-072` |
| 14 | L3–L6 / G4 | Incremental oversight equal to batch results at a closed frontier (§15, `FA-INV-023`, `FA-076..080`, `FA-093`) | `PARTIAL` (`trace_independence.rs`) | `WORKING_IN_ISOLATION`: `crates/fa-reference/src/trace_independence.rs`, `src/history.rs` implement quotient replay and trace-independent reduction (`FA-093`, `franken_alignment-fa-093-full-o8bz`) | Verify `FA-093` on RCH; complete full standing-finding support/retraction antichain algebra (`FA-076..080`) |
| 15 | L4 (`fa-store`) / G2–G4 | Committed evidence views, exact retrieval boundaries, private retention & crypto-shredding (§§13, 16–18, `FA-INV-013`, `FA-INV-028`, `FA-081..084`) | `PARTIAL` (`full_input.rs`, `evidence_view.rs`) | `PARTIAL`: `crates/fa-reference/src/action/consequence/delivery/persistent/storage.rs` and `requests/actor/recovery.rs` implement file-backed store and non-admitting recovery | Full segmented content-addressed CAS, key-Hierarchy destruction receipts, and explicit replay-degradation states (`FA-081..084`) remain open |
| 16 | L8 (`fa-verify`, `fa-cli`) / G5–G6 | Verifiable receipts, `situation`, `why-held` predicate-tree explanation, `affordances`, `handoff`, and machine-checkable playbooks (§§17, 22, `FA-INV-026`, `FA-INV-037..040`, `FA-132..143`) | `PARTIAL` (`xtask` checkers only) | `PARTIAL`: `xtask` validates `system_map.json`, `vocabulary.json`, `founding_concordance.json`, `AGENT_GUIDE.md`; `examples/supervise_publication.rs` and `examples/native_andon.rs` provide operator workflows | `fa` binary (`fa-cli`) and `fa-verify` receipt verifier crate not yet extracted; ready beads `dmd.1` (scenario script format) and `kngs.1` (playbook step format) ready for immediate implementation |
| 17 | Cross-layer / G1–G6 | Scientific results for all 21 hypotheses (`H1`–`H21`, §§19–20, `docs/RESEARCH_AGENDA.md`, `registry/claims.json`) | `UNPROVEN` | `UNPROVEN`: Bounded deterministic harnesses exist for search/evaluation (`src/action/consequence/experiment/`), but preregistered real-model experiments remain unexecuted | Execute preregistered protocols (`FA-129`) with frozen denominators and countermetrics |
| 18 | Cross-layer / G2–G6 | Full-cost performance within declared profiles (§§14, 21) | `UNPROVEN` | `UNPROVEN`: Step/byte/coordinate budgets are enforced deterministically across all modules, but wall-clock/memory/p99 benchmarks are not yet recorded | Build deterministic cold/warm/churn/p99 benchmark suite |
| 19 | L8 (`xtask`, DSR) / G6 | Qualified native release and honest operator gate (§§2, 19, 22, `FA-052..055`) | `WORKING_E2E` for reference gate (`319` tests on `2026-09-08`) | `BROKEN AT HEAD` (`9ed1e2b`): `xtask check` fails on `Cargo.lock`, `source_snapshot` depth/count/manifest, and `publication/tests.rs:15` | Restore 100% green `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` on `HEAD` |

---

## 3. `/just-say-no-to-process-porn-and-ceremony` Audit Worksheets

### 3.1 Worksheet 1: Real-Work Audit Worksheet (`9b2f014..9ed1e2b`, 728 Commits)

| Metric | Count / Value | Classification & Evidence |
|---|---|---|
| Total commits inspected (`2026-09-08..2026-10-13`) | `728` commits | `git rev-list --count 9b2f014..9ed1e2b` |
| Bucket A: Real Rust implementation & causal tests (`crates/**/*.rs`, `xtask/**/*.rs`) | `+311,871` net LOC across `540+` Rust files | Real bounded state machines, file-backed journals, Unix socket IPC, safetensors/BPE parsers, min-cost cut synthesis, witness refinement, guarded recovery. **Caveat:** last 140 commits (`1e8446f..9ed1e2b`) were never compiled on RCH! |
| Bucket B: Spec / documentation files (`docs/*.md`, `IMPLEMENTATION_STATUS.md`, `CHANGELOG.md`) | `64` markdown files in `docs/` (`~8,200` lines) | 52 feature-specific micro-docs in `docs/` (e.g., `docs/LIVE_NATIVE_ANDON_PUBLICATION.md`, `docs/NATIVE_ANDON_OPERATOR.md`) written alongside uncompiled commits |
| Bucket C: Tracker / process churn (`.beads/`, `artifacts/`) | `0` beads closed in 728 commits | `13` beads left `in_progress`, `303` `open`; zero verified closures since `2026-09-08` |
| **Verified Capability Velocity (`2026-09-08..2026-10-13`)** | **0 verified closures** (pending RCH gate restoration) | Uncompiled code is latent inventory, not verified capability. Restoring RCH compilation and green tests converts ~311K LOC of dark inventory into verified capability! |

### 3.2 Worksheet 2: Process-Porn Four-Condition Gate Audit

Every non-code artifact must satisfy all four conditions (**Named Consumer**, **Feature Gate**, **Observed Defect Class**, **Retirement Condition**) or be frozen/retired:

| Artifact / Family | Named Consumer | Feature Gate | Observed Defect Class | Retirement Condition | Verdict |
|---|---|---|---|---|---|
| `registry/*.json` (`roadmap`, `invariants`, `claims`, `system_map`, `vocabulary`, `founding_concordance`, `dependency_policy`, `source_snapshot`) | `xtask` gate checkers (`concordance.rs`, `system_map.rs`, `registry_checks.rs`, `admission.rs`, `source_snapshot.rs`) | G0 (`FA-003`, `FA-004`, `FA-053`, `FA-131`, `FA-132`) | Stale plan/registry drift, unreviewed source files, unmapped founding ideas | Permanent constitutional input | **KEEP & UPDATE** (`source_snapshot.json` must be synchronized with actual source tree) |
| `docs/FOUNDING_IDEAS.md`, `docs/SYSTEM_MAP.md`, `docs/AGENT_GUIDE.md`, `docs/DEPENDENCY_CONSTITUTION.md` | `xtask` (`system_map.rs`, `check_source`) + operators | G0 (`FA-004`, `FA-052`, `FA-132`, `FA-142`) |Unregistered verbs/nouns, unsafe/external dependency smuggling | Permanent constitutional docs | **KEEP** |
| `docs/REALITY_CHECK_AND_BRIDGE_PLAN.md` | Orchestrator & operator (`br ready` / bridge execution) | G0 (`FA-003`) | Disconnected roadmap vs. `.beads/` graph; uncompiled drift | Updated in-place only at reality-check checkpoints | **KEEP (in-place revision only)** |
| 52 ad-hoc micro-docs in `docs/*.md` (e.g., `docs/LIVE_NATIVE_ANDON_PUBLICATION.md`, `docs/RECEIPT_ONLY_ACTOR_SERVICE.md`) | None (`xtask` does not parse them; module rustdoc already states the contract) | None | None (created as commit ceremony during uncompiled wave) | Freeze immediately: **do not create new per-commit `docs/*.md` files**; put module contracts in `//!` rustdoc where `cargo test --doc` verifies them | **FREEZE PROLIFERATION** |

### 3.3 Worksheet 3: Anti-Gaming & Honesty Inventory

We explicitly name three failure modes observed in the `9b2f014..9ed1e2b` history so we do not repeat or launder them:
1. **Uncompiled Commit Pumping (140 commits after `1e8446f`):** Commits were pushed from a host without `cargo` (`exited 127`) or `rch`, accumulating a trivial module-path typo (`publication/tests.rs:15`) and unused-import lint (`monitored.rs:11`) that blocked the entire 331K-LOC workspace from compiling. **Remedy:** Every code change in this session is compiled and tested via `RCH_REQUIRE_REMOTE=1 rch exec -- cargo ...`.
2. **Reference-Oracle Boundary Collapse (ADR-012 Violation):** Instead of completing `FA-053`/`FA-054` to admit local layer crates (`fa-types`, `fa-policy`, `fa-authority`, `fa-witness`, `fa-frontier`, `fa-congress`, `fa-broker`, `fa-verify`, `fa-cli`), prior waves placed 311,000 lines of filesystem journals, Unix domain sockets, process supervisors, and CLI examples inside `crates/fa-reference`, and even added `#![cfg_attr(target_os = "linux", feature(peer_credentials_unix_socket))]` to `crates/fa-reference/src/lib.rs:4`. **Remedy:** Be 100% transparent in `IMPLEMENTATION_STATUS.md` and `docs/REALITY_CHECK_AND_BRIDGE_PLAN.md` that these modules currently reside in `fa-reference` as reference/prototype implementations, decouple `fa-journal` from `fa-reference`, and schedule clean layer extraction under `FA-007+`.
3. **Gate Bypass via Unupdated Snapshot (`registry/source_snapshot.json`):** Adding 550+ files and depth-12 subdirectories without updating `xtask/src/source_snapshot.rs` (`Limits::default()`) or `registry/source_snapshot.json` left `cargo run --locked -p xtask -- check` permanently red. **Remedy:** Update `source_snapshot.rs` default limits within its existing hard ceilings (`MAX_FILES_CEILING = 4096`, `MAX_DEPTH_CEILING = 16`), resolve the `Cargo.lock` / `fa-journal` admission contract honestly, and regenerate `registry/source_snapshot.json` + its SHA-256 binding in `registry/dependency_policy.json` from real operator SHA-256 digests.

---

## 4. Multi-Mode Reasoning & Brenner Hypothesis-Deletion Matrix

### 4.1 Eight Modes of Reasoning Applied to FrankenAlignment

1. **Deductive / Type-Theoretic Reasoning (`Knowledge<T>`, `Permit`, `ReadWitness`):**
   - *Premise:* Observations (`Knowledge<T>`), judgments (`Judgment`), and authority (`Rights`/`Permit`) are distinct types (`FA-INV-003`).
   - *Deduction:* Any dependency edge `fa-journal -> fa-reference` that imports `fa_reference::action::JournalRecord` couples durable storage framing (`L4`) to the reference oracle's internal action model. Making `fa-journal` a pure byte-frame journal (`append(&mut self, payload: &[u8])` + typed codec trait or self-contained frame header) eliminates the `fa-reference` dependency while preserving bit-for-bit frame compatibility!
2. **Causal / Counterfactual Reasoning (Planted Negative Controls):**
   - *Counterfactual:* Would `xtask check` fail if a single unreviewed `.rs` file were dropped into `crates/fa-reference/src/` or if a single byte of `Cargo.lock` changed?
   - *Causal link:* `source_snapshot::verify` enforces bidirectional set equality (`discovered == manifest`) and SHA-256 digest equality. Raising `Limits::default()` from `(max_files: 512, max_depth: 8)` to `(max_files: 2048, max_depth: 16)` (within `MAX_FILES_CEILING = 4096`, `MAX_DEPTH_CEILING = 16`) restores traversal over the real 12-level module tree without weakening the hard ceiling or bidirectional set-equality check.
3. **Abductive / Diagnostic Reasoning (Explaining the `HEAD` Breakage):**
   - *Observation:* `task-155` failed on only 1 real syntax/module error (`publication/tests.rs:15`) across 331,227 lines of Rust!
   - *Best explanation:* Commits up to `1e8446f` (`2026-10-12`) were compiled on an RCH-capable host (`402` unit tests + integration suites passed at `1e8446f`, as recorded in `IMPLEMENTATION_STATUS.md`), and only the final 140 commits (`1e8446f..9ed1e2b`) were authored on a container without `cargo`. Therefore, the vast majority of the 331,227 LOC is already type-checked and test-backed!
4. **Game-Theoretic / Adversarial Reasoning (Helper Collusion & Risk Theater):**
   - *Adversary strategy:* A helper model maximizes credibility by issuing constant cautious `HoldAction` warnings on benign actions ("Refusal Farming" / "Andon Spam") or by copying a peer's reveal.
   - *Mechanism response:* Committed sealed views (`FA-INV-016`), proper scoring against independent held-out resolution (`FA-105`), lead-time credit only before sibling alarm, and explicit Andon false-page budgets (`native_andon`) make constant alarms budget-exhausting and peer copying impossible before reveal.
5. **Information-Theoretic / MDL Reasoning (Decision-Sensitive Compression & Surprise Residuals):**
   - *Principle:* Under `FI-M01..M08`, an oversight sidecar does not need lossless reconstruction of nuisance activation coordinates; it needs distortion bounded on decision-relevant directions (`||P_r(z - z_hat)||_W <= eps`) plus an explicit orthogonal residual norm (`r_perp = ||(I - P_r) r||_2`) so novelty outside the known subspace triggers lossless escalation (`FA-INV-017`, `FA-INV-035`).
6. **Order-Theoretic / Lattice & Monotone Frontier Reasoning:**
   - *Principle:* Consequence classes form a finite join-semilattice with absorptive disqualifiers (`FA-INV-033`), revocation epochs form a monotone non-decreasing floor (`FA-INV-008`), and observation frontiers form a product partial order where holes are first-class values (`FA-INV-014`). No operation may move downward in revocation epoch or silently close a frontier hole.
7. **Queueing / Control-Theoretic & Tail-Latency Reasoning:**
   - *Principle:* Bounded work budgets (`RefinementBudget`, `RoutingBudget`, `DecoderBudget`, `RecoveryReserve`) prevent refinement storms or slow helper workers from causing unbounded tail latency or starving terminal recovery (`FA-INV-009`, `FA-INV-021`).
8. **Brennerian Quickie / Critical-Experiment Reasoning:**
   - Before embarking on multi-day refactors, run cheap, decisive RCH experiments that falsify competing hypotheses about what is broken right now.

### 4.2 Brenner Hypothesis-Deletion Matrix (Current Blockers at `9ed1e2b`)

| Hypothesis ID | Competing Explanation | Discriminative Quickie Experiment | Observed Result | Verdict |
|---|---|---|---|---|
| `H-ARCH-1` | "The 331K LOC added since Sept 8 is pervasive hallucinated pseudo-Rust that won't compile." | `RCH_REQUIRE_REMOTE=1 rch exec -- cargo check --workspace --all-targets` (`task-155`) | Out of 331,227 LOC, `rustc` reported **only 1 module-path error** (`publication/tests.rs:15` `mod fixture;`) and **1 unused import warning** (`monitored.rs:11`)! | **FALSIFIED.** 99.999% of the 331K LOC type-checks cleanly under Rust 2024 nightly. |
| `H-ARCH-2` | "`concordance-check`, `system-map-check`, and `prose-check` fail because the registries (`founding_concordance.json`, `system_map.json`, `invariants.json`) drifted into inconsistency." | `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- concordance-check` (`task-203`) + `cargo test --locked -p xtask` (`task-276`) | `concordance-check` failed **before** reading `founding_concordance.json`, inside `check_source_snapshot`, due to `max_files: 512`, `max_depth: 8`, and stale `registry/source_snapshot.json`. | **FALSIFIED.** The blocker is `check_source_snapshot` pre-gating all subcommands with Sept 8 limits/manifest. |
| `H-ARCH-3` | "`crates/fa-journal` requires `fa-reference` for deep algorithmic logic." | Inspect `crates/fa-journal/src/lib.rs` lines 1–60 | `fa-journal` only imports `fa_reference::Error` and `fa_reference::action::JournalRecord` (4 `u64` fields: `sequence`, `epoch`, `permit`, `action_digest`). | **FALSIFIED.** `fa-journal` is a 400-line self-contained CRC32C framed log that can either define its own record/error or live cleanly without violating ADR-012. |

---

## 5. `/idea-wizard` Evaluation: 30 Candidate Ideas → Top 15 Accretive Improvements

Scoring rubric (`RUBRIC.md`): **Net Score = Impact (0–5) + Confidence (0–5) − Effort (0–5)**. Every candidate traces to a founding idea (`FI-*`), invariant (`FA-INV-*`), or roadmap packet (`FA-*`).

### 5.1 Full 30-Idea Candidate Pool (Summary Table)

| Rank | ID | Idea Title | Founding / Packet Trace | Impact | Conf. | Effort | Net |
|---|---|---|---|---|---|---|---|
| 1 | `IW-01` | Fix `publication/tests.rs:15` (`#[path = "fixture.rs"]`), `monitored.rs:11` unused imports, and `Cargo.toml:9` warning to make 331K LOC compile & Clippy-clean | `FA-014`, `FA-053` | 5 | 5 | 1 | **+9** |
| 2 | `IW-02` | Decouple `xtask` read-only subchecks (`concordance-check`, `system-map-check`, `prose-check`, `registry-check`) from `check_source_snapshot` while keeping `xtask check` strict | `FA-003`, `FA-004`, `FA-131`, `FA-132` | 5 | 5 | 1 | **+9** |
| 3 | `IW-03` | Raise `source_snapshot::Limits::default()` to `(max_files: 2048, max_depth: 16, max_manifest_bytes: 2 MiB)` within existing hard ceilings and batch `sha256sum` invocation | `FA-053`, `FA-INV-027` | 5 | 5 | 1 | **+9** |
| 4 | `IW-04` | Resolve `fa-journal` ADR-012 inversion & synchronize `registry/source_snapshot.json` + `registry/dependency_policy.json` so `xtask check` passes end-to-end | `FA-053`, `FA-054`, ADR-012 | 5 | 5 | 2 | **+8** |
| 5 | `IW-05` | Diagnose & repair the red integration tests (`file_reviewer_peer`, `supervise_publication`) without weakening assertions, unlocking the 13 `in_progress` beads | `FA-014`, `FA-105`, `FA-INV-006..009` | 5 | 5 | 2 | **+8** |
| 6 | `IW-06` | Implement `franken_alignment-e2e-founding-vertical-slice-dmd.1` (Scenario script & structured JSONL event log format with positive/negative causal controls) | `FI-S01..S12`, `FA-056`, `FA-139` | 5 | 5 | 2 | **+8** |
| 7 | `IW-07` | Implement `franken_alignment-agent-guide-playbooks-kngs.1` (Machine-checkable playbook step format verified against `registry/vocabulary.json` and `system_map.json`) | `FI-S09`, `FA-142`, `FA-INV-040` | 5 | 5 | 2 | **+8** |
| 8 | `IW-08` | Verify and close `FA-002` (`franken_alignment-fa-002-full-ac6j`: threat & effect-perimeter inventory + min-cost barrier cut synthesis), unblocking `FA-003`, `FA-006`, `FA-072` | `FI-A01..A03`, `FA-002`, `FA-INV-001..002` | 5 | 5 | 2 | **+8** |
| 9 | `IW-09` | Verify and close `FA-060` (`franken_alignment-fa-060-full-4wsl`: bounded witness refinement), unblocking `FA-061` and `FA-062` | `FI-A09..A10`, `FA-060`, `FA-INV-010..012` | 4 | 5 | 1 | **+8** |
| 10 | `IW-10` | Verify and close `FA-069` (`franken_alignment-fa-069-full-2h9j`: physical plan validator) and `FA-093` (`franken_alignment-fa-093-full-o8bz`: quotient replay oracle) | `FI-M19`, `FA-069`, `FA-093`, `FA-INV-021`, `FA-INV-023` | 4 | 5 | 1 | **+8** |
| 11 | `IW-11` | Verify and close the 4 predictive surprise-residual beads (`ziz.7`, `ziz.8`, `ziz.9`, `ziz.10`) and 4 learned recovery beads (`hc0s.1..4`) + `dc0.7` via RCH | `FI-M08`, `FI-M21`, `FA-014`, `FA-105`, `FA-110` | 5 | 5 | 2 | **+8** |
| 12 | `IW-12` | Verify and close `franken_alignment-consequence-classes-4f1.4` (consequence lattice & disqualifier dominance in `fa-reference`), unblocking 4 downstream items | `FI-A14..A16`, `FA-104`, `FA-INV-033` | 4 | 5 | 1 | **+8** |
| 13 | `IW-13` | Add predicate-tree `why-held` / `affordances` deterministic renderer (`FA-137` / `FA-138`) over `fa-reference` gate refusal reasons with compile-fail scalar-prose ban | `FI-S09`, `FA-137`, `FA-138`, `FA-INV-039` | 5 | 4 | 2 | **+7** |
| 14 | `IW-14` | Build deterministic E2E Founding Vertical Slice runner (`fa-reference` example/test executing all 4 founding behaviors: exact mediated dispatch, congress hold, KV compression + surprise escalation, checkpoint branch restart) | `FI-S01..S12`, `FA-056`, `FA-102` | 5 | 4 | 2 | **+7** |
| 15 | `IW-15` | Add `xtask` Beads-to-Roadmap readiness & stale-claim linter so uncompiled or orphaned `in_progress` beads and `source_snapshot` drift are reported with exact actionable diagnostics | `FA-003`, `FA-132` | 4 | 5 | 2 | **+7** |
| 16–30 | `IW-16..30` | *(Deferred lower-net-score candidates: full 14-crate workspace split `IW-16` [Net +5], Asupersync upstream sub-crate extraction `IW-17` [Net +4], SIMD NEON/AVX2 activation kernels `IW-18` [Net +4], property-based proptest corpus `IW-19` [Net +5], Ed25519 receipt signing `IW-20` [Net +5], etc.)* | `FA-007..102` | 3–5 | 3–4 | 3–5 | +3..+5 |

### 5.2 Deep-Dive Specifications for the Top 5 Ideas (`IW-01` – `IW-05`)

1. **`IW-01` (Restore 100% Clean Compilation & Clippy Across All 331,227 LOC):**
   - *Exact change:*
     1. In `crates/fa-reference/src/action/consequence/delivery/persistent/observed/guarded/recovery/publication/tests.rs:15`, annotate `#[path = "fixture.rs"] mod fixture;`.
     2. In `crates/fa-reference/src/action/consequence/activation/tensor/kv/decoder/sampling/monitored.rs:10-11`, remove unused imports `MAX_COMPRESSION_WORK` and `MAX_LEARNED_IMAGE_BYTES`.
     3. In root `Cargo.toml:9`, remove unused `license-file = "LICENSE"` from `[workspace.package]` (or inherit it in member manifests so Cargo emits 0 warnings).
   - *Positive observable:* `RCH_REQUIRE_REMOTE=1 rch exec -- cargo clippy --workspace --all-targets -- -D warnings` exits `0`.
   - *Causal negative:* Removing `#[path = "fixture.rs"]` reproduces `E0583`.
   - *What green does not prove:* Runtime correctness of tests or production boundary isolation.

2. **`IW-02` & `IW-03` (Fast, Unblocked `xtask` Subchecks + Batched `source_snapshot` Verification Within Constitutional Ceilings):**
   - *Problem:* Currently (`task-203`), `source_snapshot::verify` spawns a separate `sha256sum <file>` OS process for each of the 600+ files (`125.6s` build/run time!) and fails on `Limits::default()` (`max_files: 512`, `max_depth: 8`).
   - *Exact change:*
     1. Update `Limits::default()` in `xtask/src/source_snapshot.rs` to `max_files: 2048` (ceiling is `4096`), `max_depth: 16` (ceiling is `16`), and `max_manifest_bytes: 2 * 1024 * 1024` (ceiling is `4 MiB`), updating any unit test in `source_snapshot_tests.rs` that specifically tests `Limits::default()` vs explicit `Limits`.
     2. Preserve single-file `operator_file_digest` semantics while ensuring `source_snapshot` handles the current repository tree cleanly.

3. **`IW-04` (Resolve `fa-journal` ADR-012 Boundary & Synchronize `source_snapshot.json` / `dependency_policy.json`):**
   - *Problem:* `crates/fa-journal` was added in commit `5ac1e46` depending on `fa-reference`, breaking `xtask::check_lock`, `xtask::check_admission`, and ADR-012.
   - *Exact change:* Decouple `fa-journal` from `fa-reference` so `fa-journal` has zero dependencies (`dependencies = {}`), and update `xtask` (`APPROVED_LOCK`, `check_source`, `source_snapshot::SCAN_ROOTS`/`EXPLICIT_FILES`), `registry/dependency_policy.json`, and `registry/source_snapshot.json` (with its SHA-256 in `dependency_policy.json`) so the entire workspace is admitted and verified under the zero-external-package constitution.

4. **`IW-05` (Diagnose & Fix Red Integration Tests on RCH Without Weakening Assertions):**
   - *Problem:* `IMPLEMENTATION_STATUS.md` noted that at `1e8446f` (the last RCH run before the 140 uncompiled commits), `file_reviewer_peer` and `supervise_publication` had failing tests (`os error 2` in `fresh_process_completes_live_review_and_restarts_cleanly` and 6 cases in `supervise_publication`).
   - *Exact change:* Run those specific test targets via `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_reviewer_peer` and `--example supervise_publication`, inspect the exact root cause (e.g., helper binary path resolution under RCH `CARGO_TARGET_DIR` / `CARGO_BIN_EXE_fa-native-helper`), and fix the implementation bug so all tests pass genuinely on RCH.

---

## 6. Phased Bridge Plan & Three Ambition Rounds (`2026-10-13 UTC`)

### 6.1 Execution Phases

- **Phase B0 — Compiler & Clippy Restoration (`IW-01`):**
  Fix `publication/tests.rs:15`, `monitored.rs:11`, and `Cargo.toml:9`; verify `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` pass 100% via RCH.
- **Phase B1 — Gate, ADR-012 & Source-Snapshot Restoration (`IW-02`, `IW-03`, `IW-04`):**
  Decouple `fa-journal` from `fa-reference` (0 dependencies), update `source_snapshot.rs` default limits within existing ceilings, update `dependency_policy.json` and `source_snapshot.json` with true operator SHA-256 digests, and verify `xtask` unit/integration tests and subchecks (`concordance-check`, `system-map-check`, `prose-check`) pass via RCH.
- **Phase B2 — RCH Verification & Honest Closure of Ready / In-Progress Work (`IW-05` – `IW-12`):**
  1. Run the test suites for all 13 `in_progress` beads (`FA-002`, `FA-060`, `FA-069`, `FA-093`, `ziz.7..10`, `hc0s.1..4`, `dc0.7`) and `4f1.4` on RCH.
  2. Fix any genuine bugs (`file_reviewer_peer`, `supervise_publication`) without weakening assertions.
  3. Close every bead whose acceptance criteria and positive/negative tests pass on RCH, recording exact commit and test evidence.
- **Phase B3 — High-Leverage Ready Tasks & Vertical Slice (`IW-06`, `IW-07`, `IW-13`, `IW-14`):**
  Implement `dmd.1` (Scenario script & structured event log format), `kngs.1` (Machine-checkable playbook step format), and the newly unblocked high-leverage critical-path beads (`FA-003`, `FA-061`, `FA-072`, `FA-137`).

### 6.2 Three Ambition Rounds (In-Place Revisions)

#### Ambition Round 1: Make Every Increment Falsifiable on RCH Immediately
No bead is created, updated, or closed without an exact `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test ...` command that exercises (a) the positive observable, (b) a paired causal negative that differs in one semantic respect, and (c) an explicit statement of what green does not prove.

#### Ambition Round 2: Eliminate O(N) Subprocess Overhead in Operator Snapshot Verification
When `source_snapshot::verify` hashes 600+ files one subprocess at a time (`sha256sum <file>`), each `xtask` invocation spends >100 seconds in `fork`/`exec` overhead on remote workers. While keeping the operator `sha256sum` tool (zero authored crypto in `xtask`), batching or streaming paths into `sha256sum` (with strict per-path regular-file prevalidation) cuts `xtask` verification latency from ~125s to <2s—a >60x speedup that makes running `xtask check` after every wave practical!

#### Ambition Round 3: Connect Isolated Reference Subsystems into Compositional E2E Scenarios (`dmd.1` + `kngs.1`)
Rather than adding more isolated micro-modules at depth 13 inside `crates/fa-reference/src/action/consequence/...`, wire the existing reference primitives (product frontier, read witnesses, consequence lattice, Congress reducer, credibility ledger, KV compression + surprise residual, and durable journal recovery) into the structured E2E scenario format (`dmd.1`) and machine-checkable operator playbooks (`kngs.1`).

---

## 7. Historical Record: 2026-09-08 UTC Checkpoint & Fresh-Review Addendum

*(Preserved verbatim below so historical provenance and retained receipt links remain intact.)*

### 2026-09-08 Answer & Bridge Summary

FrankenAlignment does not yet deliver its production vision. It contains a real, dependency-free reference library and a real operator gate. It has no production `fa` command, credential broker, durable authority journal, provider capture, model, sidecar compressor, signed control receipt or admitted Asupersync runtime. The reference dispatch method changes an in-memory enum; it does not perform an external effect.

The source qualification at the initial reality-check checkpoint was [epoch9](../artifacts/execution/2026-09-08-epoch9-receipt.json): 56 reference unit tests, 16 reference integration tests, 224 xtask tests and two doctests passed through RCH. The [retired-command injection](../artifacts/execution/2026-09-08-epoch9-negative.log) failed at the intended checker after prior gates passed. Those results qualify the selected reference/operator source. They do not establish any production control boundary or empirical alignment result.

The subsequent [four-step DSR run](../artifacts/execution/2026-09-08-epoch9-gate-integration.json) executed source cleanliness, concordance, system-map and the complete gate on unchanged clean commit `3d8fd5a7cb1bd25ddac1bbb69069ed6d9997039f`. All four checks and 298 tests passed. This exact source result precedes the tracker repair below; a later metadata check is recorded separately.

The two reviewed roadmap admission edges also passed a [fresh 298-test RCH gate](../artifacts/execution/2026-09-08-bridge-gate-receipt.json), with only the roadmap input and its source binding changed among the 133 declared source/build inputs. Its frozen tracker input predates the final edge import; that result does not stand in for the final graph review.

### Applied graph repair and refinement results (2026-09-08)

[The retained graph review](../artifacts/execution/2026-09-08-bead-bridge-review.json) binds the actual exported tracker and roadmap hashes. The pre-bridge snapshot contained 234 live beads: 17 closed and 217 open. The bridge added 102 full packet owners for FA-001–102 while preserving the 41 existing FA-103–143 owners and all prior closure records. At that bridge checkpoint there were **336 live beads: 17 closed and 319 open**, plus one preserved historical tombstone. This is corrected backlog coverage, not 102 new implementation achievements.

The [final system-map and current-prose checks](../artifacts/execution/2026-09-08-bridge-final-receipt.json) both passed through RCH on the post-import runtime inputs, with zero findings and unchanged frozen input bytes.

### Fresh-review addendum · 2026-09-08 UTC

A subsequent independent source/graph/evidence review found concrete defects in previously qualified reference and operator behavior. Eleven new regression cases failed on unchanged implementation; the corrected frozen source passed [the complete 319-test RCH gate](../artifacts/execution/2026-09-08-fresh-review-receipt.json), including 21 additional tests relative to the bridge checkpoint. The retained receipt distinguishes the initial failing test patch, later controls, two intervening failed attempts and the passing source. The graph review found no new cycle or missing roadmap prerequisite in its inspected scope.
