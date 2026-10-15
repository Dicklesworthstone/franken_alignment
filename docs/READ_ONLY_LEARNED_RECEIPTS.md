# Read-only learned receipt completion

`FileLearnedRecovery::finish_receipt(request, finish_after)` consumes a complete
original numerical replay into `Knowledge<ActorOutcome>`, without returning a
live owner or writing a recovery fence. `None` selects a message; `Some(id)`
selects a finish bound to that exact confirmed message predecessor. The ordinary,
raw-predictive and owned-K/V guarded cursors expose the same operation only after
their full independent guard, policy, model/predictor and floor checks pass.

This closes a capacity gap in receipt retrieval: an original retained result
can be inspected when there is no space left for another fence, and repeated
reads do not consume the remaining emergency reserve. It is scoped FA-014 work
serving the effect/receipt distinction in plan sections 8.4 and 16.4. The full
packet and its prerequisites remain open.

The finalizer uses the original retained message/finish validator and the SAME
actor projection used by live polling. No second outcome reducer is introduced.
A denied admission needs no FrozenAction. Pending work stays Pending; dispatched,
unknown and even published-but-unreconciled work stays Unknown. No source is read,
no logical clock is consulted, no effect is repeated and no liability is refunded.
Original numerical work still executes to verify historical witnesses during
replay; this is not an inference-free or constant-cost journal read.

Before returning data, the finalizer compares the entire canonical image against
the bytes originally held under the exclusive Store lock. Incomplete/failed
replay, invalid selection, unmet guards/floors or changed bytes expose no result.
The private machine and lock are dropped on completion, including on error.
There is no cleanup, replacement, sync, fence, reset, observer issuance or pending
intent completion. Staged files remain untouched. Existing writable recovery,
stop recovery and reconciliation retain their original behavior.

The result is historical at this verified cut, not a fresh evaluation, current
permission, or proof that an unresolved effect did not occur. Access authorization
is still the embedding operator's responsibility. Independently retained anchors
remain optional inputs to the existing anchored openers, not authentication of
the suffix or protection against jointly rolling back the journal and its anchor.
This read does not fsync or acknowledge a new canonical image and does not issue
a successor custody anchor. Cooperative event limits are not hard time bounds.

Four regression functions are authored using the original loaded numerical,
forecast, native review, human approval, request book and publication owners.
They compare returned observations against the live wire; exercise denied and
confirmed messages in all three modes; exhaust ordinary and terminal event slots;
retain Pending/Cancelled/Dispatching/published-unreconciled states; and reject
partial replay, busy writers, bad selection, unmet floors and canonical changes.
The test fixture receives an explicit optional configuration to exercise real
small journal limits; existing callers and assertions retain their old behavior.

These Rust tests remain UNEXECUTED. The required remote-only RCH test command
stopped before compilation because `rch` is absent; `cargo`, `rustc` and `rustfmt`
are absent too. Git preimage/whitespace validation is not Rust validation. No
production, compilation, formatting, test-suite or performance qualification is
claimed. The executable receipt-only option is a separate consuming increment.
