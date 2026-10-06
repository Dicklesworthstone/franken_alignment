# Forecast before original output sampling

Source implementation, not execution-qualified. This is a declared temporal
observation boundary for the existing FA-111 consistency monitor (plan 12.7).
It introduces no predictor, likelihood reducer, journal tag, request book,
configuration format, token override, runtime or dependency.

## Prompt boundary and acknowledged forecast

Register the existing owned-residual predictor before generation. Advance the
original generator through its complete fixed prompt, but not one continuation
attempt. Call `FileConsistencyObserver::begin_pre_output_request` with the exact
journal revision, numerical revision/position and external request key.

The method accepts only original Generating state at the exact prompt length,
with zero current AND cumulative sampling attempts and no accepted sample.
An incomplete prompt, already generated text, held/failed computation, a pending
numerical intent, interrupted source, paused recovery or a reset with previously
spent sampling work cannot masquerade as this boundary. The latest accepted
prompt residual drives the original ForecastHostedRequest transaction. No next
model token or random draw is computed to obtain the forecast.

Success returns a non-cloneable `FilePreOutputForecast` only after that original
transaction acknowledges. It is supervisor-side data and progression, not an
actor ticket, source, observer role, generator owner or either publication key.
Original prediction refusal remains a distinct acknowledged inner error. The
existing general pre-action forecast methods remain unchanged: they do NOT
inherit this stronger before-sampling contract merely because this path exists.

## Original continuation and source-only admission

`FilePreOutputForecast::advance` advances one step via the original
`advance_learned_generation`. It checks the actual pending request/attempt,
immutable deadline, authority epoch and exact previous numerical inspection on
every call. Interleaved clock/source transactions are allowed with a fresh
caller-observed journal revision; a different numerical step, consumed/replaced
forecast, source interruption or lost coverage cannot be silently adopted.

Intent and witnessed outcome still have two distinct storage acknowledgments.
Only acknowledged original output is returned. An inner numerical failure or a
held step is retained; it cannot retry from an older quiet state. The existing
replay cache, numerical algorithm, sampler and work budgets are unchanged.
Individual steps and storage operations remain synchronous and unbounded by a
wall-clock latency guarantee. Observed time comes from the host, not a watchdog.

After completion, submit the generated result under request() through the
existing source-only raw-text or stream actor port. That original transaction
classifies the real output and records the single likelihood observation.
Subsequent congress, independent human approval, source validation and
publication still apply. This boundary forecasts a registered category of the
complete later output, not correctness, maliciousness or every generated token.
Calibration and conditional validity for this boundary require separate evidence.

Dropping this process-local handle does not answer or cancel the durable
forecast. Its deadline and coverage-loss rules remain, and abandoned work is
not refunded. Recovery loses the handle, replays the independent recipe and
records pending-forecast coverage loss; it supplies no replacement observer,
error budget, sampled continuation or reconstructed approval key.

## Verification

Nine regression functions use original model/sampler, congress fixture and
canonical journal files. They cover exact output and journal parity with the
unwrapped path, prompt/late-call refusals and a positive control, binding and
pending intent, interleaved time, competing numerical work, consumed forecast,
expiry/recovery, real monitor hold, every forecast-write barrier and both
numerical write stages at all five barriers. Two compile-fail examples deny
cloning and authority extraction. Synthetic parameters are controls, not a
calibration, detector-effectiveness, containment or deployment result.

The targeted and required full RCH gate must execute on the final revision.
The current environment has no rch or Rust executables; compilation, tests,
rustfmt and Clippy are unexecuted. No local compiler fallback, passing check,
performance result or bead closure is claimed. Logs are kept outside source.
