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
