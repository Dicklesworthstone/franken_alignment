# Incremental progress for frozen residual-generation requests

## Capability and scope

The residual-monitor path can now yield after one original token and durably acknowledge each such boundary. This extends the hosted generation in `HOSTED_RESIDUAL_GENERATION.md` and the persistent intent/receipt path in `DURABLE_MONITORED_GENERATION.md`. It does not change the separate learned-KV generator, add a decoder or sampler, publish external effects, or introduce an actor-facing command. Compilation, tests, doctests, formatting and Clippy for these changes remain unexecuted in this development environment; no acceptance or production gate is closed.

The numerical `MonitoredSampledDecoder::into_generation` consumes its execution owner into `GenerationSession`. Its request, full-prompt admission and cumulative budgets are fixed before the first token. `advance(expected_position)` computes at most one original monitored step; `run_to_stop` drives the same step reducer. An unfinished session cannot return its decoder, including through a dropped or forgotten mutable borrow: it owns the decoder rather than borrowing one. `into_parts` is available only after termination, and a returned held/failed decoder keeps its original latch. Admission errors consume the supplied owner; the existing borrowed `generate` API remains available for one-shot use.

`GenerationProgress` explicitly separates partial observations from a terminal `GenerationReport`. A partial progress value has no finish classification or completed report. Token IDs are only previously released, completely reviewed continuations; no held candidate or sampler word is exposed. The original one-shot driver uses the same new private cursor, rather than keeping a second generation algorithm.

## Persistent execution

First commit the existing `begin_decoder_generation(journal_revision, command)`. Then call `advance_decoder_generation(current_journal_revision, command.id(), expected_generation_revision)` until `is_complete()` is true. The request-local generation revision starts at zero and counts acknowledged progress transitions, including one that terminates without a new token. It is not the decoder position, journal revision or physical computation count.

Each new progress call replays the canonical history, advances the original private cursor through the original hosted single-token operation, records its exact result/review/state comparison, and uses the existing Store replacement boundary. Candidate output never escapes before acknowledgment. Budget exhaustion, stops and native failures become the original request's terminal receipt; a held token invokes the existing configured containment policy. The intent's logical history reservation is consumed once, not once per token.

An older generation revision returns the current retained progress without inference or a journal write; a future revision refuses. Consumers identify already-consumed output by request ID and output offset rather than append the returned cumulative prefix blindly. Terminal receipt retries are also read-only. These are supervisor-side observations, not external-output delivery or effect permits.

Once a request has acknowledged incremental progress, its old whole-request `generate_decoder` route refuses. Otherwise it would reinterpret an obsolete numerical predecessor and could discard an unfinished supplied prompt. Continue with the incremental API. Unchanged intents that have not made incremental progress can still use the existing compact batch route. Completed receipts remain available through the original lookup APIs.

## Recovery and restriction

A recovery fence retains the original command, acknowledged cursor progress, sampled draws, cumulative admission work and original model state. It pauses inference and withdraws old authority as before. After a fresh trusted clock observation and explicit `resume_decoder`, the next progress call continues from the last acknowledged token rather than treating the original prompt or remaining suffix as a new request.

The pending-intent admission gate remains in force throughout. Alternate forced tokens, new proposals, changed commands, and checkpoint capture/reset cannot bypass the unfinished request. Manual suspension, source withdrawal, clock observation and reconciliation of existing obligations retain their original behavior. Suspension can leave the request permanently pending; there is no new cancellation-to-refund or reset-to-reroll path.

A failed result replacement poisons the live owner. `read_decoder_generation_progress` can inspect the complete canonical image without its writer lock, cleanup, a fresh-clock assertion or issuing a role. It pins the exact independently supplied decoder configuration before replay and distinguishes initial pending input, partial progress, and a recorded terminal result. After a lost directory-sync acknowledgment the newer progress can be readable even though the failed live owner still exposes no candidate result. Reading that image does not claim the failed sync succeeded.

Recovery still physically re-executes historical numerical operations to compare their witnesses. A crash can also require repeating the unacknowledged latest step. No exactly-once CPU execution, complete accounting of abandoned physical work, provider authentication, statistical detector quality or serving performance is claimed.

## Costs, limits and compatibility

The existing 4,096-step request, 128-request history, 65,536-declared-step history and fixed monitor limits remain. Before the first incremental step, admission checks room for every declared progress event. An undersized incremental journal refuses without computing a new token; the same untouched intent may still fit the compact batch path. This forecast is not a reservation against intervening supervisor operations. Actual byte limits and configured recovery reserves continue to be checked by the original canonical encoder at every cut, so later storage exhaustion remains possible and never erases prior progress.

Each optional progress cut carries full numerical-state comparison material. This uses more journal bytes and replay work than the compact whole-request transcript, in exchange for resumable boundaries and earlier acknowledged observations. It is not a delta-cache codec or a throughput optimization. No cursor, budget counter, verdict or cache is decoded as live state; all are reconstructed from the original operations.

Numerical event tags 0 through 7 and their byte layouts remain unchanged. Tag 8 adds a request ID, expected request-local revision and bounded nonempty comparison witness. Its blob length uses the original journal's u32 count encoding. Semantic replay rejects altered witnesses, reordered/duplicated progress or a progress event for another intent.

## Verification targets

Seven new numerical-session tests cover segmentation against the original stepwise sampler, partial/terminal distinctions, stale calls and terminal polling, cumulative/full-prompt budgets, stop-token review, held draws, post-computation failures, and unfinished-owner extraction. Thirteen new durable tests cover parity, recovery without budget reset, idempotent progress, alternate-route refusal, stop/containment behavior, all five Store failure barriers on quiet and alarming steps, witness/key corruption, the independent wire vector, whole-mode event admission, intervening control work, native prompt refusals and whole-image inspection. These tests are added source, not passing execution evidence. Two new compile-fail examples cover mutable-owner bypass and progress-to-permit conversion.

The required revision-bound verifier remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
