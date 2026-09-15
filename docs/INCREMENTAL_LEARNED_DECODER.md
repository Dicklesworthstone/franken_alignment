# Incremental learned monitoring at the original decoder publication boundary

`DecoderModel::monitored_session` now runs the original forward implementation with an independently fitted, frozen learned codec and complete frozen K/V monitor roster. It owns an initially empty original DecoderSession and has no mutable session accessor, unchecked prefix adoption, hold-reset method or Clone implementation.

For each original input token, the decoder computes its ordinary pending logits, layer observations and all-layer KV bytes exactly once. Before publication, the adapter captures just those newly staged rows through the existing ModelKvCapture implementation, preserving the actual stream, absolute position and sequence. This is an independent one-position source window with source_revision 1, not a historical checkpoint whose earlier rows have been silently removed. The original codec compresses it under its declared held-out lineage checks; the original source checker derives bounds and retains the frozen head selection; the existing adaptive monitor evaluates every layer and both K/V sides.

Only complete numerical quiet invokes the same original all-layer publication transaction used by ordinary DecoderSession::advance. The original numerical forward implementation and atomic capture transaction are not duplicated, approximated or replaced. The private publication helper is extracted without changing its transaction body. Pending data has no public constructor.

Alarm, exact threshold equality, unresolved residuals and monitor budget exhaustion produce held events without pending logits or original query/residual observations. They latch the stream and do not publish any cache layer, token, logits or successful-decoder work. Actual computation/preparation errors also latch. Stale positions and invalid token IDs are rejected before execution without poisoning a valid stream. Neither an error nor a hold can be bypassed by skipping to another token or supplying larger limits to advance. Last accepted logits are available explicitly as accepted_logits; they are never relabeled as the rejected candidate's logits.

The incremental path never exports, copies or recompresses its accepted prefix. Its new source coordinates per admitted token are exactly the model's stored KV width across all layers. Codebook and descriptor bytes are charged in every standalone one-position representation; no wire-level codebook amortization is claimed. Original autoregressive attention still reads history and retains its own increasing inference costs. An explicit accepted_cache_image diagnostic export is separate and is not invoked by advance. The wrapper retains only its most recent event; callers who retain earlier event handles own those observation lifetimes.

LearnedStreamRetention can retain no residuals, all residuals, or a fixed set of structural layer/side/head keys. Structural keys are mapped to the current absolute position before source checking, not treated as a caller-selected scan subset. All probes/taps remain mandatory regardless of residual availability. Training origin or stream reuse refuses before the first forward pass. These checks do not authenticate arbitrary external model names, task labels or serving hosts.

This is an actual numerical decoder-state/logit publication guard, not a host-wide sandbox, an empirical harmfulness guarantee, a token-policy classifier or an external-effect authority. An input token is not itself the next output token, and a model's next-token selection requires subsequent task-specific controls. Accepted events remain observations, with no conversion into Permit. Resumption from preexisting checkpoints, trained detector qualification, host authentication and outside effect-release adapters remain separate work.

## Initial regression sources and execution status

Six new integration-test functions cover bitwise parity against ordinary nontrivial decoder execution; one-position source coverage rather than repeated prefix inspection; a late compression-erased alarm that withholds distinguishable pending logits while preserving all accepted cache bytes; missing evidence, threshold equality and exhausted refinement holds; structural head retention across absolute positions; declared train/evaluation overlap, stale/invalid calls, and latched preparation/inference failures. A compile-fail example rejects converting an event into an effect permit.

The targeted command was attempted in this environment:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_decoder
```

It failed before compilation with rch: command not found (127). Rust compilation, test execution, doctests, formatting and Clippy remain unexecuted. Source reconstruction of the existing decoder was checked against its Git blob SHA before the narrow helper extraction. This is source-integrity verification, not a Rust test pass. No dependency, unsafe block, Beads status or production qualification gate was changed.
