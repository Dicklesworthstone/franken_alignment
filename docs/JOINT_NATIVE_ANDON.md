# Joint promotion checks with native Andon accounting

Plan section 9.9; FI-A07/FI-A08/FI-A13/FI-A18. This integrates the original joint
congress replay with the native evaluation/accounting journal. No execution gate
or credibility-ledger packet is closed by this source change.

## One native evaluation owner

`FileOversight::create_joint_andon_guarded` publishes the original guards,
`EnableJoint` and `EnableAndon` together in its first canonical image. The explicit
`create_joint_andon_guarded_reserved` variant uses the existing reserve-first
bootstrap required by live publication. There is no intermediate marginal-only
owner, extra evaluator, new event encoding or alternate weight rule.

Construct the existing `FileAndonRecoveryRequirements`, then explicitly select
`require_joint(policy)`. The policy's ID, generation, escape ceiling, benign-stop
ceiling and both work limits are independently retained requirements, not values
learned from disk. The default remains marginal-only and requires absence of the
joint policy. Neither profile can silently recover the other. Mixed or duplicate
evaluation activations refuse before historical event execution. Final native
validation checks the exact selected policy again.

The same `begin_open_andon_guarded`, `open_andon_guarded`, `read_andon_guarded` and
`FileAndonRecovery::finish_reconciled` now carry this explicit requirement. They
retain the original anchor, window inventory, guards, policy/floors, event budget,
Store lock, canonical comparison and acknowledgment boundaries. Readiness is not
an owner; failure returns no role. Recovery still reconstructs labels, receipts
and incidents in original acceptance order and withdraws old approvals/role brands.

## Actual promotion, not a saved positive report

Call the existing `joint_credibility_report(evaluation_revision)` to preview the
original deterministic candidate. The existing `promote_credibility` remains the
only weight-changing operation. It recomputes both original marginal qualification
and joint replay over all owned current-policy cases. A candidate that introduces
a new per-origin miss or false stop refuses even when its aggregate ceilings pass.
An exhausted joint work budget is a refusal, not a marginal-only fallback.

A permitted promotion retains the native Andon ledger and full receipt provenance.
Andon budget exhaustion remains a calibration incident and cannot lower a review
threshold. The original native evaluation protocol's separate false-stop-budget
qualification still applies unchanged. These two scoped counts are not conflated.
Use `joint_credibility_promotion(operation)` to retrieve the original joint report
for an acknowledged promotion; exact operation retries do not reweight again.

## Verification and boundaries

Seven authored regressions use actual native review/evaluator/file-owner APIs.
A paired two-helper case admits a candidate under one unchanged hold threshold
and rejects it under another where the same weights would introduce an escape.
Tests also cover retained false-stop incidents and full receipts, post-restart
censor resolution with fresh roles/time, every joint contract field, marginal and
mixed activation refusal, bounded reconciled recovery, original bootstrap and
promotion storage barriers, and joint-work exhaustion. Existing tests are unchanged.

This worktree contains selected upstream source and additions, not a complete
workspace checkout. Required `RCH_REQUIRE_REMOTE=1 rch exec` targeted tests and
unchanged xtask were attempted; the launcher is absent (exit 127 before Cargo).
Compilation, Rust tests, rustfmt and Clippy remain unexecuted. Source/hash/patch
checks do not establish runtime correctness, population calibration, evaluator
identity or institutional independence. No dependency or authority rule changes.
