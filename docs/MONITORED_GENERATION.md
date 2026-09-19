# Original-token generation with incremental learned audits

`decoder::sampling::monitored` connects the existing stochastic sampler to the existing incremental learned decoder guard. This is an end-to-end numerical generation path: fixed original prompt IDs, temperature/top-k/top-p sampling from original logits, source-checked learned KV monitoring, and original all-layer publication. It does not introduce another forward engine, approximate inference cache, RNG implementation, tokenizer, or external-effect authority.

## Entry points

Construct `GenerationSpec::new(prompt, max_new_tokens, stop_tokens, SamplingStart)` and call `DecoderModel::monitored_generation(stream, evaluation_origin, spec, policy, budget)`. The owned `LearnedGeneration` starts empty. `advance(expected_position)` processes exactly one prompt token or continuation candidate. `run_to_stop()` repeatedly invokes that same method until a stop, hold, or failure. Both routes preserve the same original computation and RNG transitions; the convenience runner has no unaudited prefill shortcut.

For explicit run-wide telemetry limits, use `DecoderModel::monitored_generation_with_telemetry(..., GenerationTelemetryBudget)`. The older entry point uses the bounded default aggregate rather than resetting an unbounded allowance on each token.

`GenerationStatus` distinguishes `Prefilling`, `Generating`, `Finished(TokenLimit)`, `Finished(StopToken(id))`, `Held(outcome)`, and `Failed(error)`. Only active runs can advance. Repeated `run_to_stop` calls on a finished or held run do no work; a failed run continues returning its failure. Stale-position calls refuse without charging or poisoning an otherwise active run.

The prompt, maximum continuation, stop IDs, sampling policy, initial RNG stream/seed, codec, complete K/V probe roster, residual retention, and all budgets are fixed at construction. There is no token override, mutable decoder/sampler accessor, unchecked-prefix adoption, imported-sampler attachment, hold-reset method, or Clone implementation on the execution owner.

## Publication and stopping

Every prompt token passes through the original learned guard before it enters the accepted prefix. Prompt ingestion never consumes a random draw. After prefill, the existing sampler privately prepares a candidate from the last accepted original logits. The candidate is then computed by the original decoder and its newly staged K/V rows are checked and audited across every layer and both sides. Only complete numerical quiet publishes the original candidate cache and logits. The sampler's next state and accepted sample history are committed immediately afterward with only infallible assignments and a preallocated push.

Alarm, threshold equality, unresolved retained evidence, and monitor budget exhaustion return held events without an accepted computation or sample. They leave accepted tokens, cache, logits, RNG state, and accepted sample history unchanged and latch the execution owner. Computation or preparation errors also latch. There is no retry path that silently searches another draw, enlarges the budget, or skips to a different prompt position.

Stop-token matching happens **after** an accepted continuation audit, never before it. A candidate that happens to be EOS cannot bypass monitoring or turn a hold into successful termination. Stop IDs appearing in the original prompt do not terminate prefill. A stop token that also reaches the continuation limit is reported as `StopToken`, and accepted stop IDs remain in the generated token sequence. There is no extra draw after either successful stopping rule.

`GenerationEvent::accepted()` exposes an original `DecoderStep` only after publication. `sample()` exposes its original `SampledToken` only for an accepted continuation. A held event still exposes its numerical audit and compression report, but not the underlying forced-token decoder event, rejected candidate ID, random word, or pending logits. Custom Debug implementations preserve that distinction. This is API publication discipline, **not secrecy**: deterministic replay state and retained numerical evidence may allow an observer to infer the candidate independently.

## Numerical and telemetry accounting

`estimate_monitored_generation` validates token IDs, sampler vocabulary/stream, and the full declared context, then prices all prompt tokens plus the maximum continuation and every full-vocabulary sampling scan. `monitored_generation` requires that complete worst-case estimate to fit `GenerationBudget`, even when early EOS seems likely. It also checks the largest single-token decoder cost against the existing frozen per-token inference limit before prefill begins. The total token inventory is bounded by `MAX_GENERATION_TOKENS`; samples are preallocated accordingly.

