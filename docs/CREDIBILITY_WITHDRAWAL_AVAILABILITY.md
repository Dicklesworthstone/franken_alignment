# Credibility loss during unavailable observations

FA-105 continuation; plan sections 9.9, 9.11 and 14.9. This extends the existing
held-out FileOversight path rather than adding another qualification mechanism.

A held-out withdrawal is a restrictive transition. It must remain recordable
when inference is paused or the file-source reader was interrupted. The same
source-admission and replay-admission owners now distinguish withdrawal from
activation and ordinary work. Withdrawal does not acknowledge a source capture,
resume the decoder, observe a new clock, or restore either publication key.
Activation, publication and ordinary proposals retain their existing guards.

The original broker still validates operation identity and predecessor sequence
and epoch. The original canonical replacement still owns acknowledgment. An
unknown dispatch remains charged until the original endpoint supplies settlement
evidence; recording loss is not proof of nonexecution. Healthy historical retries
return the original receipt without appending or repairing unavailable inputs.

The added source-interruption tests cover live and recovered clocks, successful
canonical replay, unchanged capture latches, stale/invalid requests, blocked
reactivation and publication, historical retries, and receipt-only reconciliation
of unknown charges. They use the existing private interruption latch and original
broker/publication fixtures, not a replacement reducer or endpoint.

Validation status: the required RCH invocation exited 127 because rch is absent.
Rust tests, compilation, Clippy and rustfmt are UNEXECUTED. This is a source
implementation and regression corpus, not FA-105 closure or a production claim.
