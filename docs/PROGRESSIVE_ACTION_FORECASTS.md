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
