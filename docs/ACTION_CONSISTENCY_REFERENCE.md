# Pre-action forecasts and sequential consistency enforcement

## Implemented capability and qualification

This batch connects a pre-action activation forecast, an exact likelihood-ratio
process, and the existing owning OversightBroker. The dispatcher actually
consumes the evidence: missing, outstanding, stale or crossed consistency state
blocks positive review application, authorization and both dispatch paths.
It is not another rights ledger or a disconnected score/report.

The numerical module is `action::consequence::activation::consistency`; its
consumer is `oversight::consistency`. No Cargo dependency, crypto primitive,
runtime, inference host or storage backend was added. This is safe std-only
reference code, not production admission or execution of a real actor model.

Rust compilation, formatting, Clippy and the repository gate have NOT run.
Cargo/rustc/RCH are unavailable in this session. No bead or production packet is
closed, and historical source receipts do not qualify these files. A separate
executed Python arithmetic check is scoped below; it is not Rust execution.

## A forecast fixed before its outcome

`ForecastModel` retains a registered linear probe, calibration domain/generation,
exact-policy generation, nonempty payload-prefix category and three probability
pairs. The existing source-checked decoder and exact linear probe determine the
negative, equality or positive band. A frozen table selects the registered null
and alternative probabilities. The probe margin itself is never mislabelled as
a probability. This is an exact-reconstruction numerical baseline, not learned
predictor training, a native inference adapter or a calibrated detector claim.

`enable_action_consistency` freezes the model, stream, lifetime alpha, prediction
quota and maximum age before any proposal. Its tenant/model-generation checks
use the owning broker's actual scope and supplied actor profile. There is no
setter, disable, re-arm or free replacement error-budget operation.

```text
forecast_action(attempt, expected_actor_revision, supplied_source)
  -> exact activation score -> registered probability pair
  -> one retained forecast, before the action bytes are accepted
propose(attempt, action, snapshot)
  -> validate the forecast/context and bounded action
  -> classify the actual payload and advance lifetime likelihood evidence
  -> existing exact policy, derived witnesses and normal action lifecycle
independent congress -> positive application -> authorization -> dispatch
  -> require current below-threshold consistency evidence as an extra condition
endpoint receipts -> existing reconciliation, regardless of monitor availability
```

The captured position must be the current supplied full-prefix actor state's
last token; stream/profile/dimension identity and sequence are checked. The
actor may advance to generate the proposed action after the forecast. Its
observed actor revision is then retained; later actor-state changes invalidate
positive reuse for that action. Epoch changes invalidate outstanding old
forecasts and prior positive action evidence. Sequences never rewind on reset.

These are API ordering and metadata checks. They do not prove that a host
excluded future action tokens from the supplied activations, disclosed all its
inputs, or served the claimed weights. Those are real capture qualification and
future-leakage experiments required by the original FA-111 packet.

## Do not select the outcomes after seeing them

Only one forecast may be outstanding. It cannot be replaced by a different
attempt, replayed under a fresh sequence, or discarded to restore a quiet state.
While it remains unresolved, even an older approved action cannot dispatch.
Expired forecasts remain outstanding; they are not silently censored, renewed
or assigned a neutral likelihood factor. This is deliberately conservative:
resuming after a lost forecast requires a future governed recovery contract.

A well-formed, matching, current action is classified BEFORE the exact policy,
provider-snapshot and endpoint-admission checks. A subsequently denied action
or missing policy snapshot cannot remove its observed category from the process.
Consequently `propose` may return Err while a new consistency observation was
retained. This changes evidence only, not a failed proposal's rights accounting.
Inspect `consistency_observation(attempt)` rather than blindly retrying that ID.

Malformed, foreign, expired or wrong-epoch proposals are outside this admitted
observation profile and are not counted. Their pending forecast stays unresolved
and prevents selective retry under a new forecast. The calibration population
must cover the actual typed proposal stream, including exact-policy denials and
post-observation admission failures, not just successfully dispatched effects.

## Exact sequential likelihood evidence

Probabilities have denominator 65,536, with strictly positive support for both
categories in both distributions. If the actual category is Y, the factor is
q/p; otherwise it is (1-q)/(1-p). Under the explicit null that p is conditionally
calibrated given the entire prior history, its conditional mean is one. The
alternative must also be fixed before Y. This construction is not the sum of
negative log probabilities, an arbitrary anomaly score, a posterior probability
of intent, or a product of supposedly independent detectors.

