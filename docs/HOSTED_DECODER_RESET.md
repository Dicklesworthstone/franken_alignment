# Paired numerical checkpoint and containment reset

Consumer: the original OversightBroker owning a MonitoredSampledDecoder. This
implements the missing coordinated numerical-reset path identified in
HOSTED_DECODER_REFERENCE.md (FA-025/027/108). The existing standalone monitored
owner still has no public reset, mutable state or raw-checkpoint escape.

## Capture and preparation

capture_hosted_checkpoint requires a live quiet nonempty prefix. It compares the
actual original tokens, encoded complete-layer KV state and 96-byte sampler with
the controller's actor copy before pairing the original branded ledger checkpoint
with private immutable numerical data. Handles expose only an identifier, not
cache/RNG bytes or rights. Both the ledger's original state-retention bounds and
an additional 32-checkpoint/8-MiB logical numerical-state bound apply; this is not
an allocator/RSS bound. Weights remain shared immutable parameters.

reset_hosted_decoder checks the exact owner, actor revision, control sequence,
authority epoch, reset binding and numerical allowance. It stages a new stream by
re-executing every ORIGINAL token with the SAME immutable parameters and monitor
roster, under the CURRENT remaining monitoring allowance. Every replayed token
must obtain a fresh complete quiet review. Complete logits, all layers' key/value
bits and the saved sampler state must match before the candidate is usable.
There is no lossy import, threshold change, reseeding, or saved quiet approval.

The old source becomes unavailable before replay starts and remains unavailable
on admitted computation/review/comparison failure. Monitoring charges persist on
both normal failure and unwind. A bounded lifetime replay-attempt/product
allowance is charged before each admitted attempt, including one whose original
controller transition ultimately refuses. Completed replay computation is also
reported separately from admission and never erases abandoned numerical work.

## Original authority remains authoritative

Only after replay succeeds does the ORIGINAL containment reset advance the
revocation floor, intersect the target ceiling, count the incident, and cancel
undispatched work. It refunds only original undispatched reservations. Sent or
unknown effects remain charged and reconcile through the same endpoint receipts.
If the existing incident policy suspends instead of restoring, the prepared
numerical owner is discarded and cannot reopen the controller.

For a restoring transition, the original reset's actor revision is followed by
one original host-state update for the new source stream. HostedResetReceipt
preserves the original ResetReceipt and separately reports the final synchronized
actor revision. No callback or external I/O occurs across those transitions.
The gate then installs the internally prepared source without replacing its
historical records or cumulative retention costs. Old observation handles close;
old evidence, permits, helper approvals and human keys cannot become current.
New work requires a new proposal and review under the advanced epoch.

The sampler's logical position and exact random state intentionally rewind for
continuation replay; numerical work and monitoring budgets do not. The source
stream advances. Original elapsed clocks, policy history, effect obligations,
incident counts and stop latches remain outside the checkpoint. Terminal stop
cannot be undone. Already returned envelopes still require the existing endpoint
fence/stop protocol; reset alone is not remote nonexecution evidence.

## Reference scope and verification

This is full-prefix CPU recomputation, not a constant-time cache installation or
production restart-speed claim. The authority process and paired handles must
survive; durable whole-process recovery, hostile-process containment and actual
trained-probe accuracy are not established. The caller remains the trusted
supervisor permitted to invoke the existing reset contract.

Seven integration scenarios and one compile-fail example cover exact stochastic
continuation/full logits, learned holds, original permit revocation, both key
profiles and unknown effects, stale/foreign requests, monitoring exhaustion,
incident escalation, bounded capture and terminal stopping. The numerical
fixtures and helper verdicts are synthetic. Rust compilation, formatting and
tests have NOT run in this editing environment; RCH is unavailable. No dependency,
original test expectation, historical execution receipt, or bead status changes.
