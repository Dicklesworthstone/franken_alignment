# Durable learned-K/V generation and effect-bound recovery

`FileOversight` now has an opt-in learned-generation path using the original
`OversightBroker::own_learned_generation` and `advance_hosted_learned` operations.
This connects learned observation and source-bound publication to the existing
exclusive journal owner. It serves plan sections 8, 10.15, 11.1–11.2, 11.7 and 16:
original computation, independently judged effects, and recovery that never
restores old permission. The founding roots are FI-A05, FI-A16 and FI-I03.

## Installation and operation

Construct `decoder::learned::FileLearnedConfig` from the actual independently
selected `DecoderModel`, `LearnedSourceConfig` and `LearnedDecoderBindingLimits`.
The existing learned-generation archive encoder binds all model, fitted codec,
probe, prompt, stop-token, sampler and budget inputs at the empty prefix. It is
not used to import or install state. The wrapper additionally binds the source's
monitor generation and cumulative evidence-retention limits.

Call `enable_learned_generation` before proposals or other actor-state work. It
owns a new original generator, synchronizes its actual actor cache/sampler and
enables the original final-publication guard. Ordinary and learned decoder modes
are mutually exclusive; no mode-switch, raw-token override, reseed, external
state adoption, mutable numerical owner or budget-refill operation exists.

`advance_learned_generation` persists a step intent, then computes and persists
its outcome. A fresh call advances the journal by two. For explicitly separated
operation, use `begin_learned_step`, followed by `complete_learned_step` with the
same actor revision and position. Both paths use the same original computation.
An accepted result escapes only after its completion replacement is acknowledged.
The outer error means no acknowledged completion; an inner numerical error means
that error and its resulting state were recorded. Held samples expose audit
information but not the candidate token or uncommitted random draw.

## Interrupted computation is not an older quiet prefix

The write-ahead intent is essential. A crash or failed completion write must not
let a supervisor forget that a numerical step was attempted and authorize from
the earlier quiet state. Pending intents block new proposal, review, approval,
dispatch and first publication. They survive recovery and explicit resume.
Only recomputing the same original step can complete an intent. There is no
cancellation operation that drops it and makes the old source eligible again.
Manual stopping and original receipt reconciliation remain possible. Missing
status does not refund a charged unknown effect.

A storage or encoding failure poisons the live owner; it does not return the
candidate result or fall back to the prior numerical state. Recovery reads the
actual old-or-new replacement. If only the intent exists, it remains pending.
If the outcome became visible but its acknowledgment was lost, replay verifies
that outcome instead. A replayed hold/error cannot be resumed into a quiet run.

## Independent configuration and exact replay

Use `open_with_learned_generation(directory, profile, &expected_config)`.
Parsing creates unbound configuration expectations, not a trusted runtime.
The independently supplied complete configuration must match before numerical
replay, cleanup, or recovery writes. Generic `open` cannot hydrate this profile.

Every recorded step reruns the original generator. Comparison material includes
numerical status and errors; all numerical/telemetry counters; accepted tokens,
sampler/sample bits, cache/logits; the separately synchronized actor copy; exact
source-checked learned bytes; all probe score limbs, refinement states and audit
outcomes. Descriptive compression-error aggregates are not qualified by this
witness. No saved tensor, verdict, permission, cost counter or actor state is
installed instead of recomputation.

The original authority recovery transition fences old dispatchers, withdraws
human keys, cancels only undispatched reservations and preserves charged unknown
liabilities. Numerical recovery is paused. A fresh clock and exact
`resume_learned_generation` predecessor are required before continuation; saved
time is not current time. Resume does not discard a pending intent or refill any
original allowance. A terminal token-limit/EOS result stays terminal.

`learned_generation_inspection` is supervisor-side acknowledged state and costs,
not an agent permit or detector qualification. The independently configured
`read_publication_with_learned_generation` is a historical RAM-only projection.
It exposes no writable owner, current evidence handle, old key or helper session.

## Cooperative recovery

`begin_open_with_learned_generation` reads and validates the complete canonical
journal and independently supplied recipe, then returns `FileLearnedRecovery`.
It holds the original exclusive storage lock but executes no journal event.
Only event counts and `FileLearnedRecoveryStatus` are exposed; no reconstructed
token, writable owner, reviewer or historical key is available during replay.

