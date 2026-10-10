# Forecasting from an original learned reset successor

`FileLearnedConfig::with_forecast_reset_successors` opts an independently retained
recipe into following the original learned generator's restored source streams.
Call it after pinning either the required raw-residual pre-output predictor or
the required owned-K/V pre-output predictor, before creating the durable owner.
The predictor itself keeps its exact calibration, event domain, origin stream,
timing, likelihood process and lifetime ceilings.

```rust,ignore
let config = learned_text_config
    .with_required_sidecar()?
    .with_required_pre_output_forecast(required_predictor)?
    .with_forecast_reset_successors()?;
```

The equivalent owned-K/V configuration uses
`with_required_owned_pre_output_forecast` before the same final option.
`follows_forecast_reset_successors` reports the immutable selection. A duplicate
selection refuses. Unpinned predictors and cumulative text-stream owners cannot
enable it; cumulative streams retain their distinct frontier/reset obligations.

## Why this is a new recipe version

Successful original learned reset already creates a strictly newer numerical
stream. Legacy forecasting compares captures with the fixed origin stream, so
even a fresh accepted token from that restored generator receives `Binding`.
Some historical raw forecast events do not carry an outcome witness. Globally
relaxing their stream comparison would let an old acknowledged refusal replay
as a successful forecast.

The new `FALFRST` version-one recipe wrapper binds the opt-in to the original
canonical recipe bytes. Existing recipes and their historical refusals keep the
old behavior. Recovery must independently supply the same wrapped recipe;
adding or removing the option cannot rewrite an existing journal. This creates
no public runtime rebinding operation and adds no replacement predictor or
separate journal writer.

## Accepted continuation

1. Advance the original generator into a nonempty prompt prefix and capture an
   original paired checkpoint **before the prompt is complete**.
2. Complete the prompt without starting a forecast or sampling output. If reset
   is required, use the original reset intent and exact checkpoint/authority
   predecessors. An interrupted acknowledged intent can use the appropriate
   predictive recovery cursor and `finish_pending_reset`.
3. On recovery, observe fresh time and explicitly resume the paused owner. The
   restored source is Empty; a reset receipt cannot supply a forecast.
4. Execute the remaining original prefill. Its newly accepted, completely quiet
   event supplies the restored stream's current residual or checked K/V audit.
5. Obtain the original pre-output forecast and continue original sampling.
   Generated request admission, sidecar review, identity/source requirements,
   both publication keys and endpoint checks remain separate operations.

The broker accepts only the stream reported by its current original learned
owner. Existing source accessors verify the exact latest accepted event, model,
layer, dimensions, stream, sequence, position and complete quiet audit. The
forecast frontier is `(stream, sequence)`: sequence must advance within a stream,
and a newer original stream may begin at a restored prefix. That frontier is
updated only at the existing irreversible forecast-job admission point.

## What reset retains

No reset or source successor clears a pending forecast, lost coverage, a crossed
likelihood boundary, consumed jobs, retained observations, learned acquisition
work, retained-source bytes, unreported work, automatic stop or sampled-work
history. The unchanged required pre-output checks refuse abandoned output draws,
even when the old forecast was answered and coverage remains complete. The
original authority reset still narrows rights and counts incidents; unknown
effects stay charged until actual reconciliation.

A checkpoint at the completed prompt cannot supply a new accepted prefill token.
This option does not make that checkpoint eligible for predictive continuation.
It also does not rescue an unanswered forecast or permit sampling the same output
again under a fresh budget. These refusals are necessary boundaries of this
finite original continuation protocol.

## Regression coverage

The public `forecast_reset_successors` harness uses the actual learned numerical
model, tokenizer, original journals and guarded recovery APIs. It compares raw
and owned successor forecasts, all original sampled-token/RNG receipts and
generated request admission with independent original numerical execution. It
also checks replay of legacy refusals, exact recipe substitution, pending loss,
owned lifetime exhaustion and answered-but-abandoned sampling history. These
controls establish implementation behavior, not empirical forecast calibration
or production runtime qualification.
