# Original-token, all-layer probe campaigns

Consumer: an operator fitting the fixed residual inventory consumed by the
existing compulsory MonitoredDecoder (FA-024/025/028 and FI-A05/FI-I05).
This connects the actual decoder to the existing three-way ProbeCorpus,
class-balanced logistic optimizer, exact emitted-score calibration and untouched
evaluation. It does not replace or weaken those implementations. The older
prepared two-split trainer is not installed alongside this implementation.

## Capture

DecoderCorpus::capture validates every original token ID, context, declared task
and lineage, and both class denominators in Training, Calibration and Evaluation
before numerical execution. Tasks and lineages are unique over the whole corpus.
Task IDs determine unique streams. Input order cannot change processing order.
Every prefix executes from its first original token through the original decoder;
only its final computed residual from each and every layer enters that layer's
immutable corpus. No text retokenization, alternate engine or supplied classifier
score is accepted. Exact original prefixes remain available for audit.

CaptureBudget is shared across cases and repeated calls. It charges aggregate
original-token retention, final residual coordinates, case count and numerical
product allowance before the first numerical operation. Malformed input or
insufficient admission leaves it unchanged. An admitted numerical failure returns
no partial corpus and does not replenish its allowance. This is planned-work
admission, not measured failed-run operations, peak memory or wall-clock bounds.

## Fit, calibrate, evaluate

Each layer has explicit immutable FitPolicy and CalibrationPolicy. The complete
roster must match the actual model. The existing fitting implementation reads only
Training values; the existing calibration picks among declared thresholds using
Calibration; the existing final evaluation consumes all Evaluation cases without
refitting or retuning. Class counts remain per split and layer. Repeated origins
across layers are not independent additional statistical samples.

CampaignBudget admits all fitting, threshold comparison and final evaluation
work across the entire model before the first fit. It is persistent across calls
and is not renewed for each layer. A hard numerical error returns no partial
campaign; its full charge remains. The report distinguishes admitted upper-bound
work from completed scoring work when a layer has no eligible threshold.

Statistical failure is retained as a result. A missing calibrated threshold does
not stop other layers from completing. A failed untouched evaluation remains in
the original EvaluationReport with its denominator and original threshold.
DecoderCampaign::probes refuses unless the entire frozen layer roster passed.
There is no operation to discard a failed layer, substitute a threshold or export
a partial passing roster as a complete detector. Low-level candidate probes remain
available through the pre-existing research API; this is not policy promotion.

## Verification boundary

Ten new integration scenarios and one compile-fail example cover positive
learning, separate full-decoder score comparison, malformed late cases, split
violations, every budget dimension, persistent charges, numerical failures,
unchanged fitting/calibration under evaluation changes, failed-layer retention
and deterministic ordering. Synthetic separable and nontrivial numerical fixtures
are not pretrained-detector evaluations. Rust compilation, formatting and tests
have NOT run. The required RCH command was attempted, but rch is unavailable.
No task, production gate or historical execution evidence is promoted by this
source increment. No dependency, runtime, manifest, credential or authority changes.
