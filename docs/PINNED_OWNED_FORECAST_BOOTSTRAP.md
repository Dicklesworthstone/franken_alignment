# Required owned-code forecasting in the first generator state

This closes the installation gap between the learned generator recipe and the
required pre-output K/V forecast lane (FA-111). It reuses the original decoder,
source checker, forecast gate, two-write sampling path and canonical storage.
The feature is source-complete for the interfaces described below, but execution
validation is outstanding. No calibration or production qualification is claimed.

## One independently retained recipe

`FileLearnedConfig::with_required_owned_pre_output_forecast` binds the complete
`FileLearnedConsistencyConfig` into the original text recipe after mandatory
sidecar selection. The prediction must already select owned generation and
required pre-output timing, with exactly matching text/stream classification.
A raw-residual pin and an owned-code pin cannot coexist. A duplicate, optional
predictor, supplied source, missing sidecar or stream mismatch refuses.

The comparison encoding wraps the complete previous recipe and exact predictor
bytes under `FALBOOT` version 1, the existing zero discriminator and the distinct
`FALKPRD` version 1 domain. Previous recipes and forecast encodings stay unchanged.
The source/probe profile, dimensions, model layer and stream are checked by the
original bootstrap reducer, not duplicated by a second validator or trusted from
an archive. A malformed actual binding cannot create a writable weaker owner.

The original learned `Enable` installs its source and then applies the original
owned-prediction `EnableLearned` reducer within that SAME transition. No separate
optional predictor event may be omitted to weaken the independently bound recipe.
There is no extra initial snapshot, source importer, numerical engine or journal.

## Existing creation and attachment interfaces

`FileOversight::create_with_pre_output_forecast` accepts either the pinned raw
residual mode or this pinned owned-code mode. It still delegates to the original
text/stream first-image constructor and returns observer and human custody only
after canonical acknowledgment. No forecast, draw, clock or approval is invented.
Ordinary pair-returning text constructors also enforce the pin, but do not issue
an observer; choose the triple-returning constructor to receive that custody.

`enable_learned_generation_with_pre_output_forecast` installs the same recipe in
an existing provisioned owner. The existing actor gateway remains the request
owner. A failed write returns no observer and leaves the canonical image old or
fully pinned; it cannot acknowledge a generator lacking its predictor. Duplicate
attachment cannot issue another observer. Identity or other prerequisites must
already exist; this attachment is not a bypass of their original bootstrap law.

## Reconstruction, not re-arming

Generation-only reconstruction now has enough independently supplied information
to reinstall this nested gate: both configurations are in the exact recipe.
It still returns no consistency observer and does not restore live eligibility.
The owned-predictive basic and guarded recovery interfaces additionally compare
the separately supplied expected predictor against the pinned one. They reject
weaker/different recipes, second standalone predictor installations and supplied
capture records rather than synthesizing or deleting history to make it match.
Legacy separately installed owned predictors retain their previous recovery path.

The original recovery fence preserves pending request identity, numerical intent,
likelihood and lifetime spend. An unanswered prediction remains coverage loss;
a new observer cannot manufacture another forecast or complete the held sample.
Generic recovery without the independent generation recipe still refuses it.

## Validation

Six authored regressions use the original model/codec/source-checker, predictor,
actor gateway and file owner. They cover first-image installation and independent
explicit-installation numerical parity; mutually exclusive source/timing/stream
contracts; failure before creation; preserved gateway retry/cancellation; exact
recovery and unanswered forecasts; and all five original attachment-write faults.
Existing tests and numerical, likelihood, timing and storage algorithms are kept.

Targeted tests and the mandatory full gate were attempted through required RCH,
but `rch` is unavailable (exit 127). Compilation, tests, rustfmt and Clippy have
not run. Selected-preimage hashes and patch checks are not a full-checkout build.
No bead or execution gate is closed. Synthetic fixtures do not qualify forecasts.
