# Terminal recovery capacity for predictive learned streams

Required forecasts must not consume the publication operator's last opportunity
to fence a recovered owner, stop it and complete the stop sweep. The predictive
stream owner now composes with the original learned-stream recovery reserve.

## Create the complete first image

`FileOversight::create_with_pre_output_forecast_with_reserve` takes the original
profile, a complete learned STREAM recipe with its required raw-residual or
owned-K/V predictor, and an independently selected `RecoveryReserve`.

It delegates to the original stream bootstrap, which writes StreamBootstrap,
ReserveRecovery and learned Enable in one canonical replacement. Learned Enable
contains the complete pinned predictor. The host, human reviewer and separate
consistency observer are returned only after that replacement succeeds. Startup
does no inference, forecasting, sampling, clock admission or review.

A missing predictor or non-stream recipe refuses before creating storage.
Invalid or unfunded reserves return no owner and publish no canonical journal;
the original store may leave its empty directory and lock file. The existing
unreserved constructor and its journal/configuration bytes keep their meaning.

## Require the same reserve before recovery

Both `FilePredictiveLearnedRecovery` and `FileOwnedPredictiveRecovery` expose
`require_recovery_reserve`. Call it before the first replay quantum with the
independently retained reserve. It uses the same original locked event list and
the same exact-match check as ordinary guarded learned recovery.

Absent or changed reserves refuse without replay, canonical replacement or
staged-file cleanup. A late requirement also refuses. Successful reserve
admission does not bypass the full predictor, source, guard, effective-policy
and external-floor requirements checked by the original recovery wrapper.
Finish still validates the canonical cut, fences once and issues fresh process
custody. An observer from the old owner cannot forecast on the recovered owner.

The original numerical state and forecast evidence remain retained. A clean
recovery before forecasting needs fresh time and explicit numerical resume.
An unanswered forecast retains its request and loses coverage under the
existing recovery policy; a new observer cannot repair it by forecasting again.

## What the reserve buys

`RecoveryReserve::terminal()` reserves three events and fifty encoded bytes for
the original Fence, Stop and StopProgress lane. Ordinary time observations,
forecast work, numerical work, reviews and effects cannot consume that tail.
Recovery does not replenish it or create numerical, forecast or effect funding.
This is logical journal capacity, not physical disk preallocation.

The public regression fixture fills the ordinary event allowance with actual
prompt processing and a pending forecast. It then recovers, fences, stops and
drains at the exact terminal limit, preserving numerical work and publishing
nothing. Both predictor representations use the original implementations.

## Qualification

Five test functions in
`crates/fa-reference/tests/learned_text_stream_durable/continuation/forecast_reserve.rs`
cover original numerical/forecast parity, bootstrap refusal, exact pre-replay
reserve and floor checks, observer custody and exhausted-capacity terminal drain.
The predictor probabilities and model inputs are controlled fixtures, not
calibration or deployment evidence.

As of 2026-10-12 these tests are authored and source-reviewed, but unexecuted.
The required remote build launcher is absent in the restored workspace. No
compiler, target test or workspace qualification is claimed. The scoped bead
and FA-111 remain in progress; the original research acceptance is unchanged.

See [the original reserve contract](LEARNED_STREAM_RECOVERY_RESERVE.md),
[per-message forecasts](FORECAST_TEXT_STREAM_CONTINUATION.md) and
[owned continuation custody](OWNED_PRE_OUTPUT_CONTINUATION.md).
