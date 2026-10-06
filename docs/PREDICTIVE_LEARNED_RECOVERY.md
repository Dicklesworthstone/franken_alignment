# Guarded recovery of pinned predictive learned owners

Source implementation; compilation and execution remain unqualified. This
connects the existing learned recovery to the complete predictive guard/role
contract (FA-014 and FA-111; plan sections 12.7 and 16). It introduces no reducer,
model, request ledger, journal format, approval issuer or dependency.

## One recovery, one independently supplied contract

`begin_open_predictive_guarded_with_learned_generation` takes both the original
`FilePredictiveRequirements` and independently supplied `FileLearnedConfig`.
The predictor must be pinned inside that recipe with
`with_required_pre_output_forecast`, and equal the exact expected predictor.
The complete recipe binds before any numerical event. A separate prediction
Enable, different numerical engine, undeclared topology, missing or unexpected
evaluation protocol cannot replace the pinned bootstrap. Legacy separately
installed predictors remain under their existing entry points and are not
silently accepted by this one.

`FilePredictiveLearnedRecovery` owns the SAME `FileLearnedRecovery` and exclusive
store lock. Advances run bounded counts of original journal events, including
all numerical/witness checks. Before Ready and again before finish, the original
predictive guard validator checks the entire inventory, effective policy,
credential epoch and independent journal/control/authority floors. A mismatch is
terminal. The candidate and partial roles have no public extraction path.

The anchored begin variant additionally compares the original independently
retained exact history prefix, on the same canonical bytes and lock, before
numerical replay. Floors alone do not distinguish equal-counter forks. Anchors
are sensitive operator data, not authenticated statements or current external
observations. `open_predictive_guarded_with_learned_generation` synchronously
consumes the same cooperative implementation; it does not replay history twice.

## Role custody is not renewed authority

Finish invokes the original exact-cut check, cleanup and single recovery fence.
Only after its canonical acknowledgment does it provision the original
`FilePredictiveRoles`: human reviewer, configured identity observer/policy
governor, consistency observer and any independently registered evaluator.
There is no public role getter, live credential reconstruction or approval-key
restoration. Keep these roles with their separate custodians.

The learned owner remains paused, clock/source/identity eligibility is withdrawn,
and existing unknown effects remain charged. Without an unanswered forecast,
a recovered observer can make a NEW admissible prompt forecast after explicit
resume and any required requalification. It cannot adopt an old observer identity.
An unanswered forecast retains its pending record and permanent coverage-loss
latch. Returned observer custody does not replenish prediction work, erase
likelihood evidence, renew a deadline or allow its unresolved sample to complete.
Dropping partial recovery or failing the final write exposes no owner or roles.

## Authored verification

Six regression functions use the original learned-text fixture and canonical
journal files: positive guarded recovery and resumed forecast/generation/intake;
stale progress, exclusive-lock and abandonment boundaries; mismatched recipes,
predictors, final guard floors and sticky failures; unanswered forecasts with
and without a pending sample; exact anchors versus equal-counter forks; and all
five original fence-write fault barriers. Two compile-fail examples prohibit
inner-owner/role extraction. Existing validators and tests are unchanged.

The required targeted and full RCH commands cannot launch in this environment:
`rch` is absent (exit 127). No local compiler fallback is used. Compilation,
tests, rustfmt and Clippy are unexecuted; source checks do not substitute for
these gates. Synthetic models are controls, not calibration or detector-quality
evidence. No timing guarantee, deployment qualification or bead closure is claimed.
