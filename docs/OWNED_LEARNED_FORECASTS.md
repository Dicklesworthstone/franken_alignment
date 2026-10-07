# Forecasts from the owned generator's accepted K/V audit

This extends the original FA-111 pre-action predictor (§12.7) with an in-process
owned source. It does not create a numerical engine, observation ledger, error
budget or publication key. It reuses the exact learned predictor, monitor and
lifetime accounting already used by supplied checked-source forecasts.

`enable_owned_learned_action_consistency` and its `_with_limits` variant bind the
selected layer, K/V side, tensor profile, dimensions and source stream to the
existing original learned generator. All binding happens before any token work;
position zero alone is insufficient if cumulative admitted work is nonzero.
The selected mode is held outside numerical checkpoints, with no conversion,
disable or budget-refill operation. Failed bootstrap leaves the old owner intact.

`forecast_owned_learned_action(attempt, actor_revision)` accepts no source, row,
sequence, model, probability or timestamp argument. The original latest event
must have an accepted step, matching status/position/token and a complete quiet
audit. The shared source must be Ready and the host synchronization failure latch
clear. Empty, held, failed or interrupted generation cannot use an older quiet
event. The row is selected at the actual accepted token and the frozen layer/side.
The original broker then enforces complete-cut, source-sequence, actor revision,
clock, pending-attempt, job, lifetime and inventory requirements.

Forecast acquisition shares the original immutable checked audit. It does not
recompute a token, fit another codebook, spend a sampling draw or reconstruct an
unchecked cache. The predictor nevertheless pays its normal complete-inventory
charge and begins at the original coarse view: a generation monitor's earlier
refinement does not give it a free refined forecast. Failed predictions preserve
actual reported work and close coverage through the existing consistency lane.

The supplied-source forecast API refuses this mode, even for byte-identical
captures. A successful forecast does not replace whole-input congress or the
independent human key. It also does not claim that generation's next sampled
output is bound to this forecast; required pre-output timing is a separate
contract. External process authentication and calibration are separate claims.

Six regression functions use the original numerical model/codec/monitor fixture,
independent generation, the original predictor and the actual two-key broker.
They cover numerical/audit parity with real selective refinement, no extra draw,
complete congress-to-endpoint publication, invalid and late registration,
source-substitution and stale-call refusal, held/failed source loss, and work
and inventory ceilings across consecutive tokens. Existing tests are preserved.

The targeted and required full RCH commands were attempted but cannot launch:
`rch` is absent (exit 127). Compilation, tests, rustfmt and Clippy are unexecuted.
Selected-context hash/patch checks are not execution evidence. No bead or
qualification gate is closed. The durable extension below uses this same broker source.


## Durable request binding and replay

`FileLearnedConsistencyConfig::with_owned_generation()` selects version two of
that configuration. Its predictor parameters, stream classification, stop policy,
per-job and lifetime caps stay byte-identical to version one after the domain.
The selection is still validated against the actual installed learned generator
before work. Existing configurations and raw/supplied event tags are unchanged.

The original `FileConsistencyObserver` now has
`forecast_owned_learned_action` and `forecast_owned_learned_request`. These take
only the journal predecessor, attempt/request identity and actor revision. The
original machine selects the latest accepted audit at live execution and replay;
no source or row can be substituted through another forecast event. A keyed
forecast binds the existing next-attempt allocator only when its numerical report
certifies a prediction. The actor cannot consume it under another request key.

New event tags 10 and 11 record those intents plus the existing bounded outcome
and accounting comparison witness. They do NOT embed exported checked captures
or import fidelity bounds. Original generation events already reconstruct the
actual source through numerical execution and source checking. The original
transaction poisons the live owner before prediction; final witness encoding,
capacity checks and canonical replacement all precede any returned report.
Refused numerical reports and consumed observed categories retain their original
one-shot semantics. No separate journal, numerical reducer or likelihood process
is added. Existing proposal, whole-input congress, human approval and checked
publication remain the effect boundary.

`open_with_owned_learned_consistency` requires both the independently retained
`FileLearnedConfig` recipe and the intended owned `FileLearnedConsistencyConfig`.
It binds both configurations before replay or cleanup, rejects supplied-capture
records, and reconstructs each source from the original numerical history. Every
saved numerical/outcome witness is compared, not loaded as state. Generic open
and generation-only recovery cannot hydrate the missing forecast configuration.
Only after the original recovery fence is acknowledged are new human/observer
roles returned. Generation stays paused, clocks are historical, old approvals
are invalid, unanswered forecasts retain coverage loss, and logical spend remains.

`read_publication_with_owned_learned_consistency` uses the same independent
binding and one canonical image to return publication, learned costs/evidence
and pending-request identity. It works under an unavailable writer's retained
lock without cleanup, fencing, source refresh, new roles or repeated publication.
Query-only settlement after recovery requires no generation resumption or new
forecast and preserves charges for outcomes that remain unknown.

Eight additional regression functions use the original learned-generation and
file-oversight fixtures. They exercise request-key isolation and exact retries,
fresh forecasts after a fully observed recovery, congress/human publication,
unanswered forecasts, generic-recovery refusal, all five forecast-write and
atomic-publication barriers, unpublished versus visible-unreconciled dispatch,
substituted recipes/configurations/outcome witnesses, exact framing/truncation,
numerical-budget refusal and interrupted source admission. They remain unexecuted.

This is an owned-source pre-action lane, not mandatory pre-output forecasting:
the older hosted-raw timing configuration still cannot be relabelled as a
learned-source contract. The new recovery method is synchronous and requires
independent generator/forecast inputs; it is not the complete guard-inventory
recovery API. Replaying repeats physical numerical work even though logical
budgets are restored exactly. Source ownership does not establish model-identity
qualification, remote authentication or probability calibration.
