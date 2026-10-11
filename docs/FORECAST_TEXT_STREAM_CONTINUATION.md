# Required forecasts across finite learned message continuation

This opt-in reference integration implements part of FA-111 and founding idea
FI-A12: predict from the original pre-output state, then accumulate the realized
message as sequential evidence. It composes the existing required forecast owner
with receipt-bound finite continuation from plan sections 8.7, 12.7 and 13.2.
It does not establish predictor calibration or close the FA-111 research packet.

## Explicit configuration and compatibility

Start with a learned text-stream configuration and its required sidecar. Pin
either the existing required raw-residual predictor or the existing owned
learned-K/V predictor, then select:

```rust,ignore
let config = text_stream_config
    .with_required_sidecar()?
    .with_required_pre_output_forecast(required_raw_predictor)?
    .with_forecast_text_stream_continuation()?;
```

For the owned lane, use the existing owned predictor installer in place of the
raw installer. The new selection requires a stream, a required sidecar and an
already pinned required predictor. A duplicate continuation selection refuses.

The new immutable wrapper is `FALMPRD\x01`. It leaves old configuration bytes
and their interpretation intact. The original
`with_text_stream_continuation()` still refuses required forecast composition;
the new behavior cannot be installed implicitly into an old store. Recovery
must independently provide the complete exact original configuration.

The executable publication recipe `/4` remains the existing ordinary finite
continuation route. This change adds the explicit public library API; it does
not add predictor provisioning to that executable recipe.

## Original source and exact message boundary

The first message is forecast after the complete prompt and before any sampled
token. A later message follows a confirmed Executed receipt for the previous
complete message. The original durable continuation command records the
predecessor request, successor request, actor revision and numerical position
before additional inference.

A forecast for the successor must match that exact retained request and
numerical cut. Missing, wrong-request or late forecasts refuse before another
sample or canonical write. The common live and replayed step admission checks
the same obligation, including cooperative and cache-finalization routes.

Continuing changes the original generation's status from accepted stop to
generating without running a token. Only this explicitly selected mode can use
the preceding accepted stop's original quiet audit at that boundary. The source
owner checks the original output offset, complete accepted-token history,
sampler count, stop token, numerical position and confirmed audience basis.
The ordinary residual and owned K/V extractors still enforce the accepted step,
complete quiet audit and exact source identity. No replacement source is supplied.

Each forecast measures actor progress from its own message boundary. Sampling
and cumulative numerical work are still checked against the original prompt.
The continuation changes neither model nor tokenizer, appends no new prompt,
and replenishes no inference, telemetry, source-retention or probability budget.

## Message, finish and lifetime evidence

Message and finish are distinguished using the original release frame and
receipt-confirmed stream state. A position later than the prompt is insufficient
to call an action a finish: later messages also use later source positions.

A pending message forecast cannot be spent as a finish. Conversely, a forecast
made at the prior confirmed receipt cut is a finish obligation and prevents a
continuation from reinterpreting it as a new message forecast. Finish remains a
separately forecast, reviewed and charged effect, with no additional inference.

The existing consistency owner accumulates all message and finish observations
in one lifetime sequence. Consumed samples, spent work and retained source costs
are conserved. Expiry or coverage loss does not refund them. Quiet evidence
grants no permission: the original sidecar, committee, human approval, witness,
currentness, dispatch and endpoint receipt requirements still apply.

An unanswered forecast surviving recovery retains its original obligation and
evidence while losing live coverage under the existing fail-closed recovery
contract. This change does not restore permission or make that interrupted
forecast resumable. Changed configuration refuses before storage mutation.

## Tests and qualification boundary

The public durable-stream tests in
`crates/fa-reference/tests/learned_text_stream_durable/continuation/forecast_messages.rs`
cover both required raw and owned predictors. Their positive control produces
A, stop, B, stop from the original numerical owner, compares its retained
numerical history to uninterrupted generation, and separately reviews and
publishes both messages and finish. The forecast table and committee ballots
are deliberate deterministic controls; they are not native committee inference
or evidence of useful surprise detection.

The causal cases cover missing, wrong-request and late forecasts; exact raw
handle actor/position binding; a finish forecast that cannot fund continuation;
unconfirmed publication; expiry without a work or likelihood refund; and pending
recovery plus changed-recipe refusal without writes. Existing legacy refusal
tests remain unchanged.

These new tests have been authored and source-reviewed, but have not executed.
The prescribed remote-only RCH gate could not start compilation: no workers were
configured and the daemon was unavailable. The workspace executor subsequently
disconnected, also preventing formatter and targeted test execution. No syntax,
formatting, compilation, test or full-workspace qualification is claimed.
FA-111 and the scoped continuation bead remain in progress.