`advance(expected_events, max_events)` applies at most that many original
transitions to the same private machine. The predecessor must equal the completed
event count, and the quantum must be nonzero and within the original journal
event ceiling. Each event still performs its original validation, numerical work
and exact witness comparison. A failed or interrupted event permanently prevents
resuming that candidate; its first error and completed count remain inspectable.
No numerical or authority allowance is reset between quanta.

`Ready` means that the complete history and learned contract have been verified.
`finish` must still recheck the exact canonical bytes, perform original storage
cleanup and durably append the original recovery fence before returning an owner
and reviewer. The fence applies directly to the verified machine and uses the
existing candidate-persistence boundary. It does not replay the learned history
a second time. A private empty machine used by that persistence boundary performs
no learned inference and is never returned.

Dropping an unfinished recovery releases the lock without changing canonical or
staged storage. An early `finish` also refuses without cleanup. The successful
result has the same paused numerics, withdrawn old keys, charged unknown effects
and explicit clock/resume requirements as synchronous recovery. Existing
`open_with_learned_generation` now consumes this same recovery implementation.

This is a bounded event cursor toward FA-014 and plan sections 11.1–11.2 and
16.4–16.9. Initial reading/decoding, one original event, final encoding, the fence
and storage I/O remain synchronous. No wall-clock deadline, preemption,
constant-time recovery or hostile-writer exclusion is claimed.

## Compatibility, limits and nonclaims

The existing outer journal is unchanged. Decoder subtag 15 is the new learned
family, with enable/step/resume/intent subtags 0/1/2/3 and the
[source-bound sidecar family](DURABLE_LEARNED_SIDECARS.md) at subtag 4.
Old decoder tags 0–14 retain
their bytes and meaning; older readers reject the new family. Configuration
encoding reuses the original archive format. New witnesses are bounded at 32 MiB
and configuration at 16 MiB, but the original total journal byte/event limits
remain authoritative and can be smaller. New learned records conservatively
use the existing work-capacity class; they bypass no recovery reserve. Intent
admission leaves an event slot for completion, but does not guarantee the eventual
witness fits the remaining bytes. A completion-capacity failure stays pending.

Ordinary mutable transactions still replay the complete history, following the
existing journal owner. Cooperative recovery makes no measured speedup,
constant-time checkpoint, latency, RSS or hardware power-loss claim. Models and
monitor recipes retain their native bounds. Failed telemetry can include work
for which the original engine returned no complete report; absence is not zero.
Repeated reconstruction is the same lineage, not independent evidence.

The scope is an operator-controlled local journal-as-publication sink, not a
remote provider transaction, authenticated helper service, anti-rollback store,
Asupersync integration, qualified restart grade or production release. Numerical
quiet still requires original committee review, the mandatory human key and
current-source validation at publication. The
[durable learned sidecar extension](DURABLE_LEARNED_SIDECARS.md) connects original
source-bound helper inputs and their provenance to the same journal owner.

## Verification status and change record

The original durable-generation addition contains thirteen unit regression functions and one compile-fail
example. Tests pair successful original generation and two-key publication with
recovery pauses, interrupted intents, unknown-effect reconciliation, stale source
publication, exact/one-less event capacity, held/failed computations, differing
recipes, corrupt expectations and five existing storage fault barriers. They
use real numerical algorithms and real temporary journal files, with synthetic
probe parameters and helper ballots rather than trained-detector evidence.

Cooperative recovery adds seven unit tests, two text integration tests and two
compile-fail boundaries. They exercise single-event reconstruction at every
numerical cut and actual continued sampling; stale/invalid quanta; lock retention
and abandonment; a late corrupt witness; changed canonical bytes; pending intent
and charged unknown dispatch; all five original fence-write failure barriers;
exact capacity refusal with a successful one-more-event twin; and original
completed-text publication through both keys. These are authored test sources,
not executed results.

All new Rust tests, compilation, rustfmt and Clippy are UNEXECUTED in the current
preparation environment. Required targeted and full RCH commands cannot launch
because RCH is absent. The current checkout has no available Rust toolchain.
No historical execution receipt qualifies these changes; no roadmap or Bead is
closed. The existing release refusal, verification commands, dependency graph,
license and original numerical/authority algorithms are not weakened.
