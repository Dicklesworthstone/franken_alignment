# Monitored original-token checkpoint example

Source status: the example and its nine tests are uncompiled/unexecuted. Run the
repository's required verifier through RCH; do not treat this source as a green
execution receipt. The lower-level contracts are MONITORED_DECODER_REFERENCE.md
and MONITORED_SAMPLING_REFERENCE.md.

The `decoder_monitored_from_checkpoint` example uses the existing bounded Llama
configuration and SafeTensors file loader. Seven arguments select greedy output;
an optional eighth selects explicitly configured stochastic sampling:

```
CONFIG_JSON WEIGHTS_SAFETENSORS TOKEN_REQUEST_JSON MONITOR_JSON CONTEXT NEW_TOKENS PRODUCT_BUDGET [SAMPLING_JSON]
```

A synthetic source-checkout greedy invocation is:

```bash
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example decoder_monitored_from_checkpoint -- \
  crates/fa-reference/tests/fixtures/decoder_llama_config.json \
  crates/fa-reference/tests/fixtures/decoder_mixed.safetensors \
  crates/fa-reference/tests/fixtures/decoder_original_tokens.json \
  crates/fa-reference/tests/fixtures/decoder_monitor_quiet.json \
  16 8 10000000
```

Append `crates/fa-reference/tests/fixtures/decoder_sampling.json` to use the
retained stochastic example policy. That bounded `fa.decoder-sampling/1` document
explicitly declares policy ID/generation, vocabulary, temperature, top-k, top-p,
RNG stream and seed. No values are silently defaulted. A missing or invalid
supplied sampling file refuses; it does not fall back to greedy generation.
See MONITORED_SAMPLING_REFERENCE.md for the numerical and RNG commit contract.

The command uses the repository's selected toolchain; the illustration itself
is not a toolchain or execution qualification. The original unmonitored example
is unchanged. There is no fallback from the monitored example to that path.

The token request retains the original identity-plus-token-ID schema. No
retokenization, guessed BOS, EOS interpretation or tokenizer implementation is
introduced. Before its first numerical step or output event, the execution loop
checks the entire original-token list, context and prefix-plus-continuation
product budget. Each subsequent numerical allowance is that step's exact portion
of the admitted total. Monitors consume their separate, cumulative session budget.
Both selectors share this same preflight, review and output loop. Forced prefix
positions never consume a stochastic draw.

Standard output is newline-delimited JSON. Prefix steps emit `input_reviewed`.
Generated steps emit `generated` with a token ID **only after that step's complete
residual review**. A held step emits `held` without the selected token or logits,
reports incomplete layer coverage and retained work, and ends execution with exit
code 2. A completed requested run emits `complete` and exits 0. Input/numerical/I/O
errors exit 1. Every event explicitly says `permission: not_issued`. Neither mode
prints seeds, generator words, sampled probabilities or next-token logits.

This is streaming output: tokens already reviewed and printed are not retracted
by a later hold or failure. The loop neither buffers a speculative unreviewed
suffix nor prints an unexecuted next-token prediction at completion. A failed
output write stops the loop before another numerical step. A sampled token already
computed before that output failure retains its consumed RNG draw; the loop does
not retry with a replacement token. This is not network backpressure, asynchronous
cancellation, an external effect journal or a sandbox. Operator-controlled file
paths must remain stable; metadata/open races and source authenticity remain
outside this example's regular-file checks.

The five original example tests remain intact: complete greedy continuation,
no token/logit emission on hold, full-run preflight with no work/output on refusal,
termination after a BrokenPipe and the original-ID request boundary. Four new
example tests cover sampler-equivalent output without RNG disclosure, a generated
sample held after a quiet prefix, sampled full-run admission, and a BrokenPipe
after a committed draw with no automatic reroll. Their presence is test source,
not a passing run. Synthetic weights and the broad quiet threshold are controls
only; the example does not supply a trained or calibrated monitor.
