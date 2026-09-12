# Scoped KV interventions that continue the original decoder

This reference increment connects the existing sparse KV experiment engine to
actual full-decoder inference (plan sections 11.3, 11.7 and 12.2). It is not a
single-layer fixed-query attention comparison. New query/key/value projections,
all attention layers, residuals, feed-forward blocks, final normalization and
vocabulary logits use the SAME private numerical forward function as the
ordinary DecoderSession. Its architecture and parameter/context limits remain
unchanged. No second inference backend or dependency is introduced.

## Frozen intervention and independent continuations

DecoderCheckpoint::intervene takes an experiment ID, a per-layer map of
DecoderLayerIntervention (explicit position/side scope and exact expected-bit
edits), and a maximum proposed-edit count. Only a checkpoint created by the
original decoder can enter this path. Its exact immutable model object and KV
arrays remain pinned. IDs do not authenticate external weights or host identity.

Every layer is admitted through the existing KvExperiment before any plan is
returned. That engine checks source bounds, expected bits, duplicate cells,
finite exactly representable replacements and the frozen edit scope. Admission
is all-or-nothing across the model. All proposed records, including no-ops,
count against an aggregate limit of 65,536 and the existing 1,024-per-layer cap.
The plan retains both the specification and effective-change count. It never
mutates the original checkpoint or another continuation.

DecoderIntervention::session selects a Control or Intervention arm. Both use the
same pinned model and prefix; only the selected treatment substitutes scoped
historical KV scalars. New per-token KV rows are private, owned numerical data,
not recaptured actor observations. Sessions share the original prefix and sparse
plan; neither copies the entire prefix nor shares a mutable continuation cache.

## What the intervention means

These are POST-checkpoint KV interventions. They do not recompute past hidden
states or reinterpret the logits stored in the checkpoint. Both experimental
arms start without logits. The caller must consume an explicit first original
token ID before reading newly computed logits or invoking advance_greedy.
For a paired test, use the same first token in both arms. A common teacher-forced
suffix isolates responses to identical subsequent inputs; independent greedy
suffixes instead measure the ensuing rollout, including token-feedback effects.
This is not a claim to have changed the model's earlier computation retroactively.

Every advance checks expected position, token ID, remaining context and the full
single-step product budget before inference. All layers finish before publishing
any new cache row, token, logits or successful-work counter. On a Result-level
failure, including numerical overflow, the prior numerical state is unchanged.
Allocator aborts are not recoverable transactions. DecoderWork counts successful
matrix/attention work, not failed attempted work, wall time, memory usage, sparse
lookup comparisons or asynchronous cancellation. Reusing a per-call DecoderBudget
does not provide a cumulative experiment allowance.

## Type and provenance boundary

DecoderExperimentSession and DecoderExperimentStep do not convert into original
DecoderSession, DecoderCheckpoint, DecoderStep, ModelKvImage, TensorCapture,
SourceFrame or permission types. Scalar inspection and logits are explicitly
experimental data. source() returns only the ORIGINAL unedited checkpoint.
The shared core uses a private temporary query capture to enter checked attention;
that object never escapes an experimental execution, no residual observation is
captured, and no live ModelKvCapture is populated. Ordinary sessions still publish
through their existing atomic all-layer capture path and retain their API.

## Regression source and execution status

Eight new tests cover 32-step bitwise no-op/control equivalence with original
inference (including every KV scalar), explicit first-token semantics, an
analytically checkable uniform-attention value intervention, all-layer refusal,
budget/context/stale-position atomicity, owner lifetime and independent suffixes,
empty prefixes, and numerical-overflow atomicity. Three compile-fail examples
assert that experimental results cannot be relabeled as original state/capture.
Existing decoder, checkpoint and imported-weight tests remain unchanged.

The prescribed RCH command was attempted but rch is not installed in the editing
environment. Rust compilation/tests have NOT run and this source is NOT qualified
by prior execution receipts. No local compiler fallback, bead closure, trained
model quality, production authority, causal-necessity or deployment claim is made.
