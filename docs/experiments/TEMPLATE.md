# H<n> — <Short Hypothesis Title>

- **Hypothesis ID:** `H<n>`
- **Founding Roots:** `<FI-...>`
- **Plan Sections:** `<§...>`
- **Preregistration Status:** `template`

## Question

State the exact falsifiable question and the scope boundary of the experiment. Identify what a positive result establishes and what it does not prove.

## Treatments

List each experimental treatment arm, baseline configuration, and parameterization to be evaluated at matched budget.

## Outcomes

State the primary outcome metric, secondary metrics, exact denominators (unconditional and attack-conditioned), and mandatory countermetrics (`FA-INV-019`). Include cold-start, policy-churn, byte-movement, and tail-latency accounting where applicable.

## Controls

List matched-cost baselines, sham/random-feature controls, contaminated-arm exclusions, held-out strata (model revisions, prompt families, mechanisms), and exact runtime invariants verified on every trial.

## Sample Size

Specify minimum trial counts per stratum, power or precision rationale, and any sequential stopping or early-termination rule fixed before confirmatory execution.

## Confidence Method

Specify the uncertainty estimator (e.g., stratified bootstrap confidence intervals, exact binomial bounds, permutation tests), multiple-testing adjustment across arms/strata, and rule for censored or delayed outcomes.

## Kill Criterion

State the exact falsification threshold or qualitative failure condition, and the mandatory engineering fallback if the hypothesis fails.

## Bound Identities

- `protocol_schema`: `fa.experiment_protocol/1`
- `harness_target`: `<crate/binary or test target>`
- `model_family`: `<pinned model/checkpoint identity>`
- `tokenizer_epoch`: `<pinned tokenizer identity>`
- `codec_profile`: `<pinned sidecar/codec profile>`
- `policy_epoch`: `<pinned policy/authority fixture epoch>`
- `holdout_manifest`: `<pinned holdout split digest or identifier>`