The code establishes neither conditional calibration nor the validity of the
registered workload/selection population. Marginal calibration alone does not
suffice. Reset, changing actor behavior, committee feedback and selected attack
workloads must be within the actual calibration/conditional-null contract for
the statistical interpretation to apply. A changed exact-policy generation
refuses further forecasts in this frozen profile rather than silently reusing
its calibration. Calibration promotion and multiple-generation alpha allocation
remain separate, unimplemented governance work.

Products use 129 fixed u64 words each for numerator and denominator, with checked
integer cross-products against alpha. Up to 512 factors below 2^16 require fewer
than 8,192 bits; the 64-bit alpha multiplier fits within the 8,256-bit storage.
No float underflow, rounding, clipping or invented reset-to-one occurs. Equality
at E = 1/alpha crosses. The first crossing stays latched even if the pure numerical
oracle subsequently receives factors that lower E.

The live broker holds its whole scope once crossed. It does not manufacture an
exact disqualifier or label the proposed action harmful. A below-threshold result
is only an additional prerequisite: it does not replace exact policy, complete
helper inputs, empirical approval, an effect permit, a human key or endpoint
fencing. The shared approval check covers `dispatch_with_human` too.

## Lifetime, resource and failure boundaries

Actor checkpoint reset retains the evidence, first crossing, original alpha,
calibration table, sequence floor, observation history and prediction-job count.
Restarting the effect dispatcher also leaves these untouched. Neither operation
claims that in-memory history survives an OS crash or hostile storage rollback.

An explicit `consistency_unavailable` marks a capture gap even when no valid
SourceFrame could be built. A context-valid prediction that exhausts capacity or
fails numerical work similarly marks lost coverage. An ordinary recapture cannot
clear that gap and expose older quiet permission. The finite profile deliberately
has no implicit recovery from this loss or from an unresolved forecast.

Up to 512 predictions and observations are retained, further restricted by the
configured quota. Each job has one fixed-dimension source bounded by MAX_VALUES,
one full source-checked encoding, exact decode and one linear-probe pass. Retained
observations contain probe results and probability/factor/context records, not a
second copy of every activation vector or action payload. These are finite work
and logical retention bounds, not measured latency, peak memory or a compression
saving. The exact predictor intentionally pays the full-reconstruction cost.

Restriction, cancellation, human withdrawal, dispatcher fencing and endpoint
reconciliation do not depend on a quiet process or available capture. Unknown
external effects stay charged until the existing terminal-receipt protocol
resolves them. A genuine nonexecution refund does not remove its earlier observed
proposed category or grant a new statistical error budget.

The older DecisionArchive and ObservedReceipt wire contracts do not export or
verify this entire forecast/history basis. Historical numeric observations and
current exact numerator/denominator words are inspectable in memory. No durable
or authenticated sequential-evidence archive is claimed by these additions.

## Source tests and arithmetic cross-check

Nine numerical unit tests cover support/alpha validation, correct likelihood
factors, neutral alternatives, exact threshold equality, latched crossings,
512-step extreme ratios, independent u128 comparisons, a complete eight-step
Bernoulli null tree and actual source/probe forecast selection. Fourteen public
integration tests cover successful publication, denied/unavailable proposals,
pre-action order, unresolved forecasts, stale/future capture metadata, actor
advancement, reset, capacity, policy changes, human keys and receipt settlement.
These 23 Rust test functions are unexecuted source.

The executed operator-only Python limb port was checked against Fraction on
64 random paths plus extreme 512-step and crossing/reversal cases: 5,123 exact
step comparisons, zero mismatches. A separate enumeration of all 256 eight-step
Bernoulli(1/4) histories gave crossing probability 3365/32768 under alpha=1/4.
That finite calculation checks the specified algorithm/null example, not the
calibration of a real forecast. The script and hash-bound result are retained as
`artifacts/execution/2026-09-10-consistency-arithmetic.{py,json}` and explicitly
state NOT Rust execution. No compiler, detector-quality or performance result
follows from them.

This is scoped reference progress toward FA-111 and plan 12.7/13.2, grounded in
FI-A12 (activation/output mismatch), FI-A05 and FI-A06. Real native capture,
trained/held-out calibrated predictors, masking-adversary campaigns, durable
lifetime budgets and the admitted production integration remain mandatory and
open. It does not satisfy those original obligations by substituting toy labels,
a source-only test or a self-reported confidence score.
