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

## Qualification

Native positive and negative regression sources accompany the implementation.
Required RCH compilation, formatting, Clippy, runtime tests and full qualification
remain pending. The development workspace became unavailable during this change;
source review is not execution evidence. Operator and paired-topology integration
are tracked separately from this native increment.
