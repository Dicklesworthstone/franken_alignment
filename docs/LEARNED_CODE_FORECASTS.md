# Source-checked forecasts over learned K/V codes

Source implementation for FA-111, plan 10.15, 12.7 and 13.2. It reuses the
original learned codec, source-checker, selective refinement monitor, exact
linear-probe evaluator and registered probability table. Execution is unqualified.

## Distinct evidence; the same registered question

`ForecastModel::into_learned(LearnedMonitorBudget)` selects learned-code
acquisition explicitly and freezes its byte, coefficient-work, reconstruction,
materialization and refinement limits. A configured binary32 mantissa ladder is
rejected, not silently ignored or reinterpreted. The original exact/progressive
raw-source interfaces and their defaults remain unchanged.

`LearnedForecastModel::predict` takes an actual `CheckedLearnedKv` and `KvRow`.
It runs the ORIGINAL one-probe `LearnedRefinementMonitor`. The monitor certifies
a coarse interval when sufficient and uses its original dependency ordering and
source-checked XOR residuals only when needed. The shared original forecast-band
selector maps certified negative, positive and exact-zero intervals to the
registered probability pairs. It does not read a midpoint, treat absent source
values as zero, or use the fitted codec's MSE as a fidelity certificate.

`LearnedPrediction` retains the actual learned observation, source-bound view,
row/profile/sequence identity, registration identity and total monitor work. It
is deliberately not convertible to the raw `Prediction` with its mantissa
observation, nor to a permit. No fake raw-frame measurement is manufactured.

`LearnedForecastReport` keeps the original monitor report on both success and
refusal. Budget exhaustion yields `Limit`; unresolved evidence yields
`Incomplete`. Partial scoring, used residuals and missing groups remain
inspectable, but cannot supply a probability. Structural/evaluation failures
return an outer error with no completed report; zero cost must not be inferred.
There is no raw-source fallback or hidden retry with a larger allowance.

## Cost, calibration and integration scope

Monitor work includes the whole base representation plus each used residual,
including headers, and actual original coordinate/reconstruction/materialization
counts. It excludes fitting, compression, source-check preparation, already
retained residual storage, allocator overhead and wall-clock time. Those costs
remain in the original upstream reports. The supplied raw source need not be
retained inside the forecaster; the checked compact source retains its own
explicitly selected residual inventory. Every operation remains synchronous.

Successful forecasts certify the same registered band as exact reconstruction,
not identical score intervals or empirical probability calibration. This is
linear forecasting of original coordinates FROM learned codes with checked
error envelopes; it is not a newly trained nonlinear predictor on latent codes.

This increment is the numerical evidence component. It does not add a durable
learned-code forecast event or alter the live broker's raw-residual forecast
lane. Request binding, pre-output chronology, one-shot observation, lifetime
error allocation and publication authority are not supplied by this standalone
API. The existing `LikelihoodEvidence` can consume the resulting registered
pair in an explicitly governed consumer; cloning a report grants no right to
observe the same request twice. No source authenticity or model independence is
established by local source checking.

## Authored verification

Seven regression functions use the original decoder, disjoint training/evaluation
streams and fitted rank-one codec. They pair coarse decisions and missing
residuals with actual lost coordinates, positive/negative/equality controls,
subnormal products, all five resource limits, row/shape/profile refusal,
original-monitor parity and exact-predictor/likelihood parity. Two compile-fail
examples prohibit raw-evidence relabelling and effect permission conversion.
Synthetic weights and probabilities are causal controls, not calibrated data.

Required targeted and full RCH commands cannot launch because `rch` is absent
(exit 127). Compilation, tests, rustfmt and Clippy are unexecuted. Source/hash
checks are not runtime qualification. No existing assertion, dependency,
qualification gate or bead is weakened or marked complete.

## Shared caller allowances without rebuilding the forecast model

`predict_with_budget(source, row, remaining)` delegates to the ORIGINAL
monitor's componentwise intersection of the frozen model limits and the caller's
remaining shared allowance. Bigger supplied caps cannot enlarge a registered
limit; smaller caps stop the next operation with its original partial-work
report. Default `predict` uses this same path with the frozen allowance.

This is not a second resource ledger. The caller owns and debits its shared
allowance, including work in refused reports. An outer error has no complete
work receipt and cannot be accounted as zero. Reusing a checked source does not
install a pre-refined view or claim free base bytes: each call pays the original
base and each residual it actually uses. No persistent numerical or statistical
budget is replenished here.

Five additional regression functions pair shared-budget calls with the original
monitor and exact predictor. They exercise every frozen and remaining cap,
repeated-source costs, missing residuals, malformed allowances, and a constant
exact-equality control that still pays base bytes and coefficient visits. All
seven first-increment regression functions are unchanged. The total is twelve
regression functions and two compile-fail examples, still unexecuted because the
required RCH executor is absent. Live request/durable integration remains separate.

## Native broker integration

The numerical API above remains standalone. The native `OversightBroker` now
provides an explicit learned-source mode using its original pending slot,
attempt chronology, exact lifetime evidence, proposal consumption, deadline and
stop policy. Typed learned observations pass through the original congress and
publication gates. See [the broker contract](LEARNED_CODE_BROKER.md) for source
trust, refusal costs and the authored integration tests. Durable serialization
and the external-key pre-output driver remain separate; this is not a claim
that `FileOversight` can replay checked learned-code captures.
