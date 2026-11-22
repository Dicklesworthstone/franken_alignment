# Ordinary learned prompt containment

Ordinary learned text streams can apply checkpoint-bound `NarrowAuthority`
without installing a forecast predictor. This uses the original numerical
checkpoint, containment consequence and journal event. It is scoped implementation
of FA-026/FA-108, plan §11.10 and FI-A02/FI-A09/FI-A16/FI-I03.

## Native contract

`FileGuardedLearnedRecovery::finish_narrowed_stream_checkpoint` consumes the
original independently anchored recovery cursor and returns only
`FileLearnedStreamControlHistory`. It issues no live owner, observer, reviewer,
effect key or recovery Fence. The caller supplies the existing
`FileLearnedStreamNarrowRequest`: exact checkpoint coordinates, journal revision,
control sequence, actor revision, authority epoch, operation, evidence binding
and retained publication targets.

The original guards, effective policy, credential epoch and recovery floors must
hold before and after preparing the native event. The current original numerical
state must still be the complete prompt, in `Generating`, with zero sampling
attempts. The captured actor, cache, sampler, stream, confirmed audience, target
and continuation boundary must match. A stale predecessor or a target outside
the current ceiling refuses. The evidence binding records the independently
selected supervisor's observation; it does not authenticate an incident detector.

Narrowing intersects the target ceiling, withdraws old keys and refunds only
undispatched reservations according to the existing containment law. It leaves
the actor state, numerical work, confirmed messages, charged or Unknown effects,
policy generation and incident counter intact. The resulting consequence
sequence can supply the later incident frontier for a separately selected
`BeginStreamReset`. Only reset completion increments the reset incident count
and performs the original restart audit.

A fresh narrowing appends the existing `StreamNarrow` event. An exact historical
retry returns its original receipt and appends zero events. Changed request data,
an interrupted predecessor, a sampled prompt or a changed confirmed audience
cannot be reinterpreted as the same operation. Ordinary journal capacity funds
the operation; terminal recovery reserve remains unavailable for this purpose.

## Predictor compatibility

An ordinary stream has no action-consistency predictor in its original admitted
generation. It is not a predictive stream with an omitted recovery field.
Complete configuration and guarded replay continue to reject that downgrade.
The existing explicitly selected owned prompt-audit predictor retains its
current-policy and fresh-audit requirements. Raw, optional and legacy predictive
profiles remain outside this narrowing contract.

The event and request encodings are unchanged. No source, policy or authority is
inferred from the absence of a forecast. Checkpoint capture and reset completion
retain their original contracts.

## Operator workflow

The existing Linux `supervise_publication create-learned-generated` command
connects the complete ordinary lifecycle. Select the original ordinary learned
stream recipe and an explicit `functional_restart` configuration. Keep all
identity, credential, checked-publication, joint-credibility, effective-policy
and campaign requirements that the original owner installed.

1. **Prepare the complete unsampled prompt.** Select
   `--prepare-stream-checkpoint CHECKPOINT_PROFILE` and
   `--retain-history-anchor PREPARED_ANCHOR`. The
   [preparation contract](LEARNED_PROMPT_PREPARATION.md) defines the exact
   checkpoint profile. Its expected position is the tokenized prompt length and
   expected sampled draws is zero. No actor request or effect is issued.
2. **Narrow the original authority.** Select
   `--narrow-stream-checkpoint NARROW_REQUEST`,
   `--history-anchor PREPARED_ANCHOR` and
   `--retain-history-anchor NARROWED_ANCHOR`. The closed
   `fa.learned-stream-narrow/1`
   [request schema](LEARNED_PREDICTIVE_STREAM_RESET.md) is shared with the
   predictive profile. Supply the exact checkpoint and current predecessor from
   independently retained observations; the command never infers them from the
   mutable store.
3. **Initiate reset.** Select `--begin-stream-reset RESET_REQUEST`, the
   narrowed anchor as input and a new pending anchor as output. The existing
   `fa.learned-stream-reset-recovery/1`
   [request schema](LEARNED_STREAM_RESET_OPERATOR.md) binds that checkpoint,
   current control/actor/authority predecessor, retained targets and restart
   budget. The narrowing receipt's `first_incident_sequence` is the original
   consequence frontier. Begin acknowledges the exact intent without fencing it.
4. **Complete that intent.** Select `--complete-stream-reset RESET_REQUEST`,
   the pending anchor and a new completed anchor. Use the same independently
   retained request. Completion performs the original numerical restart and
   recovery Fence; a successful reset preserves work, increments the incident
   count and still grants neither publication key.
5. **Continue under current evidence.** Select the normal reviewer profile,
   `--continue-generation` and the completed anchor. The original profile's
   current source and identity checks, native review and a new independent human
   decision remain mandatory.
   A successful reset cannot authorize a later publication on its own.

Every invocation retains the full original configuration and recipe. Each output
must be a new file outside the authority store. The optional offline
`--capture-stream-checkpoint` command uses the same original contract and exact
request coordinates; it cannot generate a missing prompt or rebase a checkpoint.

## Topology-backed custody

Preparation selects `fa.mediated-create/2` with a new source-checkpoint output.
Every subsequent offline operation selects `fa.mediated-submit/2` with the
exact original source checkpoint corresponding to its input history anchor and
a distinct new source output. Both outputs must describe the same acknowledged
cut. The existing [paired profile](LEARNED_MEDIATED_PREDICTIVE_STREAM_RESET.md)
and bounded source contract are shared with the predictive workflow.

The original ordinary `FileMediatedLearnedRecovery` can select
`require_stream_control_checkpoint` at event zero and returns the same sealed
`FileMediatedStreamControlRecovery` used by predictive control. It retains the
actual ordinary cursor and all its independent guards; it does not construct a
predictive owner. Its only successful finalizer returns immutable acknowledged
history and paired source custody, with no live owner or observer.

Offline control checks the original initial documents and retained pair; it
performs no refresh from the next graph or inventory files. It preserves consumed
source checks, reserved bytes, the last source operation, original documents and
monotone recovery floors. Narrowing withdraws the previously accepted cut without
changing source availability or adding a second authority-epoch change. Live
continuation must perform the original source rebind and current-cut
certification; changed or missing source evidence cannot be replaced by the
retained checkpoint.

A lost archive or report cannot undo an acknowledged operation. Retry the
identical request with its original input pair and fresh output paths. The
paired finalizer accepts only the unchanged retained cut or its one matching
selected control event; a different later event cannot be covered by the older
source archive. Reset completion retains its separate original reset/Fence
suffix rules. Keep the full pair from each acknowledged step before proceeding.

## Qualification

Native and actual-command positive and negative regression sources accompany
the implementation, including fresh human approval/rejection and failed current
source checks. Required RCH compilation, formatting, Clippy, runtime tests and
full qualification remain pending. The development workspace became unavailable
during this change; source review is not execution evidence. No claim of
production qualification or general post-sampling narrowing is made.
