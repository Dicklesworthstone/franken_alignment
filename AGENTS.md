# Working on FrankenAlignment

Read the complete main plan, README, implementation status and relevant source audit before changing semantics. This is a design-first project, not a license to replace hard contracts with easier demos.

## Constitution

Use Rust 2024 and the latest qualified nightly. Every local crate must forbid unsafe. Asupersync is the runtime. The dependency universe is closed to admitted Asupersync/FrankenSuite packages and individually approved fundamental exceptions. Inspect the complete target-feature/build/runtime closure before adding anything; no C, ONNX, libtorch, Python or second-executor fallback. Do not infer a package identity from a directory name.

Observations, judgments and authority are different types. Never let missing evidence increase authority, copy rights into a branch, refund an unknown effect through cancellation, or restore an old revocation floor. Read witnesses include negative domains and semantics; opaque model judgment depends on the whole actual input view.

## Work packets

Use the dependency order in registry/roadmap.json. Mark a packet in progress when work begins and record exact source, local commands, result artifacts and negative tests before closing it. A source stub or document update does not satisfy an execution gate. Keep changes small and independently reviewable. Do not alter unrelated projects in a donor review.

## Local verification

Use `cargo run --locked -p xtask -- check`. The driver checks formatting, compiles, runs Clippy and runs tests. Source formatting must be done before freezing a release checkout. The initial lockfile guard admits only the two reference packages; implement full dependency admission before enlarging it.

There is no reliance on GitHub-hosted Actions. DSR invokes required checks on operator machines. Keep raw logs outside frozen source roots. A dry run is not execution, a missing target is not success, and previous evidence does not validate new code. The preparation environment did not compile this Rust workspace; never erase that historical fact by changing prose alone.

## Proof and benchmark discipline

Distinguish exact invariants, scoped proofs, bounded models, statistical evidence, operational targets and research hypotheses. Graph cuts are conditional on complete modeling; local gradients are not global bounds; approximate retrieval cannot prove absence. Benchmark cold cases, policy churn, failures, byte movement and memory, not just steady-state arithmetic.

No new production feature activates without its exact boundary/negative tests and source/format/epoch compatibility. Update the relevant contracts, status and changelog in the same logical change. Preserve the custom license verbatim.
