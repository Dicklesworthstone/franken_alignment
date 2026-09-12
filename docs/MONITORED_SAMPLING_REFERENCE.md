# Monitored stochastic decoder

## Connected consumer

`activation::monitor::decoder::sampled::MonitoredSampledDecoder` connects the existing stochastic selector to the existing all-layer monitored decoder. It serves original-token monitoring and continuation in FA-024/FA-025/FA-113, plan 10.15 and 11.1-11.3, and the independent activation-observation requirement FI-A05. This is L7 numerical reference execution with L1 computed residuals; it does not implement a serving sandbox, production inference runtime or effect authorization.

The owner starts a new decoder and requires the original complete fixed monitor roster and shared lifetime refinement budget. It cannot wrap an advanced sampled session to skip its prefix. Every explicit input token is teacher-forced through the same residual review. Every generated token is selected by the existing SamplingPolicy/Sampler and computed by the same DecoderSession. There is no second forward engine, monitor evaluator, RNG algorithm or mutable cache implementation.

## Computation and review are different commit points

`advance_sampled` first checks the original predecessor, status, context and numerical product budget. It computes the next distribution from the preceding fully reviewed logits and stages one draw using a private copy of the original sampler state. Missing preceding logits, invalid sampling limits or exhausted draw counters leave the existing session and current quiet evidence unchanged.

The original monitor owns the only execution path. Its private `advance_with_commit` seam accepts an internal infallible assignment, not a public callback. Immediately after the existing decoder successfully commits all K/V layers, token and logits, that assignment publishes the staged RNG state. Only then does the original residual review run. Its live observation is already Computing and cannot supply a quiet prefix during this interval.

A numerical failure before the token commit consumes no draw and the original monitor latches Failed. By contrast, an alarm, equality/uncertainty hold, exhausted refinement budget or later review error follows a successfully computed token: the draw stays consumed even though the token is withheld. This distinction prevents a held or failed review from rewriting a real computation as an unused random choice. The internal test also forces an arithmetic error in review accounting after successful inference to exercise the Result-error case, not only the normal Held return.

Both held and failed owners refuse subsequent forced or sampled advancement. There is no reroll, greedy fallback, reseed, monitor replacement, checkpoint export or reset method. The counter records successfully computed sampled tokens, not released-token count or a refunded computation allowance. Failed work can still consume CPU and temporary allocations without a completed-work receipt, just as in the original numerical engine.

## Release and live evidence

Only MonitoredSampledStep::Released contains ReviewedSampledStep: the original ReviewedStep together with that token's sampling diagnostics. A Held result contains only the original DecoderReview and exposes neither the selected token nor its logits/random word. Converting a sampled result to MonitoredStep discards optional sampling diagnostics without changing the release/hold branch. Debug representations omit the token, logits, seed and generator words.

`observation()` returns the SAME read-only DecoderObservation maintained by the original monitored owner. It does not wrap numerical IDs in a new purported certificate. Its capture is available only after a complete quiet prefix; old evidence becomes stale on a later successful step and unavailable on a hold, failure or owner drop. Foreign same-named owners cannot validate each other's evidence. Historical captured prefixes remain immutable but are not made current by preserving their bytes.

The observer is for the trusted supervisor, not the actor. The wrapper does not expose its raw sampled history, initial/current RNG state, numerical checkpoints or mutable inner decoder. Released sampling diagnostics are numerical data, not secure randomness or a source of secret audit selection. A caller that separately retained model parameters can run them elsewhere; this wrapper does not claim OS confinement or prevent such an explicitly separate execution.

## Bounds and compatibility

The original vocabulary/context/model caps, inference product admission, per-monitor limits and one shared lifetime encoded-byte/probe-coordinate allowance all remain in force. Sampling uses the existing bounded O(V log V) sort and rounded binary64 distribution. Its staged generator copy is fixed-size state plus a cloned fixed sampling policy, not a copied KV cache or a fresh authority budget. Forced positions consume no RNG words. No dependency, unsafe code, alternate executor or new numerical profile is introduced.

Existing MonitoredDecoder::advance and advance_greedy use the same private path with a no-op numerical-commit callback; their evidence, review, failure and budget behavior are unchanged. Existing monitor, sampling, archive and controller tests are retained unchanged. The wrapper cannot restore serialized monitor approval or turn a sampled checkpoint into current control evidence.

## Explicit configuration and the existing file consumer

`sampled::config::SamplingConfig` accepts exactly nine fields under `fa.decoder-sampling/1`: schema, id, generation, vocabulary, temperature, top_k, top_p, stream and seed. All are mandatory. IDs and stream must be positive integers; the explicitly supplied u64 seed may be zero. Vocabulary must equal the independently supplied model's. The original SamplingPolicy validates all temperature and top-k/top-p semantics. Unknown fields, duplicate keys, ambiguous types, nonfinite decimals and unsupported schemas refuse. There is no time-derived seed, inferred tokenizer behavior, reset configuration, executable path or authority field.

