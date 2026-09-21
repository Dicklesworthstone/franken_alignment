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

## Interrupted withdrawal recording

Accepted held-out loss now holds an exclusive, scope-bound recording guard until
its original receipt is ready. Returned errors and Rust unwinding both close the
live owner. Caller/predecessor validation and exact historical retries happen
before the guard is armed. The guard neither writes a journal nor changes a
balance; it cannot refund an unknown effect or mint an approval.

A storage fault retains its original operation and visibility diagnostics. A
panic after a completed canonical transaction reports that replacement may be
visible, even when the caller never received its withdrawal receipt. Reopening
still replays the actual bytes and requires explicit qualification reactivation.
This is unwind safety, not a claim about panic-abort or physical power cuts.

Two additional regression tests exercise an acknowledged control versus unwinds
before/after the real canonical transaction, and unwinding after each of the five
injected storage barriers. They check both possible disk histories, original
unknown charges and settlement, exact historical retries, old-key rejection and
successful two-key publication after explicit reactivation. The private seam
changes the interruption point, not the broker, storage or authority rules.
These additional Rust tests are authored but UNEXECUTED under the same missing
RCH limitation above.
