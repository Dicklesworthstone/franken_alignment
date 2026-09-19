# Incremental monitored text generation

`TextDecoder::into_generation` consumes the same native text owner and freezes a
complete `TextGenerationRequest`. Text and numerical admission complete before
inference starts. It returns `TextGenerationSession`, which owns the original
`GenerationSession`, fixed tokenizer, original byte prompt and preallocated output.
The borrowed one-shot `generate` API remains available and shares text admission
and exact output validation with this path.

`advance(expected_position)` computes at most one original forced or sampled
token. Its borrowed `TextChunk` contains only newly released content IDs and their
exact bytes, together with the cumulative byte range, native finish and work.
Prefill, a held token, a stop token and budget exhaustion yield no new bytes.
Repeated terminal polls also yield empty deltas without another sample or charge.
The caller drives every step; there is no lookahead, sink I/O or background work.

The complete prompt, explicit controls, stop set and cumulative budgets cannot
change during a session. Full prompt tokenization is not repeated between pulls.
Native monitoring remains mandatory before output can be decoded. Byte chunks
may split Unicode characters: cumulative `utf8()` is strict, while `bytes()`
retains all original bytes without replacement characters or silent dropping.
No chunk is a publication receipt, semantic safety guarantee, or effect permit.

## Completion, yielding and cancellation

Yield by retaining the session without calling `advance`. This retains its actual
KV cache, PRNG and budgets. `into_parts` returns the original TextDecoder and
TextGenerationReport only after native termination. Held/failed owners keep their
original latches; no new tokenizer, seed, cache or monitoring approval is created.
A subsequent request can continue the same numerical history after a permitted
terminal outcome, without reconstructing or retokenizing previous text.

`cancel` destroys the unfinished numerical owner before returning a separate
`CancelledTextGeneration`. It performs no additional inference, invalidates live
DecoderObservation handles, preserves the last safely decoded prefix and actual
work/draw counters, and returns no owner that could resume an incomplete prompt.
The native finish may remain None; cancellation never fabricates a successful
GenerationReport, refunds a draw, erases an alarm or settles an external effect.
Calling the consuming into_generation on invalid input also consumes its owner,
matching the original numerical API; use borrowed generate for retryable admission.

The wrapper latches before entering a native step. A caught unwind or unexpected
output contract error forbids additional pulls and owner export. Cancellation
still provides historical progress and explicit interruption/error diagnostics.
After interruption, returned native progress and decoded bytes can lag actual
work counters; those are not claimed to be a fresh complete observation.

## Verification status

Fourteen additional Rust tests and two compile-fail examples are authored. They
cover real native decoder equivalence, deltas, stale and terminal polls, prefill
cancellation, live-source withdrawal, held/stop tokens, split and invalid UTF-8,
cumulative budgets, continued KV history, complete-input admission, planted output
failure and caught post-native unwind. Existing text/tokenizer tests are retained.

The required `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`
could not start in this environment (`rch: command not found`, exit 127). Rust
compilation, formatting, Clippy and all Rust tests remain unexecuted. Lexical and
whitespace checks are not a substitute. No runtime integration, trained-checkpoint
qualification, production activation or bead closure is claimed.
