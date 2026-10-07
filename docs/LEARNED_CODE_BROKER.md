# Learned-code forecasts in the original action broker

FA-111 / plan 10.15, 12.7 and 13.2. This integrates the existing numerical
forecaster into `OversightBroker`; it does not replace the effect path or create
another likelihood process. Source and tests are authored, not execution-qualified.

## One owner and one lifetime

`enable_learned_action_consistency(LearnedConsistencyConfig)` fixes the original
forecast model, probability table, event domain, alpha, stream, job/age limits,
K/V layer and side, and per-forecast refinement budget at bootstrap. The original
configuration validator runs before installation. An existing consistency lane
cannot be converted, replaced or re-armed. Scalar progressive forecasts are not
silently reinterpreted as learned-code acquisition. The raw default is unchanged.

`forecast_learned_action(attempt, actor_revision, checked, row)` accepts only an
actual `CheckedLearnedKv`. It validates the fixed tap and the complete cache cut,
then shares the original raw forecaster's attempt, actor revision, profile,
stream, current-prefix position, source sequence, policy generation, epoch,
clock, pending-slot and job admission. Every layer must end at the selected row:
a historical row inside a cache with later tokens is refused before admission.

Numerical evaluation uses the original `LearnedForecastModel` and selective
refinement monitor. A successful report installs a typed learned prediction in
the original pending slot. An admitted report without a certified probability
spends its job and source-sequence floor, permanently loses coverage and installs
no pending forecast. Partial work remains available in the report. Outer errors
do not have a complete work receipt and must not be counted as zero. The original
optional stop policy runs after either result; containment failures remain in
its original incident.

## Observation is not permission

The original `propose` path classifies the actual proposed bytes, consumes the
pending forecast once, updates its existing exact `LikelihoodEvidence`, and only
then performs native action admission. A later quota/policy refusal cannot erase
the category or allow a second sample. Raw and learned observations occupy one
attempt map, not independent sample histories. `ConsistencyObservation` retains
its raw default type; `LearnedConsistencyObservation` carries `LearnedPrediction`.
The corresponding accessor refuses the other representation instead of inventing
a raw mantissa observation or panicking on a different source mode.

The same consistency checks still run during permitting congress review,
authorization and dispatch. A quiet forecast alone cannot deliver. Existing
human, activation, decoder, identity, policy and endpoint checks are unchanged.
The original deadline uses either prediction's actual source sequence; silence
retains the pending forecast, loses coverage and adds no likelihood sample.
Threshold crossings invoke the original optional containment policy and do not
turn unknown endpoint effects into absent effects.

## Boundaries and cost

This is the native, caller-supplied capture interface. It does not authenticate
the capture, prove that the caller obtained it before observing output, or add
an owned-decoder learned-K/V tap. The supplied/hosted raw interfaces cannot be
used to bypass a configured learned source mode. Fitted codec parameters and
source-checked error bounds certify the registered numerical band, not empirical
calibration, model independence, or maliciousness of an action.

The complete per-forecast budget and finite prediction-job count bound numerical
work. Fitting, compression, initial source checking, all retained residual storage
and allocator overhead remain separate upstream costs. This is not an RSS bound;
retained learned observations retain their source-bound views. No reuse credit is
claimed for recurring sources. Durable event serialization, recovery and the
external-key request/pre-output driver are not added by this native increment;
`FileOversight` retains its original raw/hosted journal formats and defaults.

## Authored verification

Five regression functions execute the original decoder and fitted codec in their
test bodies. They cover coarse and selectively refined decisions against the raw
broker; congress commit/reveal through the original endpoint; source/tap/cut and
attempt preflight; missing residuals and exhausted refinement; exact threshold
stops and unanswered deadlines; and a native refusal after the category was
counted. A compile-fail example forbids relabelling learned observations as raw.
No original test or assertion is changed.

Required commands:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference oversight::broker::consistency
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Both attempts stop before compilation because `rch` is absent (exit 127).
Compilation, tests, rustfmt and Clippy are unexecuted. Source/hash/patch checks
are not substitutes for that gate. No bead is closed or runtime claim qualified.
