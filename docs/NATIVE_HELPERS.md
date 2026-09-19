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

## Native worker transport

`native::peer::NativeHelperClient<S>` owns the original `HelperClient<S>` and one
unused `NativeEvaluator`. It accepts an independently provisioned salt, not an
external verdict. The request is fully decoded and profile-checked by the original
wire parser before inference. A successful evaluation feeds `HelperClient::respond`
exactly once, freezing both original commitment/reveal frames before any write.
The salt is never part of the model prompt. The native profile requires at least
16 salt bytes, checks the supervisor's salt limit before inference, and does not
claim to generate entropy or upgrade the reference FNV comparison to cryptography.

`step` performs one original bounded I/O operation OR the one bounded synchronous
model evaluation. `drive` caps state-machine calls and yields on backpressure,
input readiness, judgment, completion or failure. Counts are not syscall/latency
bounds. WouldBlock and Interrupted preserve the original offsets and never rerun
the model. Native judgment means the response is frozen, not that the congress
has accepted it. No reveal is sent before the original supervising R request.
`from_unix` selects nonblocking I/O; `AsFd` supports a host-owned readiness loop.
No listener, network dial, executor, checkpoint loader or actor privilege is added.

A failure latch precedes every transport/inference call, including a caught
unwind after a partial write. Later calls perform no I/O or inference. Cancellation
is terminal and never sends a substitute vote, erases already sent bytes, refunds
work or removes the worker from the congress denominator. Drop the owner to close
its socket; the original supervisor applies absence/deadline rules. Synchronous
inference is not preempted by this API; its registered numerical budget still
bounds admitted work, and cancellation is checked between host calls.

Eleven additional tests cover fragmented I/O, write/flush backpressure, stale and
truncated frames, failed/held output, salt limits, cancellation, fatal I/O, actual
unwind after bytes were accepted, drive limits, old-evaluation import and real
Unix sockets. An end-to-end test sends the original broker's frozen helper view
through `HelperConnection`, native inference, `HelperRound` and the original
`OversightBroker`: a completed verdict permits the normal authorization check,
while a held worker supplies no commitment and remains missing at the deadline.
The publication endpoint executes nothing in that test; helper judgments are not
external effects. All 21 new Rust tests remain UNEXECUTED pending RCH verification.
