# Experiment Preregistration Convention and Freeze Procedure

## Scope and Founding Basis

This directory holds frozen experimental protocols for the 21 research hypotheses (`H1`–`H21`) registered in `registry/claims.json` and tracked by the preregistration ledger `registry/experiments.json` (plan §§2.4, 20, 25.7; invariant `FA-INV-019`; founding idea `FI-A18`: no risk theater, including in the science).

A protocol file documents the exact experimental design before confirmatory data collection begins. Exploratory pilots and harness smoke checks must be explicitly labeled as exploratory and separated from confirmatory runs. Any result whose protocol digest was recorded after confirmatory data collection began (`frozen_on > result_collected_on`) is exploratory and cannot be promoted in `registry/claims.json`.

## File Naming Convention

- Every preregistered hypothesis `H<n>` (`n` in `1..=21`) has one canonical protocol document at `docs/experiments/H<n>.md`.
- `docs/experiments/TEMPLATE.md` defines the canonical structure and required headings.

## Required Sections

Every `docs/experiments/H<n>.md` file must include the following second-level Markdown headings (`## <Heading>`) in order:

1. `## Question` — The falsifiable scientific question, founding root (`FI-*`), and exact scope boundary.
2. `## Treatments` — The experimental arms, ladder rungs, or policy configurations under comparison.
3. `## Outcomes` — Primary and secondary metrics, exact denominators (both unconditional and attack-conditioned), and countermetrics fixed in advance (`FA-INV-019`).
4. `## Controls` — Matched-cost baselines, negative/contaminated controls, holdout strata, and invariant checks required on every trial.
5. `## Sample Size` — Planned trial counts, stratum minimums, and stopping rules fixed before confirmatory execution.
6. `## Confidence Method` — Statistical estimator, uncertainty interval construction, multiple-comparison handling, and handling of delayed or censored labels.
7. `## Kill Criterion` — Exact quantitative or structural falsification condition and the mandatory architectural consequence if falsified.
8. `## Bound Identities` — Exact model, tokenizer, codec, policy, dataset/split, and harness identities pinned by the protocol.

## Freeze Procedure

To preregister hypothesis `H<n>` before confirmatory data collection:

1. Author `docs/experiments/H<n>.md` from `docs/experiments/TEMPLATE.md` with all eight required sections populated and all denominators, countermetrics, and kill criteria fixed.
2. Compute the lowercase hexadecimal SHA-256 digest of `docs/experiments/H<n>.md`:
   ```bash
   sha256sum docs/experiments/H<n>.md
   ```
3. Update the `H<n>` entry in `registry/experiments.json`:
   - Set `"protocol_path": "docs/experiments/H<n>.md"`.
   - Set `"protocol_digest"` to the 64-character lowercase hex SHA-256 digest.
   - Set `"frozen_on"` to the UTC freeze date (`YYYY-MM-DD`).
   - Populate `"bound_identities"` with the non-empty list of bound identity keys/pins declared in `## Bound Identities`.
   - Keep `"result_artifact": null` and `"result_digest": null`.
   - Set `"status": "preregistered"`.
4. Verify the ledger with `cargo run --locked -p xtask -- check` (or `cargo run --locked -p xtask -- experiments-check`).

## Result Recording and Claim Promotion Rule

When confirmatory execution completes under a frozen protocol:

1. Retain the raw result artifact outside frozen source roots (for example under `artifacts/experiments/`) and compute its SHA-256 digest.
2. Record `"result_artifact"`, `"result_digest"`, and `"result_collected_on"` (`YYYY-MM-DD`) in `registry/experiments.json`, and transition `"status"` to `"executed"` (or `"exploratory"` if `frozen_on > result_collected_on` or if any bound identity drifted without a new preregistration).
3. The xtask experiments-ledger validator enforces that:
   - No hypothesis in `registry/claims.json` (`class == "hypothesis"`) may leave `"status": "unmeasured"` or attach `"result_artifacts"` unless its `registry/experiments.json` row has `"status": "executed"`, a verified `protocol_digest`, a verified `result_digest`, and `frozen_on <= result_collected_on`.
   - Any modification to a frozen `docs/experiments/H<n>.md` file invalidates its recorded `protocol_digest` and fails static validation unless re-frozen prior to confirmatory data collection.
