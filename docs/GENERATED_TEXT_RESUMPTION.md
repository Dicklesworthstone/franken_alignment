# Resume original text computation before publication

## Native selection, not replayed permission

`FileOversight::prepare_decoder_text_publication_recovery(generation, expected)`
selects an existing native text intent for the generated-publication consumer.
The complete independently retained `TextGenerationRequest` must match: prompt,
control and stop IDs, token/output limits, numerical budget and tokenization
budget. The result is the original immutable command, not output, an approval,
a fresh budget or an imported numerical cursor.

The method admits a pending intent or a naturally completed, fully prompt-reviewed
Control-stop result before its first message submission. It checks the current
original numerical position/revision, absence of another pending generation,
settled and unfinished stream state, and the full remaining disclosure allowance.
A generation already linked to ANY message request refuses, including cancelled
or rejected submissions. Receipt recovery is a separate operation; a new effect
request must not silently republish a previously submitted native result.

Preparation works while the recovered decoder is paused or source/time readiness
is stale. It performs no I/O, numerical step, journal write or authority change.
Actual continuation still requires independently supplied fresh source/time and
original explicit resume. Every token and the eventual native message submission
retain their original checks; UTF-8/output admission, full-input helper review,
fresh human approval and automatic authorization are not supplied by preparation.
Held, cancelled, failed, token-limited and superseded results cannot be rerolled
through this API. Original lifetime spend, per-intent budgets and random draws
remain in the same numerical history.

Before a TextMessage submission, numerical history does not bind a future effect
request ID or deadline. The operator must separately select a fresh unused
publication request and its deadline. This is first admission of an existing
native result, not renewal of an old effect deadline. The pinned native-only
stream opener remains responsible for model/monitor/sampler/tokenizer/bootstrap
validation and its recovery fence. No ordinary caller-text deployment is upgraded.

## Tests and qualification

Seven authored native tests cover every prompt/sample/complete interruption cut,
unchanged work against uninterrupted computation, exact intent/budget mismatches,
cancellation and token-limit refusal, previously submitted then cancelled output,
superseding numerical work, all five last-token storage barriers followed by
actual canonical recovery, and refusal to upgrade a caller-text deployment.
Fixtures run the original numerical and journal reducers, not imported outcomes.
Fault injection is deterministic and is not hardware power-loss qualification.

The required command was attempted on 2026-10-07:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It failed to start with exit 127 (`rch: command not found`). Compilation, Rust
tests, rustfmt and Clippy remain UNEXECUTED. Source/hash/whitespace inspection
is not execution evidence. No production gate, deployment assurance or bead is
closed. This extends the existing L1/L5 recovery consumer without changing native
journal formats, the dependency universe or the original authority semantics.
