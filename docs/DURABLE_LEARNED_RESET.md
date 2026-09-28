# Durable original learned checkpoints and containment

The existing `FileOversight` journal now records captures of the broker's
original paired learned KV/sampler and authority checkpoints. This serves the
rewind-as-containment work in plan section 11.10 and FA-108. It adds no numerical
engine, authority ledger, model importer or effect-approval bypass.

## Original capture, exact replay, branded handles

`capture_learned_checkpoint` requires the live original quiet, nonempty, active
generator, its expected actor revision and authority epoch, a fresh clock, and
no unresolved token intent. The broker retains its original restart-grade,
stream-owner, state-equality and storage limits. Capture is persisted through
the original candidate/journal replacement boundary before returning a handle.

The record binds exact original tokens, KV words, logits, samples, RNG encoding,
actor copy, work and audit state. The additional checkpoint witness binds all
remaining continuation ceilings, cumulative abandoned-work counters, original
recovery costs, incident count and authority sequence/epoch. Saved values are
comparison material only. Replay executes the original generation and capture;
it never loads archive tensors or a claimed quiet result as live state.

`learned_checkpoint` reacquires historical identity from an acknowledged owner.
`check_learned_checkpoint` checks the owner brand and original capture, not effect
eligibility. Exact capture retries return the old cut without a new write.
Foreign and pre-recovery handles do not belong to the recovered owner, even
when their numeric IDs and metadata match. Inventory and logical state bounds
remain enforced. A checkpoint is never a permit.

The original configuration binding, recovery fence, explicit numerical resume,
sidecar requirement and both effect keys are unchanged. The original token
witness layout is retained; checkpoint events have a distinct new tag. Reopening
continues to require the independently supplied exact `FileLearnedConfig`.

## Verification boundary

Seven authored regression functions cover original capture/reconstruction,
historical retries, foreign/recovered brands, pending-token and pause barriers,
corrupt comparison bytes in otherwise valid journal framing, and the exact
checkpoint-count boundary, original two-key publication, and failures at all
five canonical storage barriers. The existing numerical tests and fixtures are reused
without changing their assertions. These synthetic controls do not qualify a
trained detector or registered restart grade.

Compilation, Rust tests, rustfmt and Clippy are UNEXECUTED: the required remote
RCH launcher is unavailable in this preparation environment. Source comparisons
are not a substitute for a successful remote build. This first increment stores
checkpoint pairs; durable reset execution is the next part of this path.
