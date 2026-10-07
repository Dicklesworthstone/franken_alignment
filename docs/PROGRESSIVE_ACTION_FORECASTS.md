# Source-certified progressive action forecasts

Source implementation for FA-111 / plan 12.7 and 13.2, using the original
progressive binary32 observation and linear-probe contracts. Execution remains
unqualified. This is not forecasting over learned low-rank latent codes, a new
calibration model, or a claim of trained-detector effectiveness.

## Precision bought only when the band is unresolved

`ForecastModel::with_progressive(ProgressiveForecastPolicy::new(initial_bits,
refinement_bits, maximum_bits, max_encoded_bytes)?)` freezes a bounded precision
ladder. `predict` still uses the original model, coefficients and three registered
probability pairs. The default model retains its exact 23-bit baseline.

Each level encodes an actual initial or delta packet, verifies it against the
same immutable SourceFrame, and applies it through the original ProgressiveFrame.
The original exact integer accumulator evaluates the source-containing interval.
A strictly positive or strictly negative interval selects the corresponding
registered probability pair; exact [0, 0] selects the equality pair. Otherwise
only the next fixed precision level may run. No midpoint, omitted-zero value,
MSE certificate or later output is substituted for the missing observation.

An unresolved interval at the precision cap returns Incomplete. Insufficient
packet-byte budget returns Limit before encoding/scoring the next level. There
is no uncharged exact retry. Through the original broker, an admitted prediction
failure closes coverage and uses its existing optional terminal-stop policy.
Refinement levels do not create more prediction jobs or likelihood observations.

## Costs and scope

The policy admits at most 24 levels. Prediction::encoded_bytes includes EVERY
initial/delta header and rounded payload, not merely the last packet or the
hypothetical final prefix. It excludes allocator overhead and verification's
additional encoding copies. Multiple small deltas can exceed the exact baseline's
bytes. This is not a universal compression or throughput improvement.

The capture owner already has the actual residual; this does not remove its
acquisition or host memory cost. Every codec/probe call remains synchronous.
A successful coarse result certifies the SAME registered band as exact source
recovery, not an exact score or the accuracy/calibration of its probabilities.
Likelihood arithmetic, event categories, temporal ordering and authority are
unchanged. A quiet forecast is not permission.

## Authored tests

Eight regression functions pair coarse strict-sign controls with rare subnormal
signals, equality, mixed signs, catastrophic cancellation, and insufficient
precision/bytes. They compare every successful result with the original exact
predictor, account for all packet headers, enumerate fixed precision ladders,
and compare exact likelihood factors/evidence including the first crossing.
All prior tests remain unchanged. These are synthetic numerical controls.

Required targeted and full RCH execution cannot launch because rch is absent
(exit 127). Compilation, tests, rustfmt and Clippy are unexecuted; source and
hash checks are not runtime qualification. No bead or execution gate is closed.

## Durable configuration and the original consumer

`FileConsistencyConfig::with_progressive_forecast(policy)` selects this mode
before installation. Version 6 contains exact prior predictor bytes and the four
precision/byte fields. Stop remains outermost; progressive wraps optional timing
and message semantics. All supported builder orders are canonical. Versions 1
through 5 retain their prior encodings and exact-precision behavior. Nested,
reordered, truncated or oversized wrappers cannot silently select weaker limits.

Building this config changes only the original ForecastModel's acquisition
policy. The original broker still admits one forecast job, runs prediction on
its original source, freezes one pending request and observes its category once.
Internal refinement is not another forecast, sampler draw or likelihood sample.
An admitted limit/precision failure flows into the existing coverage-loss and
optional stop handling; callers cannot increase precision to retry that job.

The same selected mode works on the original owned prompt residual. Pinned
pre-output recipes bind its bytes before installation and on recovery. Required
forecast-before-sampling checks, original generator witnesses, source-only text
and stream intake, deadlines, review, both keys and publication remain unchanged.
A stream Message still cannot authorize Finish before receipt confirmation.
Recovery with changed or absent precision/byte limits refuses before cleanup.
A matching recovery keeps generation paused and unanswered coverage lost.

Eight additional regression functions cover independent version-6 bytes, all
24 supported wrapper orders, malformed encodings, actual prompt forecasting and
generated output versus the exact original, one-shot ticket retries/cancellation,
precision/byte failure with a permitted equality control, recovery substitution,
all five forecast-write failure barriers and original stream admission. The
complete set contains sixteen authored regression functions, none executed.
Required final targeted and full RCH attempts still fail to launch (rch absent,
exit 127); this is not a release or runtime/performance qualification.
