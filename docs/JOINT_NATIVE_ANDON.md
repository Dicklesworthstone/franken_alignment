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

## Shared operator profile and guarded promotion

The existing `native_andon` and `supervise_publication --andon` consumers share
one profile selector. `fa.native-andon-operator/1` remains marginal-only;
`fa.native-andon-operator/2` REQUIRES a complete `joint` object. The example is
`crates/fa-reference/fixtures/joint_native_andon_operator.json`. It retains all
existing evaluation/window/floor fields and adds:

```json
"joint": {
  "id": 71,
  "generation": 1,
  "maximum_escape_rate": {"numerator": 0, "denominator": 1},
  "maximum_benign_stop_rate": {"numerator": 1, "denominator": 2},
  "budget": {"cases": 512, "member_outcomes": 2048}
}
```

These numbers are example policy inputs, not population guarantees or recommended
thresholds. Native constructors validate every value. Missing/unknown fields,
wrong versions and excessive work ceilings refuse; /1 cannot carry a hidden joint
field. Both ordinary and reserved creation select the original matching coupled
constructor. Existing inspection, assessment, window and live-publication recovery
pin the same complete selection. No recovery discovers or downgrades the policy.

The privileged native operator now exposes:

```sh
cargo run --locked -p fa-reference --example native_andon -- \
  promote CONFIG JOINT_ANDON_PROFILE RECOVERY_PROFILE PROMOTION_JSON
```

This command requires the /2 profile, exact original history/floors and a NEW
independent `retain_history_anchor` destination. It uses the existing bounded
recovery and custody path; no actor receives the evaluator or promotion interface.
`PROMOTION_JSON` is a closed object with schema `fa.native-andon-promotion/1`,
nonzero `operation`, `expected_control_sequence`, `expected_authority_epoch` and
`expected_evaluation_revision`. The control tuple explicitly names the recovered
pre-promotion owner: account for the original recovery fence, not just the saved
pre-recovery epoch. The command never rewrites supplied counters or weights to
make a stale request pass. Neither candidate weights nor a positive report can
be supplied in the document.

For a new operation the original marginal/joint preview must qualify before a
clock is requested. After checked fresh time is acknowledged, the ORIGINAL
promotion recomputes its evidence and candidate again; this is two bounded joint
evaluations, not a single-call cost guarantee. Time observation and promotion
remain separate transactions. Clock refusal, timeout or rejected evidence cannot
promote, though the acknowledged recovery fence (and possibly Time) remains real
history and is retained. An uncertain storage replacement exports no stale anchor.

An already recorded operation goes directly through the original exact-request
retry check: no new clock or reweighting. Each invocation still acknowledges its
own recovery fence. Changed fields under the same operation ID refuse. Successful
output adds only actual promotion generation/revision and joint-qualification
metadata; no private helper/evaluator evidence is rendered. Output or independent
retention failures cannot undo the promotion or trigger automatic retries.

Seven additional authored operator regressions cover exact /1 versus /2 selection
with/without the live reserve, actual source-captured reviews and independent
labels, qualified and common-mode-unsafe candidates, stale requests, missing/slow
clocks, retained output failure, strict documents, exact replay and actual command
routing. Existing sixteen native operator tests and live workflow assertions are
unchanged. Both executable consumers' required tests and unchanged xtask were
attempted through remote-only RCH; the launcher remains absent (exit 127).
Compilation, runtime tests, formatting and Clippy remain unexecuted. Numerical,
checked, multi-client and signed-evaluator compositions are not added here.
