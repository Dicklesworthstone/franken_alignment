# Independent accounting at learned startup

The original learned owner can install its evaluator, Andon accounting and
terminal recovery reserve in one first canonical image:

```rust,ignore
let (owner, roles) =
    FileOversight::create_andon_guarded_with_learned_generation_reserved(
        directory,
        profile,
        &guards,
        credential_registration,
        evaluation_protocol,
        &andon_config,
        learned_config,
        recovery_reserve,
    )?;
```

This is the FA-105 composition serving FI-A08, FI-A13 and FI-A18 under plan
§§9.9, 13.4 and 16.4. It uses the existing numerical recipe, native evaluation
and Andon reducers, guarded bootstrap, reserve admission and canonical writer.
It introduces no journal format or dependency.

## Retain joint helper-credibility qualification

Select `create_joint_andon_guarded_with_learned_generation_reserved` to install
an independently supplied `JointPromotionPolicy` alongside the same evaluation
protocol and Andon configuration. Its arguments match the constructor above,
with `joint_policy` immediately after `evaluation_protocol`. The policy is part
of the first acknowledged image; no marginal-only owner or evaluator is exposed
while joint qualification is being installed.

This policy governs the original credibility ledger's candidate helper-weight
promotions. The existing joint replay and marginal qualification rules both
remain applicable. Merely configuring the policy neither qualifies a candidate
weight set nor supplies independent ground truth. The separate recipe-owned
`HeldOutJointPolicy` is a different contract: combining it with this live
evaluation ledger remains invalid, including through the new constructor.

Complete accounted recovery must select the same policy through
`FileAndonRecoveryRequirements::require_joint`. Every policy field, including
replay-work bounds, must match. Omitting the policy requires its absence; it
cannot recover a jointly evaluated history as a marginal one. A changed policy
is not accepted by learning the on-disk value during recovery.

## Preserve the original bootstrap and accounting boundaries

The [mediated accounting composition](MEDIATED_LEARNED_ANDON.md) installs the
same marginal or joint evaluator and Andon contract with the original topology
observer in one reserved first image. Plain constructors continue to require
topology absence; selecting mediation uses its explicit complete role bundle.

Every declared guard and the complete learned recipe are checked before storage
creation. A recipe-owned policy source must equal the independent declaration;
the original learned Enable installs that source exactly once. Required raw or
owned pre-output predictors need their distinct role-preserving constructor and
refuse this evaluator-only lane. The complete first-image event inventory must
leave the selected terminal recovery tail. The final canonical encoder checks
actual bytes with the store identity.

The one canonical replacement must acknowledge before returning the owner and
separate human, evaluator and selected oversight roles. The constructor does
not compute a token, observe time, acquire fresh source evidence, assert an
identity match, label a case or approve publication. Existing directories cannot
be retrofitted or reset through this creation API.

After startup, an independently labeled benign Hold is charged by the original
Andon ledger. Pending, censored and violation outcomes remain distinct. An exact
assessment retry cannot duplicate the charge. Labels do not provide the separate
human key or restart a held effect. Accounting windows and costs remain native
history; this constructor adds no recovery protocol or live endpoint by itself.
The separate [fresh learned service composition](LIVE_ANDON_EVALUATION.md#create-an-evaluated-learned-service)
retains this evaluator for bounded case discovery and assessment after the
original request workflow on the same live owner.

The narrower evaluated-only learned recovery profile rejects histories containing
Andon configuration or registered windows before numerical replay, cleanup or
fencing. Its independent requirements do not pin the accounting identity,
complete windows or evaluation floor; even an exact history anchor cannot supply
that contract. Ordinary evaluated-only histories still use their original
anchored or unanchored recovery. General lower-level learned recovery APIs are
unchanged; this refusal is specific to the evaluated-only wrapper.

The [complete accounted learned recovery cursor](LEARNED_ANDON_RECOVERY.md)
provides the corresponding positive recovery path. It requires the exact retained
history anchor, original numerical recipe, complete independently selected
evaluation/Andon windows and floors, and original guards. Native replay and
accounting stay with one locked owner through its original fence or terminal Stop.

Six public regression functions in
`tests/learned_guarded_recovery/evaluated_bootstrap/andon.rs` cover complete
role retention, exact reserve capacity, malformed input refusal before storage,
recipe-owned source identity, foreign/censored/benign labels and idempotent
costs, and actual publication requiring generation plus both review keys.
They reuse original numerical and durable APIs; fixture judgments are not
empirical detector evidence.

Two further regressions in `andon/recovery_profile.rs` exercise that fail-closed
recovery boundary and paired ordinary evaluated-only recovery. After the
workspace connection recovered, qualified `nightly-2026-09-08` parsing and
formatting checks passed for the scoped sources, as did whitespace checks and
independent source review. The required RCH command could not start because the
runner was unavailable (exit 127); compilation, Clippy and runtime execution
remain unexecuted. Roadmap FA-105 remains in progress.


## Joint bootstrap verification boundary

Five additional public regression definitions in `andon/joint.rs` exercise
original learned publication requiring both keys, independent labeled cases
feeding the native joint report, exact recovery of every joint-policy field,
late false-stop accounting in the original occurrence window, stale role/ticket
refusal, exhausted joint replay work and terminal-reserve admission. The joint
fixture admits an actual weight change, so its work-bound cases reach the
original joint reducer instead of stopping at an unchanged-weight refusal.

Independent source review checked those native contracts and test API bindings.
The execution workspace disconnected, then recovered long enough for the final
sources to pass Rust parsing and scoped formatting checks. The joint test module
passed complete formatting with rustfmt 1.10.0-nightly (2026-08-30). The required
RCH gate still exits before Cargo because its runner is absent. Compilation,
runtime tests and Clippy remain pending. These source cases neither establish
detector quality nor close the FA-105 execution gate.
