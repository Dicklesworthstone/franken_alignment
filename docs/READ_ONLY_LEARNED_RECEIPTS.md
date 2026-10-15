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
claimed. The following executable integration consumes this same finalizer.

## Executable integration

The learned publication command now exposes this finalizer as an explicit mode:

```text
create-learned-generated CONFIG LEARNED_RECIPE --resume --receipts-only [--history-anchor INPUT]
create-learned-generated CONFIG LEARNED_RECIPE --resume --receipts-only --finish-after-message PREVIOUS_REQUEST [--history-anchor INPUT]
```

This is not a change to ordinary `--resume`. Without `--receipts-only`, the original
owner recovery and reconciliation still run, including their fence and capacity
requirements. With it, the same fully guarded cursor supplies only the original
historical actor response, then drops the private machine and exclusive lock.
In particular, a published-but-unreconciled message remains Unknown until an
explicit mutating recovery accepts its actual endpoint receipt.

The outer receipt-option router delegates this exact learned command before
baseline service parsing, so it cannot accidentally select a non-numerical
recovery profile. The learned parser rejects live continuation, next-message,
stop, audit, credibility activation and successor-anchor-output combinations
before intake. The original path-valued option parser runs before stripping the
boolean flag, avoiding repair of malformed anchor arguments. Input anchors still
use their original bounded reader, external-custody checks and exact-prefix law.
No actor, reviewer or stop socket is created and no live helper is loaded.

A single monotonic elapsed budget starts before command intake. It is checked
before model loading, before guarded open, at every original replay event and at
Ready, and again after the final canonical-file comparison. Expiry returns no
response; it performs no fence or cleanup. An individual synchronous read or
reducer operation is not interruptible, so this is not a hard latency guarantee.
The original model, tokenizer, codec and predictor assets are still required to
verify numerical history. No reference to a vanished native helper/evidence file
causes a fallback to weaker replay or a newly created owner.

The existing response encoder and exit semantics are retained. Confirmed
execution can return success; denial, Pending and Unknown do not become successful
publication just because receipt inspection completed. A failed or partial stdout
write never causes an effect retry, another fence or a changed stored outcome.
No new wire schema, journal format, dependency or authority command is introduced.

Five additional regression functions cover actual command intake/output and the
outer dispatcher; denied and native/human-confirmed messages and finishes in all
three modes; wrong finish selection; published Unknown followed by explicit
original reconciliation; every replay/Ready/post-read timeout; conflicting flags;
and output failure. All original source, deadline and recovery assertions remain
registered. These five tests and the earlier four remain **UNEXECUTED**: targeted
and full remote-only RCH launches fail before compilation because the launcher
is absent. No Rust compiler, formatter, Clippy or runtime qualification is claimed.
