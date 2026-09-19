# Native helper inference (plan §9.1–9.3)

`helper_client::native::NativeEvaluator` connects the existing worker request to
actual native monitored text inference. It owns a fresh `TextDecoder` and a frozen
`NativeHelperPolicy`, not another broker or a callback returning asserted votes.
The worker operator independently registers an exact `InputProfileBinding` to
`DecoderProfile` mapping. Semantic epochs and decoder generations are separate
namespaces; numerical equality is not assumed to bind or authenticate them.

Every `WorkerInput.actual_input().submitted_bytes()` byte becomes model input,
unchanged. Transport framing, round/member identities and commitment salts are
not model input. Parts and omissions remain in the retained original request.
No automatic BOS, chat template, question suffix, trimming or truncation is added.
The registered question must already request the categorical response schema.

A verdict requires the original generation to finish with a completely reviewed
terminal control and complete prompt coverage. The complete output must be
exactly `allow`, `hold`, `deny` or `abstain`. Whitespace, prose, multiple answers,
malformed bytes and truncated output do not produce a vote. A monitor hold,
budget exhaustion, input refusal or missing terminal cannot become an implicit
Allow, Hold or Abstain. Only registered non-text controls may stop generation.

Each evaluator attempts one request. Its no-retry latch is installed before any
fallible inference work and survives a caught unwind. It retains the original
numerical report even for a held, truncated or invalid response. Reported work
and random draws are not refunded, and a later call cannot select another answer.
The independently held model parameters may exist elsewhere; this is not process
isolation or protection against a privileged operator recreating a worker.

This bounded CPU/FA-BBPE reference path does not establish checkpoint authenticity,
trained-tokenizer compatibility, classifier accuracy, calibration, helper
independence, production runtime admission, or effect authority. The existing
reference helper wire/commitment protocol remains unchanged.

## Verification status

Ten new Rust tests use actual native model computation with explicitly synthetic
parameters, the existing linear monitor and tokenizer. Controls produce all four
responses, demonstrate input-dependent results, and pair failures with permitted
execution. Negatives cover profile binding, incomplete/held terminal results,
strict output schema, full-input limits, content-stop refusal and no reroll.
These are not trained-model or power-failure tests.

RCH is absent in the editing environment. Rust compilation, rustfmt, Clippy,
Rust tests and the full xtask gate are UNEXECUTED; no bead is closed or feature
qualified by these source changes.
