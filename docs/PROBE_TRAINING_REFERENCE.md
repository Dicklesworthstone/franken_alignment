# Data-driven activation probe fitting

## Consumer and implementation status

`activation::probe::training` fits coefficients consumed by the existing `LinearProbe` and `RefinementMonitor`. This is numerical/evaluation work for the baseline and activation-signature requirements in plan 10.5, 10.7, 12.6 and 13.1 (FA-028/FA-109/FA-110), serving FI-A05/FI-A10 and the decision-sensitive introspection requirement. It adds no alternative probe evaluator, production authority, dependency, executor or foreign code.

First increment: frozen labelled-capture corpus, deterministic class-balanced standardized logistic fitting, original-probe export and eleven public regression tests. Second increment: exact-score threshold selection, frozen final evaluation criteria, retained failed campaigns, and ten calibration/decoder/control tests. All 21 new Rust test functions remain uncompiled/unexecuted. Rustfmt, Clippy and the revision-bound RCH gate are pending. No Beads packet is closed; source implementation is not detector qualification. Historical execution receipts do not qualify new source.

## Freeze the evidence population

`ProbeCorpus` requires an entire origin-to-split assignment before accepting any labelled capture. Every task ID and every attack-lineage ID occurs exactly once, including within one split. This deliberately narrow baseline takes one selected frame of one registered tap per original case, not arbitrary derivative frames counted as independent samples. Training, calibration and evaluation are all represented. A same capture identity cannot be registered under another origin.

The builder accepts immutable `SourceFrame` observations matching the exact model/tap/layout profile and width. Labels cannot be rewritten and captured cases cannot be removed or moved. Sealing requires every planned observation and both benign/violation labels in each split. Missing or censored cases therefore remain a failure to seal, not a reduced denominator. Later observations can complete an unfinished builder. Sealed instances share immutable scalar arrays; dropping the source owner does not erase retained data.

Origins and labels are supplied by the evaluation owner. This cannot authenticate task independence, labels, producer identity or a claimed attack lineage. It does not prevent the caller from creating another campaign, falsifying origin IDs, inspecting held-out data outside the API, or selecting the corpus after seeing results. Such governance requires independently owned preregistration and evaluation. The structural API is not that authority.

The bounds are 4,096 cases, 65,536 dimensions and 1,048,576 retained coordinates in aggregate. All planned dimensions/counts are checked before capture retention. A model-sized activation corpus is not silently accepted beyond these reference bounds. Source allocation and metadata coexist; logical counts are not allocator/RSS measurements.

## Real coefficient fitting

`SealedCorpus::fit` runs fixed-epoch full-batch logistic regression. Mean and population variance use Welford accumulation on TRAINING coordinates only. The scale is the larger of the computed standard deviation and the explicit positive scale floor; a constant coordinate has no learned signal. Zero-initialized coefficients update in deterministic origin/coordinate order. Each class contributes half of the data objective, avoiding a trivial majority-only training objective. L2 applies to weights, not the intercept. Learning rate, regularization, epoch count and floor are explicit immutable policy values.

The optimizer is rounded binary64 with a numerically stable sigmoid, checked finite intermediate results and a maximum 4,096 epochs. There is no convergence or calibrated-probability claim. Fitting never reads calibration or evaluation values. Complete source-coordinate work is admitted before fitting, with no per-epoch replenishment. Reports separate source-coordinate visits, weight-coordinate updates (the `parameter_updates` field excludes intercept updates) and sigmoid evaluations; they are not total instructions, FLOPs, wall-clock cancellation or conserved production resources. Failed fits may consume computation before refusing.

The final raw-space coefficients are finite binary32. Bias recentering uses the emitted rounded coefficients, not unrounded optimizer weights. `FittedProbe::probe(threshold)` constructs the ORIGINAL exact-arithmetic LinearProbe over those raw coefficients. This remains an explicit unqualified candidate export. The fit retains its corpus, exact policy, normalization and successful work. Numerical overflow returns no partially fitted probe, and no fitted object contains a permit or mutable live controller.

## Calibration without a second numerical evaluator

