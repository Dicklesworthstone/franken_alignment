# Learned consistency: lifetime acquisition and retained evidence

This extends the original learned-K/V action consistency lane. The numerical
model, one-shot attempt admission, likelihood process, congress, deadlines,
containment and effect endpoint are unchanged. It does not introduce authority.

## Frozen budgets

`enable_learned_action_consistency` retains its configuration type and per-job
semantics. It derives a finite lifetime ceiling by checked multiplication of
each per-job allowance by `max_predictions`. Its retained-source ceiling is
`MAX_CHECKED_KV_BYTES * max_predictions`, also checked for overflow.

`enable_learned_action_consistency_with_limits(config, lifetime, bytes)` allows
supervisor-selected, tighter lifetime budgets at bootstrap. Zero is a real zero
allowance, never an unlimited sentinel. Configuration failure is atomic. An
installed lane cannot be replaced, reset or given a new allowance.

Each admitted evaluation intersects its remaining lifetime allowance with the
original model's per-job cap before calling `predict_with_budget`. The broker
debits the original completed receipt's encoded bytes, probe coordinates,
reconstruction products, materialized values and refinements. Completed refusals
consume their actual partial work too. Counts never use saturating arithmetic.

## Retained evidence and failure handling

The broker reserves the complete checked source inventory before numerical work,
including residuals that the prediction does not need. Repeated sources are
charged again without assuming deduplication or sharing. This is a conservative
inventory charge, not an exact process-memory measurement. Report and observation
counts remain bounded by the original job and likelihood-sample ceilings.

Eligibility closes before numerical work or report allocation. Only a completely
accounted, installed pending prediction reopens it. A storage-cap refusal spends
the admitted job and source-sequence floor but does not create a numerical receipt
or likelihood sample. A completed numerical refusal preserves its original report
and leaves coverage lost. The original optional containment policy still runs.

`learned_consistency_report(attempt)` exposes retained typed receipts, including
refusals. `learned_consistency_work()` gives completed numerical totals;
`learned_consistency_retained_source_bytes()` gives inventory charges;
`learned_consistency_has_unreported_work()` identifies an admitted numerical or
accounting failure without a complete trusted receipt. Such failures leave the
reservation charged and cannot be retried. Even complete numerical totals exclude
upstream fitting, capture, source checking, and containment costs.

## Validation and remaining boundary

The added regression functions reuse the existing real decoder, learned codec,
source checker and broker fixture. They cover every lifetime-work dimension,
refused receipts, full retained inventories, zero budgets, atomic configuration,
two consecutive forecasts, and integer overflow/overdraw. Existing endpoint and
congress regression tests remain intact.

The required RCH checks were attempted in the authoring environment but could
not start because `rch` is absent (exit 127). Compilation, Rust test execution,
rustfmt and Clippy have NOT been verified. No bead or qualification gate is closed.

This is still the in-memory, supervisor-supplied checked-evidence path. Durable
learned-event recovery, external-key pre-output integration and an authenticated
owned-decoder learned-K/V tap remain separate, incomplete integration gates.
