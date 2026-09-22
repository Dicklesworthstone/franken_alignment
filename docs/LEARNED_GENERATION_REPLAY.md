# Learned-generation checkpoint and replay baseline

This source addition implements the token-recomputation baseline of plan 11.1,
11.2 and 11.7 for the existing learned-audited generator. It does not introduce
another decoder, sampler, monitor, effect ledger or runtime.

`DecoderModel::replayable_monitored_generation` owns the original
`LearnedGeneration` together with its immutable construction recipe. Every live
advance and run-to-stop call delegates to that original owner. The recipe retains
the actual model and fitted codec, complete probe roster, residual-retention
policy, original prompt, stop IDs, sampler seed/policy and all budget ceilings.
It cannot be replaced by a matching integer ID, Debug output or caller-supplied
checkpoint arrays.

An accepted prefix can be captured with `checkpoint(CheckpointLimits)`. The
checkpoint retains original token IDs, sampled choices, exact sampler state,
logit words, canonical cache bytes, terminal/active status, numerical work and
aggregate telemetry work. A held or failed owner refuses checkpoint creation;
its shorter accepted prefix is not silently promoted into a resumable state.
An older checkpoint never changes a subsequently held/failed original owner.

`begin_replay(ReplayBudget)` preflights the complete saved prefix and constructs a
fresh original learned generator. `advance(n)` performs at most n token steps;
zero does not certify a nonempty prefix. There is no partial-owner accessor.
Every position again passes through the original learned compression,
source-check and complete K/V monitoring path. Samples are recomputed by the
original sampler, not forced from saved output IDs. Final verification compares
cache bytes, logit words, sampler state, all sampled fields (including probability
bits), status and every numerical/telemetry counter. An error or unwind latches
the reconstruction; no candidate escapes through `finish` before verification.

After verification, `finish` returns the original generator with its reconstructed
spent counters and unchanged original ceilings. Resuming does not refill source
checks, compression, probes or residual refinements. A saved EOS/token-limit stop
remains terminal. A probe alarm immediately following a saved prefix still holds
when that prefix is reconstructed. `ReplayReceipt` records newly incurred replay
work separately from the resumed run's historical spend.

## Bounds and nonclaims

The reconstruction has an explicit position and decoder/sampler product ceiling.
Telemetry remains bounded by the original per-token and aggregate caps. Logical
state-byte limits include the encoded cache, token/sample history, sampler state,
logits and metadata; they do not describe Vec capacity, allocator overhead, model
parameter retention or peak RSS. Verification temporarily needs a second bounded
state image. No speedup, allocation, wall-clock or trained-detector claim is made.

This is an IN-MEMORY typed numerical checkpoint, not a portable archive, durable
journal, direct KV-state restart or FileOversight recovery adapter. It preserves
its original capture stream and evaluation origin as replay lineage, not a new
independent/live observation. Replay cannot establish evidence freshness, undo an
external effect or restore a permit. Integrating this baseline into durable or
live control requires explicit compatibility and authority-boundary work.

## Authored verification

Seven integration tests cover every prompt/continuation/terminal cut, parity
with the original independent sampled engine, segmented/zero-step replay, exact
and one-less resource caps, conserved source-check spend, post-checkpoint alarms
and terminal stop preservation. One private unit test pairs successful replay
with ten corrupted expected-state fields and verifies failure latching. Four
compile-fail examples prevent mutable-owner escapes and checkpoint-to-permit
conversion. Existing assertions and algorithms are unchanged.

The required RCH test/gate commands could not run in the preparation environment:
`rch`, `cargo`, `rustc` and `rustfmt` are absent. These Rust tests, compilation,
formatting and Clippy are UNEXECUTED. No Bead, qualification or release is closed.