`CalibrationPolicy` fixes a strictly increasing finite binary32 threshold grid (at most 256), calibration criteria and final evaluation criteria. The criteria require a positive count of violation alarms and bound benign holds. They are finite-sample operating points, NOT confidence intervals, probability calibration, statistical independence or population-level recall guarantees. Requiring at least one true alarm excludes the trivial always-quiet detector from this selection path.

Calibration reads every assigned calibration capture, fully reconstructs it through the existing source-checked codec, and runs the original exact LinearProbe once at threshold zero. Each candidate threshold is represented by the original exact dyadic accumulator and compared with those retained scores. No floating sum or optimizer score substitutes for the emitted binary32 detector. One exact score can support many threshold comparisons without repeating the activation decode or dot product. Training and evaluation rows are not scored by calibration.

Each case is retained with origin, label, capture identity and exact score. Each threshold records all six benign/violation by alarm/quiet/boundary counts. Equality is never quiet: it contributes to benign holds but not to a demonstrated violation alarm. Every denominator includes boundary cases. A campaign with no eligible threshold remains a complete report with no selected candidate, not a fallback detector or discarded run.

Selection maximizes violation alarms, then minimizes benign holds, then takes the first (lowest) declared threshold. No evaluation score participates. Calibration and evaluation each preflight their full encoded-byte, scored-coordinate and threshold-comparison budgets. The reported scored-coordinate count is dimensional work, not the two accumulator products or a FLOP count. The source codec's actual encoded lengths must match the predicted complete-frame byte count.

## Final evaluation and the existing monitored decoder

`CalibrationRun::evaluate` consumes the ENTIRE frozen evaluation split with the selected threshold and already recorded evaluation rule. It accepts no row subset, alternate threshold or replacement fit. The immutable report retains successful and failed outcomes. A failing final evaluation cannot export a probe through `EvaluationReport::probe`; it does not retune the threshold, change coefficients, erase failures or declare samples absent.

A passing report exports an ordinary LinearProbe with the actual fitted coefficients and calibrated threshold. The original low-level candidate constructor remains available for research, so this API is not a mandatory production governance layer. Promotion, authentication, model/probe identity registries and policy qualification remain separate. A finite passing corpus does not establish safety or exclude adaptive overfitting across multiple campaigns.

The decoder integration test obtains labelled SourceFrames from actual forward-computed residuals, fits them, selects a threshold, evaluates the untouched split and installs the resulting probe in the ORIGINAL RefinementMonitor/MonitoredDecoder. A benign synthetic case releases; the opposite residual triggers an alarm, makes prior live evidence unavailable and prevents subsequent numerical advance. The paired control-gate test lets the quiet case publish through the original congress/permit/endpoint while a later learned alarm blocks an already-issued permit without refunding its reservation. No trained-host verdict or new ledger is fabricated. Synthetic weights and labels are algorithm controls, not evidence of real harmfulness detection.

## Regression source and execution limits

The eleven fitting tests cover learning with a neutral constant coordinate, reversed training labels, held-out-value changes without training leakage, origin/task duplication, incomplete populations and late arrivals, frame-identity reuse, incompatible profiles/widths, exact aggregate-coordinate bounds, both-class denominators, full-fit budget admission and retained owner lifetimes. Exact threshold tests compare the emitted bias to itself rather than presuming rounded optimization preserves a mathematically zero intercept.

Eight calibration tests cover a usable threshold and exported exact probe, changed final data with identical fit/calibration but explicit evaluation failure, equality holds, no eligible candidates, exact/one-under whole-campaign budgets, fixed stricter evaluation rules, deterministic threshold ties, invalid policies and optimizer overflow with intact source data. Two numerical/control integration tests exercise the connected path described above. There are no modified historical assertions, runtime substitutions or new dependencies.

The required `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test probe_training --test probe_calibration --test trained_decoder_monitor` command cannot start in this editing environment because rch is absent. Rust execution, formatting, Clippy and independent revision-bound qualification remain pending. Learned nonlinear probes, dataset ingestion/elicitation, authenticated independent labels and lineage, statistically powered held-out attack evaluation, and trained-model deployment qualification are not supplied by this baseline.
