# Joint congress checks for credibility promotion

## Native capability

After `OversightBroker::enable_credibility`, install `enable_joint_credibility(JointPromotionPolicy)` before the first proposal or review. The original `promote_credibility` entry point then requires both its existing independent-label per-helper qualification and a joint comparison of the current congress against the deterministic candidate weights. There is no caller-selected candidate or caller-supplied positive-report admission path. The policy cannot be removed or replaced.

The new check re-tallies the original retained member outcomes using the existing congress reducer, including cohort caps, minimum participation, missing votes and abstentions. It compares empirical Continue versus restriction, not historical execution or exact policy-disqualifier facts. Reconstituted reference transcripts are comparison inputs, not original authenticated helper commitments. Independently supplied labels, protocol stratum and original evaluation cases remain the source of truth.

The report counts violation and benign origins once each, pessimistically retaining any failed replicate. It separately rejects every origin containing an individual case that changes from success to failure, even if a different replicate already failed under the baseline. Thus grouping cannot hide a regression. Candidate joint escape and false-stop ceilings are fixed before observing cases. A permissive ceiling cannot waive a newly introduced regression. Neither rate is derived by multiplying individual helper miss rates.

For example, two helpers can each meet a one-half recall floor, with complementary catches. With current weights 10, a hold-weight maximum of 5 and a Continue minimum of 5, either helper's Hold blocks the case. The existing deterministic weight formula can propose weight 5 for each helper. Re-tallying the same cases then yields permit-weight 5 and hold-weight 5: both formerly held violations become Continue. The joint guard rejects that promotion. A paired configuration with hold-weight maximum zero still permits the reweighting because the original reducer continues to block both violations.

## Costs and preservation

`JointReplayBudget` limits complete case admission and member-outcome visits for two reductions per case. The entire selected inventory is admitted before reduction. Budget failure, marginal qualification failure, pending/censored labels and joint failure all occur before weight, authority-epoch or cancellation mutation. Successful promotions retain the exact joint report alongside the existing native promotion history. `joint_credibility_report` previews the same candidate and owned evidence, while `joint_credibility_promotions` exposes the retained successful bases. Neither is a permit.

Legacy profiles without the new policy keep their original semantics, including old replay behavior. They do not acquire a joint-validation claim. No external dependency, runtime, effect adapter or replacement accounting ledger was added. This protects the observed calibration cases; it does not constitute held-out evaluation, a population risk bound, statistical independence, helper authentication, adaptive-attack resistance or proof that a real effect was stopped.

## Verification status

Twelve new source tests cover equal marginals with different joint outcomes, per-replicate regressions, missing/abstaining/censored/pending cases, complete budget boundaries, forbidden candidate changes, origin and policy separation, and real native proposal/congress/independent-label/promotion flows. One compile-fail example checks report-to-permit separation. They have not been compiled or executed in this implementation environment. The native addition is pending the repository's revision-bound RCH verifier; no Beads item or production gate is closed.

Required verification: `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`.