`GenerationWork` distinguishes monotonically reserved attempted decoder/sampling work from `accepted_decoder` work. A failed or held continuation retains its decoder reservation, sampling attempt, and vocabulary-scan reservation, even though its random draw and numerical publication are not committed. Decoder cost is reserved before sampling, so a sampling failure can reserve work that was not actually executed. These are explicit conservative loop-term reservations, not timing, physical resource consumption, or conserved authority.

`GenerationTelemetryBudget` separately conserves learned-compression source values, encoded representation bytes, compression work units, source-check values/bytes/reconstruction products, and monitor bytes/coordinates/products/materialized values/refinements across the entire run. The maximum defaults are each the corresponding bounded per-token universe multiplied by the maximum 4,096-token generation horizon. A custom aggregate may only be smaller.

Before each token, the generation owner converts its remaining aggregate into a `LearnedDecoderAllowance`; the lower-level decoder intersects that allowance again with its frozen per-token policy. No later token can reset a refinement, source-check or compression allowance already consumed by an earlier token. A monitor that runs out of remaining coordinates/refinement budget returns its existing `BudgetExhausted` hold. A compression or source-check operation that cannot fit its remaining cap fails and latches the generation without publishing the candidate.

`GenerationTelemetryWork` counts only telemetry operations that returned complete source-checked evidence: actual compression/source-check reports and actual monitor work. If preparation errors after doing some bounded internal work, no trustworthy partial report exists; the generation permanently fails, so the unseen remainder cannot be reused by that owner. This metric is therefore reported completed telemetry, not an assertion of exact physical CPU work on failed operations.

Early stopping spends only telemetry for attempts that actually entered the controller. Repeated polling of terminal states spends nothing. The incremental path still audits only one newly computed absolute position at a time and never repeatedly snapshots or recompresses the accepted prefix. Original autoregressive attention still reads its history and retains its increasing inference costs. `accepted_cache_image` remains an explicit diagnostic export, not part of the hot path.

The owner retains bounded accepted sample metadata and its latest event. Earlier event handles retained by callers keep their own immutable audit evidence alive. Generic allocator aborts are not recoverable Result-level transactions. A caught unexpected unwind leaves the outer execution owner failed rather than permitting a hidden retry.

## Integration sketch

The application supplies an actual `DecoderModel`, an independently fitted frozen `LearnedDecoderPolicy`, original token IDs, and an explicit `SamplingStart`. It may call `advance(run.position())` while `run.status().is_active()`, consume only `event.sample()`/`event.accepted()` as published numerical outputs, and inspect a hold's `event.audit()` for its stopping row and evidence. `generated_tokens()` contains only accepted continuations; `accepted_tokens()` also contains the audited prompt prefix. `sampler_state()` is the last committed replay state, never an unaccepted draw. `last_event()` is the last completed audit and can precede a later `Failed` status.

The original `SampledSession` remains available for ordinary reference inference and independent parity checks. It is not substituted into the guarded runner. Constructing a separate ordinary session does not acquire a guarded execution history or effect permission.

## Scope and verification

This connects actual sampled token publication to numerical learned-probe audits. It does not qualify the probes' empirical ability to detect harmful behavior, authenticate a serving host, classify output text, sandbox external effects, or authorize a tool call. The accepted input token's internal-state audit must not be confused with a proof that all next-token choices are safe. Existing production gates and Beads remain unchanged.

The original nine integration-test functions cover bitwise parity with ordinary sampled execution; one-position audit coverage; segmentation and terminal idempotence; prompt rejection; compression-erased sampled alarms with RNG/cache rollback; audited EOS and limit precedence; unresolved/threshold/budget holds that cannot masquerade as EOS; complete-run/per-token admission; invalid tokens, context, sampling and training-split bindings; and latched compression/source-check failures with nonrefunded reservations.

Five additional telemetry tests cover one refinement shared across positions, a probe-coordinate allowance that cannot reset, source-check bytes that cannot be reused, exact accumulation from returned reports, and aggregate-bound/terminal-polling behavior. Lower-level allowance plumbing also keeps ordinary one-token callers unchanged. Two compile-fail examples continue to cover mutable-guard bypass and event-to-permit conversion.

The required remote invocation for these changes is:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_generation --test learned_generation_telemetry
```

RCH is not available in this tool environment, so the new Rust has not been compiled or executed here. Tests, doctests, formatting, Clippy, and revision-bound qualification remain unverified. No Beads item or production gate is closed by this source construction.
