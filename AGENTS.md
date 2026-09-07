# Working on FrankenAlignment

Read the complete main plan, README, implementation status, the founding-ideas concordance (docs/FOUNDING_IDEAS.md), the system map (docs/SYSTEM_MAP.md) and relevant source audit before changing semantics. If you are operating a deployment rather than developing the code, docs/AGENT_GUIDE.md is your entry point. This is a design-first project, not a license to replace hard contracts with easier demos.

## Founding essays

The two essays by Jeffrey Emanuel (some_thoughts_on_ai_alignment, June 2024; llm_introspective_compression_and_metacognition, April 2025) are the premise of this project, not a citation. Every mechanism, invariant, hypothesis and packet traces to a founding idea, a synthesis of two, or a labeled engineering addition that serves one, in docs/FOUNDING_IDEAS.md and registry/founding_concordance.json. A change to plan semantics that leaves the concordance stale is incomplete. Do not describe an essay idea as borrowed from later literature; docs/RELATED_WORK.md records the chronology.

## Constitution

Use Rust 2024 and the latest qualified nightly. Every local crate must forbid unsafe. Asupersync is the runtime. The dependency universe is closed to admitted Asupersync/FrankenSuite packages and individually approved fundamental exceptions. Inspect the complete target-feature/build/runtime closure before adding anything; no C, ONNX, libtorch, Python or second-executor fallback. Do not infer a package identity from a directory name.

Observations, judgments and authority are different types. Never let missing evidence increase authority, copy rights into a branch, refund an unknown effect through cancellation, or restore an old revocation floor. Read witnesses include negative domains and semantics; opaque model judgment depends on the whole actual input view.

## Work packets

Use the dependency order in registry/roadmap.json. Mark a packet in progress when work begins and record exact source, local commands, result artifacts and negative tests before closing it. A source stub or document update does not satisfy an execution gate. Keep changes small and independently reviewable. Do not alter unrelated projects in a donor review.

The granular task graph lives in `.beads/` and is managed only with the `br` CLI (`br ready --json`, `br update <id> --status in_progress`, `br close <id> --reason ...`, `br dep add <child> <parent>`) and triaged with `bv --robot-*` flags; never run bare `bv`. Beads reference their roadmap packet, plan sections and founding ideas; the roadmap remains the packet-level authority. After changing beads run `br sync --flush-only` and commit `.beads/` yourself; `br` never runs git.

## Local verification

Use `cargo run --locked -p xtask -- check`. The driver checks formatting, compiles, runs Clippy and runs tests. Source formatting must be done before freezing a release checkout. The initial lockfile guard admits only the two reference packages; implement full dependency admission before enlarging it.

There is no reliance on GitHub-hosted Actions. DSR invokes required checks on operator machines. Keep raw logs outside frozen source roots. A dry run is not execution, a missing target is not success, and previous evidence does not validate new code. The revision 0.2 preparation environment did not compile this Rust workspace; never erase that historical fact by changing prose alone. On 2026-09-06 the complete gate passed on one operator host under a dated nightly after the documented `cargo fmt --all` step; the retained logs under artifacts/execution/ are the evidence, and a new run is required for new code.

## Proof and benchmark discipline

Distinguish exact invariants, scoped proofs, bounded models, statistical evidence, operational targets and research hypotheses. Graph cuts are conditional on complete modeling; local gradients are not global bounds; approximate retrieval cannot prove absence. Benchmark cold cases, policy churn, failures, byte movement and memory, not just steady-state arithmetic.

No new production feature activates without its exact boundary/negative tests and source/format/epoch compatibility. Update the relevant contracts, status and changelog in the same logical change. Preserve the custom license verbatim.

## Legibility

The system is one tower of nine layers (plan §6.7) with one epistemic type, one address scheme, one journal and one verb vocabulary (registry/system_map.json, registry/vocabulary.json). A new object goes in exactly one layer; a new agent-facing value is a Knowledge variant, never a bare scalar; a new command is a registered verb with authority, class, idempotency and cost, and any playbook that names it is updated in the same change. Prose explanations render from predicate trees, never the reverse.

## Swarm execution and honest credit

For the operator-requested NTM campaign, use parallel code-first waves and one central batch verifier. Each worker atomically claims one assigned bead with `br update <id> --claim --actor <AgentMailName>`, reserves its narrow source/test paths through Agent Mail, and implements real code and meaningful positive and negative tests together. Preserve peer and pre-existing staged/unstaged bytes; never reset, stash, unstage, or commit another owner's changes. Shared manifests, registries and status documents have one designated integration owner.

Workers do not run test suites or full builds during a code wave. All compilation uses `RCH_REQUIRE_REMOTE=1 rch exec -- ...`; no local fallback. A syntax check is the worker maximum and requires the verifier's build-slot assignment. Commit only owned paths, report the exact revision and acceptance-to-test mapping, and leave work open pending verification. Use `batch_pending` only if the installed tracker actually supports it; otherwise retain `in_progress` with a pending-verification comment. The verifier runs `cargo run --locked -p xtask -- check` through RCH against frozen source, retains every attempt outside the frozen root, examines test/gate changes independently, and alone closes fully satisfied beads with exact revision-bound evidence. Verification triggers on a critical prerequisite becoming ready, bounded pending debt, scope growth or elapsed risk, not commit count alone.

Working capability is the objective. A process artifact may be created only with a named consumer, feature gate, observed defect class and retirement condition; minimal recovery snapshots may prevent a named loss of evidence. Process output receives no capability credit. Runtime journals and receipts are feature work only when actual product behavior consumes them; inventing a consumer to evade this rule is forbidden.

No faked tests, fixtures presented as live proof, assertion weakening, golden regeneration to force green, hard-coded success, placeholder macros, or edits to the spec instead of implementation. Refusal-only work never closes a positive-capability item; pair forbidden cases with near-identical permitted cases. Do not move unmet acceptance conditions into follow-ups to close the original. Never self-certify or close to unblock peers. A false close is reopened with an incident comment. Name gate self-weakening, proof-class inflation, dependency smuggling, refusal farming, follow-up laundering and commit pumping when observed.

Each dispatch names a positive observable, a causal negative test and what green does not prove. Every claimed metric fixes its denominator and countermetric in advance; failures and retries remain recorded, and same-origin evidence counts once. Never silence stderr in evidence-bearing commands. Inspect pane/process truth, Beads, reservations and actual diffs before interventions; unknown state is not idle. Tracker policy is enforced only to the extent demonstrated by execution; unsupported controls remain the orchestrator's explicit responsibility.
