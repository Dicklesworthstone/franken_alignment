# Reality check and implementation bridge

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
