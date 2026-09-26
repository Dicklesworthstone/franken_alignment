# Incremental exact-KV restart

## Capability and consumer

`MonitoredKvCheckpoint::begin_incremental_restart` is the bounded continuation
of the original learned decoder's exact-KV restart path (plan sections 10.1,
10.3, 11.1 and 11.7). A policy configured for one position's full K/V tap roster
can now re-audit a longer accepted checkpoint. The old `begin_restart` remains
an all-at-once API with its original full-prefix limits and behavior.

The sealed checkpoint, actual model, frozen codec/probes, original evaluation
origin and residual-retention policy cannot be replaced. The destination first
passes the original monitored-session admission, including training-split and
stream checks, and remains inaccessible until verification and restoration both
succeed. This is an in-memory numerical API, not persisted service recovery,
containment reset, a new command, or an external-effect authorization path.

## Ordered complete verification

Each `advance(expected_position)` re-audits exactly the next ORIGINAL absolute
position across ALL registered layers and both K/V sides. It reconstructs a
one-position source image from exact retained f32 words through the original
all-layer capture path. It does not run the decoder forward pass, substitute
learned reconstructions for original values, re-tokenize, or scan/copy the
entire prefix for each position. The new audit then runs the original held-out
compression, source checking and complete learned monitor roster.

One-position descriptors retain original stream, absolute position and sequence.
Their revision 1 names this derived recapture, not the original full-prefix
snapshot revision. `source()` and the final restoration receipt retain that
original full descriptor. Derived captures do not create independent evidence
or a new training/evaluation origin. All residual-retention modes apply at every
original absolute position, exactly as during incremental generation.

A stale, skipped or duplicate position cannot move the frontier or spend an
audit call. Once an admitted call starts, the non-cloneable verifier is poisoned
before any source work. A non-quiet report permanently holds it; a preparation
error or caught unwind permanently fails it. Neither can be retried with a
larger budget. `finish` consumes the verifier and returns no session unless every
position has a fresh complete quiet audit. Empty checkpoints have zero audit
calls, not a fabricated quiet report. Dropping a partial verifier aborts it.

After verification the SAME original exact all-layer restorer writes and
recaptures the full KV state. Subsequent tokens still execute the original
learned publication guard. An earlier checkpoint never clears a later hold on
the source owner. The original authority/control journal is not copied or reset.

## Cost contract

`IncrementalRestartBudget` caps the complete retained cache, the number of
positions and each position's audit allowance. Whole-prefix position/cache
bounds are checked before work. Per-position allowances are intersected with
the original frozen policy. `reservation()` reports their checked product with
the EXACT prefix length in all eleven compression/checking/monitoring dimensions;
this is a conservative whole-verifier allowance, not measured work. It never
shrinks or renews, and no extra position can be admitted after completion.

`work()` separately counts attempts, reports, quiet positions, successfully
recaptured source values and costs in completed reports, including non-quiet
reports. Failed preparation can have bounded unreported work; it does not get
a successful report or a retry. Fresh restart cost remains separate from
historical inference. The final receipt records both cumulative audit work and
original exact restoration bytes. It makes no historical-inference execution
claim and reports no sampled draws.

The verifier retains at most its last audit in addition to the immutable source
checkpoint. Callers can explicitly retain returned audit objects; their memory
is not silently counted as free. The final exact writer still stages the complete
cache, and each audit pays codebook/metadata costs again. No constant-time,
peak-RSS, global experiment escrow, wall-clock cancellation or measured speedup
is promised. Pausing is only between bounded calls, not interruption within one.

## Complete sampled-generation continuation

`GenerationKvCheckpoint::begin_incremental_restart` uses this same ordered
verifier for the complete original `LearnedGeneration`. No generation, sampler
or pending output becomes accessible while a prefix is partially audited. The
full continuation sample capacity is reserved before verification starts. On
success both all-at-once and incremental restart use one private composition
seam, retaining exact RNG state, sample history, prompt position, stop rules,
horizon, numerical estimate and all original spent numerical/telemetry budgets.
No RNG word is drawn by audit or restoration. Old events are not republished.

Verification can pause while the original generation continues independently;
the sealed checkpoint does not change. A completed EOS/token-limit checkpoint
stays terminal, and a later hold on the source owner is not cleared. Fresh audit
cost is reported separately from historical generation spend. In particular,
restoring a generation whose lifetime source-check allowance is exhausted does
not authorize another token, even after successful separately paid re-auditing.
Repeated restarts preserve the same original generation ceilings and draw count.

Six additional integration test functions compare exact RNG words, probability
bits, logits, cache words and cumulative accounting across all empty/prompt/
sampled/terminal cuts. They also cover repeated restarts, a source advancing while
verification pauses, exhausted lifetime telemetry with uncommitted failed draws,
EOS, later held sources and failed/partial verification. An additional compile-
fail example rejects mutable generation access. The first six tests and the
previous all-at-once tests remain unchanged. All twelve new integration tests
and both new compile-fail examples remain unexecuted pending the RCH gate.

## Verification status

Six new integration test functions use the actual nonzero-attention decoder,
fitted codec and source-checked monitors. They cover a six-position prefix under
a one-position row cap, every incomplete cut, skipped/duplicate calls, exact
source-check limits, a causal second-position refinement hold after an earlier
quiet position, empty checkpoints, training-stream exclusion and later source
holds. Successful continuation compares every cache word and logit bit with the
uninterrupted and independently recomputed original. A no-history control checks
that the fixture actually depends on retained KV. No earlier test is weakened.

Tests are authored, not executed. The fresh RCH invocation stopped before
compilation because `rch` is unavailable (exit 127); local Rust compilation is
not a permitted fallback. Source/hash checks are not runtime evidence. No Bead,
roadmap packet, restart grade, detector capability or production gate is closed.
