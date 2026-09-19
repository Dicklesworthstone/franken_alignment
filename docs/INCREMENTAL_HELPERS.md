# Cooperative native helper evaluation

`NativeEvaluator::begin` admits the complete original worker input without
inference. `advance(expected_position)` then runs at most one original monitored
forced or sampled token. A scheduler can interleave helpers and cancel a long
prefill without waiting for the entire request. This is cooperative execution
between tokens, not preemption inside a matrix operation or a new runtime.

The original `evaluate` convenience method now loops over these same operations.
The helper policy, full byte prompt, exact tokenizer, output bounds, stop controls
and cumulative generation budgets cannot change at a yield. No text round trip,
replacement sampler, speculative token or callback-asserted verdict is introduced.

`NativeEvaluationProgress` exposes only status, original numerical work counters
and coverage counts, not partial answer bytes, token IDs or logits. An answer
spelling such as `allow` remains uncommitted until its terminal control passes the
original monitoring checks. The same complete-output reducer still requires exact
categorical syntax and complete prompt coverage. Held, exhausted, malformed and
truncated results yield no vote; their completed numerical reports remain available.
A successful terminal poll is historical data and never computes again. Neither
begin nor evaluate can accept a second input, including after failure.

`cancel` destroys the numerical owner before returning and invalidates retained
live decoder observations. Original work counts and last coverage remain visible;
no draw is refunded and no partial report is promoted into a complete one. Failed,
interrupted or already judged results keep their original status. Cancellation of
unfinished healthy work has the distinct `Cancelled` status. A caught unwind in
advance stays `Evaluating`, never the between-token `Running` state, and cannot
resume; cancellation can still release that interrupted owner. After interruption,
coverage and the last numerical position may lag actual work counters.

The protocol/supervisor remains the authority. These APIs neither publish effects
nor authenticate models, establish detector accuracy, or change the fixed congress
membership, commitment scheme or missing-worker rules.

## Verification boundary

Twelve new tests use the original native decoder, exact linear monitor and byte
encoder with explicitly synthetic parameters. They cover one-token boundaries,
all four verdicts and input-dependent results, one-shot equivalence, stale pulls,
terminal polls, held answers/terminals, budget and context admission, strict output,
interleaved workers, cancellation during prefill/after an answer, live observation
withdrawal and caught post-token unwind. One compile-fail case prevents mistaking
progress for a verdict. The existing native evaluator/transport tests are retained.

The required RCH verifier could not start in this environment (`rch: command not
found`, exit 127). Rust compilation, rustfmt, Clippy and the Rust tests are
UNEXECUTED. Lexical/whitespace checks do not substitute for the full xtask gate.
No production qualification, runtime integration or bead closure is claimed.
