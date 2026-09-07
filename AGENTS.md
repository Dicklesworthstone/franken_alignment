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
