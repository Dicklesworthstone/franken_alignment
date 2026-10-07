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
qualification gate is closed. Durable owned-source binding/recovery is not
provided by this broker-only increment.
