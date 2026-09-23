# Native text to complete-message admission

## Consumer and capability

`FileOversight::submit_decoder_text_message` connects the original durable text
result to the original complete-message/two-key publication pipeline. The
supervisor supplies a `FileTextMessageRequest`: durable effect request ID,
generation ID and generation revision, expected target, policy epoch and
deadline. It cannot supply replacement output bytes, a summary, a selected suffix
or a claimed numerical result. The message is derived from the original recorded
text command, immutable tokenizer and numerical result inside the same owner.

The source must have a fully reviewed prompt and a natural, monitored control
stop. Pending, held, failed, budget-exhausted, cancelled and token-limit finishes
refuse. Every configured stop ID must be a control; suppressing a content stop
cannot hide a contradictory suffix. Empty or invalid UTF-8 output refuses;
there is no replacement-character conversion, auto-EOS or extra token to repair
an incomplete message. The live numerical actor revision and position must still
match the end of that original generation, including its actual prompt work.
Later inference or an actor reset cannot substitute another live predecessor.

The existing stream builder constructs the complete cumulative message frame:
all confirmed messages, original boundaries and full-frame resource charge.
The existing request book allocates the attempt, and the original proposal
reducer decides admission, including recorded policy refusals. This operation
does not authorize, call a helper, obtain a human key, dispatch or publish. All
those original checks remain required, including revalidation at publication.
No alternate numerical implementation, tokenizer or outcome ledger is introduced.

This supplies a canonical source link for THIS operation. It is not a new
mandatory provenance policy on all ordinary proposals, and it does not remove
other existing supervisor or actor APIs. An ordinary submission cannot acquire
a generated-source link retroactively. Distinct effect request IDs are distinct
proposals; this is not a global deduplication promise for all equal text.

## Durability, replay and retries

One new oversight event, tag 31, records the source request and the initial
policy snapshot. Its decoder recomputes the message from the original prior
numerical history and expands only into the original `SubmitRequest` reducer.
The expansion is not a second canonical event, and there is no caller-supplied
callback, action frame or imported result in this path. Prediction preflight,
coverage-loss handling and request bookkeeping retain the original semantics.

The original journal size/event and recovery-reserve checks apply; this event
uses ordinary work capacity. One canonical replacement acknowledges the request
and its source link together. A storage failure returns no candidate status and
makes the original owner unavailable; a subsequent recovery must inspect the
actual canonical image. Old readers reject the new tag; every preexisting event
encoding retains its meaning. No authority, tokenizer or numerical result format
is replaced. Prior-history replay may perform real numerical work: no new token
is sampled for submission, but no constant-time/zero-CPU claim is made.

Exact retries compare ALL source/destination/deadline fields and return the
current original request disposition before source, clock or predecessor checks.
The initial snapshot is not reevaluated on retry, just as for `submit_request`.
The historical complete frame is never rebuilt against a newer stream prefix.
A lost reply cannot create a second attempt or pause a newer generation.
`decoder_text_message_request` reads the source link from the bounded canonical
event history; it is not a permit or an assertion that the message was published.

The scope remains the existing local journal-as-publication sink. No remote
recipient receipt, real model accuracy, evidence authentication, independent
human identity or OS containment guarantee is introduced. This serves the plan's
complete-message release and exact effect-binding contracts, not a completed
G1/G2/G3 deployment or broader bead closure.

## Authored regressions and execution status

Eight `generated_message_` tests use the original synthetic-weight numerical
fixture and original complete-message broker. They compare source-linked and
ordinary proposal accounting; reject unfinished/held/cancelled/content-stopped,
empty or non-UTF-8 sources; preserve consumed work; check stale source/target and
exact retries; reject retroactive provenance; and follow actual congress,
human-approval, dispatch, publication and reconciliation transitions. Both
whole-generation and incremental natural completion are exercised. Cumulative
context is retained when a second actual generation is submitted.

The targeted RCH runner is unavailable in the implementation environment.
Compilation, tests, rustfmt, Clippy and the full gate have not executed. Source
hash and whitespace checks are patch-integrity checks, not runtime qualification.

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generated_message_
```

## Source-linked recovery inspection

`decoder_text_message_snapshot` reads the acknowledged owner. Its read-only
counterpart `read_decoder_text_message` reads one entire canonical image beside
a locked/faulted owner or after restart. Both return `FileTextMessageSnapshot`:
the source link, the original request disposition, the original numerical/text
report and the enclosing stream's published and receipt-confirmed prefixes.
An unacknowledged visible write is not mislabeled acknowledged by the reader.

The reader requires independently supplied stream, full model/monitor/sampler
configuration and exact tokenizer. It checks the stream contract and uses the
existing text preflight/replay body before extracting anything. The old text
reader/opener share that body unchanged, apart from taking an already decoded
event slice. The new reader decodes once and numerically replays once. It does
not reread another file for status, import saved output, create roles, acquire
a writer lock, clean staging, append a fence or submit an action. The source
request namespace is checked before message derivation; an existing ordinary
request cannot be relabeled even if its old stream predecessor is now obsolete.

Later inference, cancellation or fences do not erase the earlier source link;
read-only historical extraction does not require that old source to remain
live. Missing means no source-linked request in this particular valid image,
not nonexecution or absence in all history. Writable recovery still uses the
existing independently constrained anchored/guarded entry points. The reader
is not an anti-rollback authority or an authentication boundary, and a digest or
source identity does not imply a model's generated statement is correct.

Nine additional regression functions exercise read-only live/recovered views,
all five original admission barriers, all five publication barriers, exact
stream/model/tokenizer pins, invalid suffixes and source-record forgeries,
independently specified tag-31 bytes and parser limits, later-inference
invalidation at dispatch and publication, exact/one-under event
capacity, modeled source interruption, and missing/lost-forecast parity against the
original ordinary admission route. Anchored recovery preserves original work,
cancels only undispatched requests and never resends a published-but-unconfirmed
message. The forecast test checks the original refusal and preservation of coverage state;
it is not a calibration or future-token-exclusion experiment.

All seventeen authored regression functions, the compile-fail example, Rust
compilation, rustfmt, Clippy and the full gate remain UNEXECUTED. Every targeted
implementation attempt failed before compilation because `rch` was not found
(exit 127). Synthetic weights and the original Store fault seam are not measured
trained-model, power-loss, recipient-delivery or isolation evidence. No broader
bead is closed and no production qualification is asserted.

The base was refreshed to `d9ce3e37b793051f825125df848465cc4697c11b` after
main gained two nonoverlapping batching commits. The source-bound submission
also has an authored regression for that original batched text API: a batch
ending after content but before the monitored stop is not a complete message;
a later terminal batch keeps its original per-token generation revision and
spent work when the message proposal is recorded. No batching source is replaced.
