# Data-driven activation probe fitting

## Consumer and implementation status

`activation::probe::training` fits coefficients consumed by the existing `LinearProbe` and `RefinementMonitor`. This is numerical/evaluation work for the baseline and activation-signature requirements in plan 10.5, 10.7, 12.6 and 13.1 (FA-028/FA-109/FA-110), serving FI-A05/FI-A10 and the decision-sensitive introspection requirement. It adds no alternative probe evaluator, production authority, dependency, executor or foreign code.

First increment: frozen labelled-capture corpus, deterministic class-balanced standardized logistic fitting, original-probe export and eleven public regression tests. Rust compilation/tests, rustfmt, Clippy and the revision-bound RCH gate are pending. No Beads packet is closed; source implementation is not detector qualification. Historical execution receipts do not qualify new source.

## Freeze the evidence population

`ProbeCorpus` requires an entire origin-to-split assignment before accepting any labelled capture. Every task ID and every attack-lineage ID occurs exactly once, including within one split. This deliberately narrow baseline takes one selected frame of one registered tap per original case, not arbitrary derivative frames counted as independent samples. Training, calibration and evaluation are all represented. A same capture identity cannot be registered under another origin.

The builder accepts immutable `SourceFrame` observations matching the exact model/tap/layout profile and width. Labels cannot be rewritten and captured cases cannot be removed or moved. Sealing requires every planned observation and both benign/violation labels in each split. Missing or censored cases therefore remain a failure to seal, not a reduced denominator. Later observations can complete an unfinished builder. Sealed instances share immutable scalar arrays; dropping the source owner does not erase retained data.

Origins and labels are supplied by the evaluation owner. This cannot authenticate task independence, labels, producer identity or a claimed attack lineage. It does not prevent the caller from creating another campaign, falsifying origin IDs, inspecting held-out data outside the API, or selecting the corpus after seeing results. Such governance requires independently owned preregistration and evaluation. The structural API is not that authority.

The bounds are 4,096 cases, 65,536 dimensions and 1,048,576 retained coordinates in aggregate. All planned dimensions/counts are checked before capture retention. A model-sized activation corpus is not silently accepted beyond these reference bounds. Source allocation and metadata coexist; logical counts are not allocator/RSS measurements.

## Real coefficient fitting

`SealedCorpus::fit` runs fixed-epoch full-batch logistic regression. Mean and population variance use Welford accumulation on TRAINING coordinates only. The scale is the larger of the computed standard deviation and the explicit positive scale floor; a constant coordinate has no learned signal. Zero-initialized coefficients update in deterministic origin/coordinate order. Each class contributes half of the data objective, avoiding a trivial majority-only training objective. L2 applies to weights, not the intercept. Learning rate, regularization, epoch count and floor are explicit immutable policy values.

The optimizer is rounded binary64 with a numerically stable sigmoid, checked finite intermediate results and a maximum 4,096 epochs. There is no convergence or calibrated-probability claim. Fitting never reads calibration or evaluation values. Complete source-coordinate work is admitted before fitting, with no per-epoch replenishment. Reports separate source-coordinate visits, parameter updates and sigmoid evaluations; they are not total instructions, FLOPs, wall-clock cancellation or conserved production resources. Failed fits may consume computation before refusing.

The final raw-space coefficients are finite binary32. Bias recentering uses the emitted rounded coefficients, not unrounded optimizer weights. `FittedProbe::probe(threshold)` constructs the ORIGINAL exact-arithmetic LinearProbe over those raw coefficients. Threshold selection remains explicit; export alone is not empirical qualification. The fit retains its corpus, exact policy, normalization and successful work. Numerical overflow returns no partially fitted probe, and no fitted object contains a permit or mutable live controller.

## Regression source

The eleven tests cover actual learning with a neutral constant coordinate, reversed training labels, held-out-value changes without training leakage, origin/task duplication, incomplete populations and late arrivals, frame-identity reuse, incompatible profiles/widths, exact aggregate-coordinate bounds, both-class denominators, full-fit budget admission and retained owner lifetimes. Synthetic examples test the algorithm and type boundaries, not a trained model's harmfulness detector. Exact threshold tests compare the emitted bias to itself rather than presuming rounded optimization preserves a mathematically zero intercept.
