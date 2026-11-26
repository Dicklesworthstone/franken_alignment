# Recover learned state with its original evaluation and Andon history

An accounted learned owner has three inseparable recovery obligations: its
original numerical reconstruction, the independent evaluation history, and the
complete native Andon contract. The accounted recovery cursor carries all three
through the same exclusive canonical cut and original replay engine.

This is the recovery composition for the
[atomic learned accounting bootstrap](LEARNED_ANDON_BOOTSTRAP.md), serving
FA-026/105 and plan sections 9.9, 11.2, 16.4 and 16.9. It preserves the L4/L5
authority/effect boundary and L8 accounting history under FI-A08/A13/A18. No
new journal format, imported accounting snapshot or runtime dependency is added.

## Supply the complete independent contract

The caller retains `FileAndonRecoveryRequirements`, the original
`EvaluationProtocol`, the exact `FileLearnedConfig` and a `FileHistoryAnchor`
outside the journal being recovered. The requirements bind:

| Independent expectation | Recovery obligation |
| --- | --- |
| Complete base guard inventory and current policy | A recovered owner cannot silently omit a source, identity, credential or other required guard. |
| Original Andon configuration | The deployment/evaluator identity and initial windows must match. |
| Complete current window inventory | Later registrations remain required; an old window or incident cannot disappear at restart. |
| Minimum evaluation revision | An older label prefix cannot satisfy a newer independently retained floor. |
| Original numerical recipe | Replay retains the declared model, cache, sampling and work contracts. |
| Exact retained history anchor | A valid equal-counter fork or older canonical prefix cannot impersonate the selected history. |
| Publication witness requirements, when installed | The checked publication contract remains required, including validation installed by the original learned recipe. |
| Joint evaluation policy, when installed | The existing independent joint-policy contract must match; it cannot be inferred from a retained on-disk enable. |

`None` in an independent guard or publication selection means required absence.
It never means to accept whatever happens to be present in the journal. The
requirements contain expectations; native reducers reconstruct all cases,
labels, counts, costs, supports and numerical state from the original events.

## Use the original cooperative cursor

An owner with topology enforcement uses the separate
[mediated accounted learned cursor](MEDIATED_LEARNED_ANDON.md). It adds exact
original/current graph and availability requirements while retaining the same
accounting and numerical contracts. Plain recovery still requires topology
absence. The source checkpoint remains independently bound to the same history;
graph equality alone cannot replace that custody.

```rust,ignore
let mut run =
    FileOversight::begin_open_andon_guarded_anchored_with_learned_generation(
        directory,
        profile,
        &accounting_requirements,
        &evaluation_protocol,
        &learned_config,
        &history_anchor,
    )?
    .require_recovery_reserve(RecoveryReserve::terminal())?;

while run.progress().status == FileLearnedRecoveryStatus::Replaying {
    // The host checks its original deadline and independent Stop here.
    let completed = run.progress().replayed_events;
    run.advance(completed, 1)?;
}

// The Ready checkpoint remains vetoable before the original finalizer.
let (owner, roles) = run.finish()?;
```

The source is `guarded/evaluation/andon/learned.rs`. The sealed
`FileAndonLearnedRecovery` retains the original `FileLearnedRecovery`; it does
not expose a candidate machine, raw cursor or evaluator during replay. An
exact predecessor and finite event count bound each advance. One numerical
event, initial file admission and final synchronization can still be synchronous;
this is not an interruptible-kernel latency guarantee.

Static recipe, history, protocol, accounting and inventory mismatches refuse
before numerical replay. A recipe-owned publication gate counts as its original
activation even though it has no separate top-level witness-enable event; the
independent requirements must still select exactly that contract. Complete guard
and counter-floor checks precede Ready
and each finalizer. A failed full check remains a failure; the caller cannot
repair the private candidate by swapping expectations. Dropping the cursor,
including after a host checkpoint refuses, leaves the canonical image and
pending staging bytes unchanged.

Successful normal finalization uses the original canonical comparison,
capacity/reserve admission and single acknowledged recovery fence. Only then
does it return a fresh human/oversight bundle and a separately held independent
evaluator. Old roles and approval keys remain unusable. The numerical owner is
paused; recovery supplies no fresh clock, source evidence, identity observation,
helper judgment or human approval.

## Preserve accounting and terminal control

Pending and censored cases, independent labels, lifetime false stops, complete
windows, full-width costs and incident support survive recovery. A subsequent
explicit assessment uses the recovered owner's newly issued evaluator and a
ticket obtained from that owner. Restoring numerical state does not forgive an
unknown effect, replenish numerical work or erase an incident.

At the fully verified Ready cut, the host may select the existing authenticated
recovery Stop path instead of ordinary live finalization. That path issues no
human or evaluator role and applies the original clock-independent admission
Stop/Fence. Its original terminal reserve and native receipts remain authoritative;
acknowledging admission Stop is distinct from draining unresolved effects.

An independently selected, already recorded pending reset can instead use the
original pending-reset finalizer. Numerical and stream reset forms remain
distinct, including the stream's original incident declaration. The full
accounting/guard contract is checked before and after native reset preparation;
only the original final replacement acknowledges reset completion and fencing.
Neither labels nor Andon history are replaced by the pre-incident numerical state.

The narrower evaluated-only learned wrapper continues to refuse Andon history.
Accounted recovery requires this complete independent contract. Required raw
or owned predictors need their own complete observer-preserving composition and
remain outside this role bundle.

## Continue through the live operator

The paired `--andon` / `--evaluator-profile` learned command composes this cursor
with explicit `--continue-generation`, `--after-message PREVIOUS_REQUEST` and
`--finish-after-message PREVIOUS_REQUEST`. It requires both an independently
retained input anchor and a fresh output, uses the original per-event/Ready
deadline and authenticated Stop handling, and returns to the original source,
helper and human approval path after successful fencing. Independent evaluation
then services new and pending historical cases under the remaining finite
invocation allowance. See
[live accounted learned continuation](LIVE_ANDON_EVALUATION.md#continue-an-evaluated-learned-service)
for the complete grammar and its bounded supported composition.

The same command accepts the explicit version-two Andon profile's
`JointPromotionPolicy`. It pins every original policy field through
`require_joint`, in addition to the complete accounting windows and evaluation
floor. Version-one absence cannot recover a joint history. This applies to
single continuations, evaluation-only history and resident Message/Finish
suffixes; none substitutes the separate recipe-owned held-out publication policy.

## Verification boundary

Eight public regression functions use the existing original numerical, native review and
independent assessment fixtures. They pair successful accounted reconstruction
and fresh role use with incorrect history/configuration/floor and terminal-control
refusals. These are authored regression sources; they do not establish empirical
detector quality, cross-hardware numerical identity or production qualification.

Required remote compilation, runtime tests, Clippy and the canonical gate remain
pending while RCH is unavailable. Exact scoped source checks and attempt results
are recorded with the implementation; source review is not execution evidence.
