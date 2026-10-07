# Explicit closure of native generated publication streams

## Runnable finish consumer

```sh
supervise_publication create-generated CONFIG ORIGINAL_RECIPE REVIEWER_PROFILE --finish FINISH_REQUEST_ID
```

`ORIGINAL_RECIPE` identifies the last receipt-confirmed message: its original
request ID, generation ID, exact prompt/control/stop tokens, budgets and pinned
model/monitor/sampler/tokenizer/stream contract. It is not a new generation. The
separate nonzero finish request ID must be unused and distinct from that message.
The recipe's lifetime bounds the NEW closure effect; it cannot retry an earlier
finish or renew a previously submitted effect. Malformed and mixed options refuse
before opening configuration files, sockets or the journal.

The existing generated-stream opener validates the immutable deployment and
fences old keys. The consumer matches the original source-linked message and
calls `prepare_decoder_text_finish` on its latest confirmed native cut. This
requires settled publication, no pending or superseding numerical work, an
unfinished stream and the original healthy numerical state. This pure check
works during recovery pause; it cannot grant authority by itself.

The same independent stop listener exists before exclusive recovery and remains
active through the existing approval/publication workflow. Fresh file evidence
and the post-read clock precede the original explicit numerical resume. Resume
restores readiness without executing a token. The native finish preflight is
rechecked after those observations. The original generated actor gateway then
builds the complete cumulative finish frame, obtains its own current admission
snapshot and records the request in the original request book.

Helper congress, independent peer-checked human review, automatic authorization,
checked endpoint publication, reconciliation and child cleanup are the existing
`execute_serviced` path. No new decision reducer, effect executor, request book,
wire format or dependency is introduced. Helpers must independently pin the
post-recovery policy epoch; a previous human key is not reused.

## Closure is an effect, not a generation status

A monitored EOS, byte limit, context limit, timeout, rejection or local Stop is
not a stream-finish receipt. Closure adds no text, token, message or sampler draw,
so a stream at its message/byte/context limit can still close, subject to the
original journal and rights capacity. Its complete cumulative frame is charged
and reviewed like any other effect. Previous messages remain visible, and their
charges never rewind. A rejected closure leaves the stream unfinished; a source
failure can stop the owner without marking the published stream finished.

Only the original restricted actor response goes to stdout. An Executed finish
confirms the local canonical endpoint operation, not delivery to a remote reader.
A failed response write does not trigger another publication. The synchronous
opener replays historical numerical work; the listener is not a watchdog inside
that replay. This adds neither operating-system isolation nor independent
anti-rollback protection, clock authentication or detector-quality evidence.

## Implementation and verification

Six authored regression functions use actual native model computation, original
source capture, epoch-pinned helper subprocesses, peer-checked reviewer sockets
and canonical publication. Cases include closure after an ordinary message and
at the exact stream byte/message ceiling, full-frame charging, unchanged numerical
state, independent rejection, wrong source/recipe/budget and reused IDs, source
loss, rejected predecessors and closed command-option admission. Synthetic model
weights and test decisions are not empirical safety or human-understanding proof.

On 2026-10-07 the required verification was attempted:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It could not start (`rch: command not found`, exit 127). Compilation, Rust tests,
rustfmt and Clippy remain **UNEXECUTED**. Selected-file patch and byte/hash checks
do not substitute for those gates. No bead, production gate or qualification is
closed. The existing plan's explicit complete-message and separately reviewed
finish semantics are unchanged; this is their runnable consumer.
