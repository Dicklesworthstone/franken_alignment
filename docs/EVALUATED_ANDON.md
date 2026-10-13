# Evaluated false-stop accounting

Source implementation, 2026-10-13. Plan section 9.9; FI-A08, FI-A13 and FI-A18; scoped advancement of franken_alignment-credibility-ledger-dc0.4. No packet or qualification gate is closed.

`action::consequence::oversight::andon` connects the original sealed `congress::credibility::CredibilitySnapshot`, original `round::review::CongressReview` and original `AndonLedger`. A minority hold is not a stopped action; a stopped action is not necessarily a false hold. Both the actual empirical decision and independently supplied ground truth must match.

## Exact join

Bootstrap retains an `EvaluationBinding` independently: deployment/evaluator accounting identity, the complete campaign/model/evaluator/held-out-manifest scope, label owner, and complete helper generation/cohort inventory. `EvaluatedFalseHold::from_review` checks that binding against the sealed snapshot. The selected case and actual review must have the same nonzero ID and identical 32-byte evidence-root bytes. This is a deliberately explicit shared-root profile, not a new hash of an arbitrary full challenge.

The actual review must be complete, exact-clear and held by its empirical policy. Every helper observation must match its actual revealed verdict, including helpers that allowed the action. Every attributed hold must precede the declared counterfactual dispatch position strictly. Only an independent final Safe label can produce a receipt. Pending, censored, missing, abstaining, exact-disqualified, ready, mismatched and too-late inputs refuse.

The receipt derives report ID from the campaign, occurrence from the dispatch sequence, confirmation from the actual label-recording sequence (not sealing time), and holder generations from the frozen campaign. Andon windows must use that same declared control-sequence domain. Delayed labels charge the original occurrence window. Retain the campaign, label and review independently for later challenge; the compact accounting record does not replace them.

## Import

`receipt.record(&mut ledger)` uses the original transition. `record_evaluated_batch` checks a bounded group on one private candidate, then publishes the in-memory state only if every transition succeeds. Identical repeated rounds remain free. A conflicting later record leaves the original ledger unchanged. The read-only `confirmation()` accessor feeds existing durable trusted-input adapters; persistence must precede acknowledgment.

No review requirement, weight, roster, permit or effect right is modified. The existing trusted entry points remain explicitly trusted, not magically authenticated by this adapter. Caller-supplied campaign truth, source authenticity, helper identity and topology remain trust assumptions; there is no signature verification, institutional-independence proof, population calibration or live process-driver integration here.

## Verification status

Nine new unit regressions and one compile-fail example accompany the source. Positive cases pair actual stopped reviews with independent safe labels; negative cases change labels, scope, non-holder generations, evidence roots, actual decisions, timing and later batch entries. The original module-registration blob was matched exactly before editing. `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::oversight::andon` exited 127 because rch is absent. Compilation, rustfmt, Clippy and Rust test execution remain pending; source inspection is not qualification.
