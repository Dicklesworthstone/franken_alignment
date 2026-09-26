# Explicit native stream completion

## Library contract

`FileOversight::prepare_decoder_text_finish(after_request, request, deadline)`
prepares the existing `FileStreamProposal` for an explicit finish after the latest
receipt-confirmed native message. It shares continuation's original confirmed-cut
validation: exact source linkage, Executed receipt, complete current message
boundaries, no unresolved publication or outstanding generation, unchanged
numerical predecessor, Ready monitor, and no Stop or suspension.

Finishing does not generate another token or append another message. It therefore
needs no unused message slot, content-byte allowance or numerical context position.
It is still a separately reviewed effect: the full cumulative frame is charged,
and the original source, helper, human, witness and publication requirements apply.
Timeout, cancellation and a partial/held generation are not substitutes for finish.
This implements the explicit closing portion of plan section 8.7 using the
existing L5 stream and actor vocabulary, not a new effect or journal format.

Preparation is read-only and can inspect a paused recovered owner. It neither
refreshes evidence nor clears the pause. Recheck it after original qualification,
source/clock acquisition and explicit resume, then submit the resulting identity
through `FileGeneratedTextActorPort::encode_finish` and the original actor wire.
No actor-provided prefix or output bytes are accepted by that port.

`decoder_text_finish_request(after_request, request)` instead reconstructs an
already recorded finish, including refused, cancelled and expired requests. It
uses the original request specification, predecessor receipt and exact full
message boundaries, never today's prefix, epoch or deadline. The previous native
request is not interchangeable with another one that happened to produce equal
text. This lookup creates no key, performs no reconciliation and retries no effect.
A storage-faulted owner refuses. Recovery must separately pin its full original
model, tokenizer and selected publication contracts as before.

## Source and validation status — 2026-09-26

The shared confirmed-cut helper preserves continuation's validation predicates;
all existing continuation tests are unchanged. Six new tests exercise original
synthetic inference and real canonical-file review/publication: full stream
capacity, unreconciled publication, a later zero-token intent, retained refusals
and cancellations, equal-text predecessor conflicts, paused recovery, subsequent
inference, and ambiguous storage failure. Fixture weights and ballots do not
qualify detectors, evaluator identity or hostile-process isolation.

UNEXECUTED: the fresh required RCH gate stopped before compilation because `rch`
is unavailable (exit 127). Rust compilation, tests, rustfmt and Clippy have not
run. Source/whitespace/hash inspection and historical gate results do not validate
new runtime behavior. No Beads item is closed, and no production qualification,
anti-rollback guarantee or remote-publication transaction is claimed.