Configuration is bounded to 4 KiB, depth two, 32 parser items and 64-byte strings. Debug and typed errors omit seed values. This is operator data, not authenticated policy; a config does not prove independent random streams or trained-model sampling parity. `MonitoredSampledDecoder::from_json` validates sampling and the original complete monitor configuration before computing any token. It creates only an empty monitored owner.

`decoder_monitored_from_checkpoint` now accepts an optional eighth positional argument, SAMPLING_JSON. Omitting it retains the original seven-argument greedy invocation. Supplying an invalid or unavailable file fails; it never falls back to greedy. Both modes share one existing-style execution/output loop: validate the complete prefix-plus-continuation context and product budget, review every input, compute/review each generated token, then emit its original ID only on Released. The bounded file reader and the existing configured SafeTensors weight loader are reused.

For example, after compilation in a qualified checkout:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference --example decoder_monitored_from_checkpoint -- \
  crates/fa-reference/tests/fixtures/decoder_llama_config.json \
  crates/fa-reference/tests/fixtures/decoder_mixed.safetensors \
  crates/fa-reference/tests/fixtures/decoder_original_tokens.json \
  crates/fa-reference/tests/fixtures/decoder_monitor_quiet.json 16 8 1000000 \
  crates/fa-reference/tests/fixtures/decoder_sampling.json
```

This command and its synthetic fixtures are not trained inference or a completed runtime test. The output schema and exit statuses remain unchanged: completed execution succeeds, hold exits with 2, and failures exit nonzero. NDJSON never includes RNG words, seeds, next logits or sampled probabilities. A held generated token is omitted even after earlier input lines were successfully emitted. A failed write/flush stops execution and cannot rewind the already-computed token or its draw. The output itself is not an authenticated trace or an effect permit; the numerical CLI does not claim the production publication protocol.

## Existing effect admission consumes the same sampled evidence

The concurrently implemented `OversightBroker::enable_decoder_monitoring` accepts the sampled owner's original DecoderObservation without another source, writer or authorization mechanism. Actual sampled prefixes can therefore support the same exact-policy/congress/automatic-and-human-key publication path. Actor state must still match the exact original token prefix. A quiet numerical result alone cannot publish.

A later sampled hold makes that live observation unavailable and blocks preissued automatic and human keys through the original gate. Refusal preserves the existing reservation; it does not consume a second permit or refund unseen external work. Effects dispatched before the hold remain charged and reconcile through their original endpoint receipts after the numerical owner and helper inputs are gone. This batch adds integration tests over those existing gates; it neither authors a second ledger nor claims ownership of the concurrent decoder-admission implementation.

## Verification record

First increment adds ten public integration tests, two internal boundary tests and two compile-fail examples. Public scenarios compare 32 stochastic continuation steps against the original SampledSession, including each selected ID, probability/random word, logits and every query/residual coordinate. Mixed forcing verifies the draw positions indirectly against the original sampler. Two-layer synthetic controls pair successful release with first/last-layer alarms, shared-budget exhaustion and final vocabulary overflow. Other cases cover admission limits, missing roster, model/policy vocabulary mismatch, live observer staleness and closure, owner loss, independently generated mixed-precision SafeTensors fixtures, full context and diagnostic privacy. Internal cases distinguish a post-computation monitor error from precomputation draw-counter exhaustion.

Second increment adds seven configuration tests, four example-output tests and three original-controller integration tests. Cases include every missing config field, duplicates, every truncation of a complete flat config, maximum seed, exact/one-over byte budgets, no-fallback configuration failure, raw-versus-monitored sampled IDs, a generated-token hold following a quiet prefix, and output failure after a real draw. Controller tests pair successful publication in both key modes with preissued-key holds and executed-versus-unexecuted reconciliation after monitoring and helper loss. The five pre-existing greedy example tests remain intact.

All 26 new Rust test functions and both compile-fail examples remain uncompiled and unexecuted. The required RCH test command was attempted and failed before compilation because `rch` is unavailable; cargo, rustc, rustfmt and br are also absent. No local compilation fallback, assertion weakening, Beads close or gate promotion occurred. Rustfmt, Clippy and the revision-bound repository gate remain pending. These tests describe synthetic numerical/probe fixtures, not trained-model safety or vendor sampling parity. Native runtime scheduling, authenticated model/capture provenance, trained probe qualification, complete OS mediation and whole-controller durability remain separate requirements.
