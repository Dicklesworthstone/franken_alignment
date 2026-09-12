# Lossy KV baseline and numerical continuation

This is FA-028 reference progress, serving the baseline comparison in plan 10.5,
10.7 and 11.1-11.3. The consumer is experimental full-decoder continuation, not
live effect admission. A compact tensor is neither an exact restart nor a source
observation. Native-host qualification and measured serving performance are separate.

## Complete-layer int8 image

`model::quantized` adds an explicit symmetric signed-int8 baseline over an existing
`ModelKvImage`. Each token, stored cache head and K/V side gets its own exact f32
absolute peak followed by codes in -127..127. V1 computes the ratio in f64, rounds
ties away from zero, and reconstructs `peak * code / 127` in f64 before narrowing
to f32. The peak is stored rather than a narrowed scale, avoiding underflow of a
scale derived from a subnormal peak. Query heads do not duplicate stored KV heads.
Zero codes reconstruct positive zero; signed-zero changes and erased nonzero
signals are recorded. Extremal peaks remain finite. This is not a learned codec.

The entire descriptor, source-value ceiling and encoded-byte cost are checked
before reading scalars. No partially quantized image is returned. Two source
passes per group select the peak and compute codes/error measurements. Reports
separate source-format scalar bytes, full original image bytes, compact image
bytes including all metadata/scales, and per-layer K/V errors. Squared error and
maximum absolute error are rounded descriptive measurements, not certificates.
A tiny group may EXPAND after its scale and header are counted. Actual memory/RSS,
latency, allocation traffic and reconstruction fidelity require measurements.

A `QuantizedKvImage` owns only compact bytes and the original metadata descriptor;
it does not secretly retain full source arrays. Clones share those compact bytes.
Scalar inspection explicitly returns experimental values and cannot produce a
`SourceFrame`, `ModelKvImage`, trusted restore receipt or permission. The codec
supports all source scalar encodings already admitted by ModelKvImage; reported
source bytes respect that encoding rather than presuming every source was f32.

## Portable representation

The 40-byte header is domain `FAKVI8\0\x01`, big-endian u64 codec ID/generation,
u32 descriptor length, u32 reserved zero, and u64 compact body length. It is
followed by the ORIGINAL ModelKvDescriptor encoding. The body visits increasing
layer ID, absolute token position, K then V, and stored head order. Each group is
one big-endian f32 peak plus one byte per channel interpreted as signed two's
complement. Code -128 is reserved. Imports require finite nonnegative peaks
(including canonical positive zero), all-zero codes for zero peaks, and a
magnitude-127 code in each nonzero-peak group. All lengths, scales and codes are
validated before retaining the compact body; no floating arrays are allocated.

The intended codec and complete source descriptor are supplied independently.
Their equality is metadata binding, NOT payload authentication. Finite code or
peak changes can remain valid different data; a regression explicitly demonstrates
that limitation. Imported images never invent a reconstruction-error report or
historical capture receipts. The global body ceiling follows the existing
16,777,216-value bound and worst-case one-coordinate groups. Each operation can
impose tighter byte/value limits. These are per-call admission limits, not
conserved effect rights, durable lifetime budgets or wall-clock limits.

## Verification record

First increment adds ten public Rust test functions and one compile-fail example.
They cover literal wire bytes and rounding ties; subnormal/extreme values; rare
signal erasure and signed-zero loss; GQA grouping and absolute positions; all-layer
budget admission; mixed original precisions; all truncations of a selected image;
late malformed groups; valid unauthenticated numerical edits; empty/zero images;
and complete-image size reduction versus expansion. Existing source capture,
serialization, and restoration implementations are unchanged apart from the new
module declaration. Their existing assertions remain intact.

Rust/RCH compilation, execution, formatting, Clippy and the revision-bound project
gate remain pending. No Beads packet or production gate is closed. The fixtures
are numerical controls, not trained-model or safety evidence.

## Actual full-decoder continuation

`DecoderCheckpoint::quantized_experiment` binds the original immutable parameter
allocation, complete original token prefix, compact image and measured encoder
report. Its `QuantizedDecoder` does NOT retain the full original KV arrays or old
logits. The source session/checkpoint may be dropped. Each new continuation shares
only that immutable compact basis and owns separate full-precision suffix rows.
This is a frozen-prefix quantization experiment, not an incremental int8 serving
cache: newly computed suffix rows are deliberately not requantized.

`QuantizedDecoderSession` calls the original private `DecoderModel::forward_token`
with on-demand dequantization of prefix scalars. A private Continuation owner is
shared with the existing sparse-intervention session; there is no second forward
engine, attention evaluator, or competing mutable suffix implementation. The
existing intervention API, control arm, preflight order and compile-fail boundaries
are preserved. An admitted numerical failure publishes no partial suffix, tokens,
logits or successful-work count. Failed CPU work and allocations are not free.

Both modes deliberately lack a next-token choice until an explicit first token
is computed. The old checkpoint logits describe the unquantized predecessor and
are not installed as compressed-state results. Later greedy choices use each
session's own newly computed logits. Experimental steps cannot be converted into
ordinary DecoderSteps, source captures, exact checkpoints or production sessions.
The compact wire parser is not an alternate constructor of this source-derived
decoder experiment: valid unauthenticated imported payloads remain standalone
approximate numerical data.

## Paired behavioral comparison

`compare_quantized_forced` and `compare_quantized_greedy` preflight the entire
quantization byte/value allowance, every supplied original token, full context,
the COMBINED product budget for both full rollouts, and all retained logit vectors
before reading cache scalars or executing either arm. A budget for one arm cannot
admit two. Both arms share the same explicit first token. Teacher forcing keeps
subsequent inputs equal; greedy mode exposes feedback from each arm's own choices.
Failure returns no partial comparison. Previously returned objects stay unchanged.

Reports retain the baseline checkpoint, compact experiment, codec loss/byte report,
full logit vectors and original per-step numerical work. They separately identify
first differing logit words, first differing next-token choice, and first differing
ACTUALLY CONSUMED token. A last diagnostic choice may be outside the consumed
horizon, and a teacher-forced run need never consume it. L2/max logit distances are
rounded descriptive quantities, not probabilities or detection certificates.

The comparison object intentionally pins both baseline and compact state; its
memory is not the compact-image byte count. Cloning its QuantizedDecoder and
dropping the comparison releases that baseline retention. Decoder product counts
retain their original meaning and EXCLUDE dequantization, allocation, square roots,
report construction and other overhead. No throughput, zero-copy, total-memory,
trained-model equivalence or safety claim is made by the encoded-size comparison.

## Continuation tests and executed arithmetic cross-check

The second increment adds ten public continuation tests and two compile-fail
examples. A nontrivial 32-step test compares on-demand compact-prefix continuation
with an independently constructed explicit sparse edit of EVERY original scalar;
logits, original IDs, all newly appended KV words and work must agree. A separate
32-step zero-cache control matches uninterrupted original inference. Imported
SafeTensors parameters go through the same paired comparison. Other cases cover
parent loss, sibling isolation, empty/full contexts, stale positions, exact/one-under
budgets, combined-arm admission and late vocabulary overflow with no partial state.
The existing sparse-intervention and comparison tests are retained unchanged.

A synthetic uniform-attention example exposes the important negative: a large
cache coordinate shares a head with a small decision-relevant coordinate. Int8
erases the latter; full continuation changes the next greedy choice. Increasing
only that small coordinate supplies the nearby retained-signal control. Forced
one-step and multi-step greedy tests distinguish the unconsumed diagnostic choice
from its subsequent consumption. These are algorithmic counterexamples, not
examples of actual harmfulness or a trained detector's error rate.

`artifacts/execution/2026-09-12-kv-int8-arithmetic.py` was executed independently
using Python binary32 packing and scalar formulas: 2,052 synthetic groups and
130,633 values passed the declared arithmetic invariants. A separate closed-form
uniform-attention calculation yielded original next-token choice 1 versus compact
choice 0, with logits 0.24617820978164673 versus 0.0 for the deciding coordinate.
The result JSON binds the script and codec source SHA-256. Random finite bit
patterns are NOT a workload distribution, and their zeroing count is not a model
metric. This check validates neither the Rust implementation nor any serving host.

All twenty new Rust test functions and three compile-fail examples remain
UNCOMPILED and UNEXECUTED. The RCH test command failed before compilation because
`rch` is absent; no local compiler fallback or verification-gate change was used.
No Beads packet is closed. Actual performance, independent native continuation
qualification, wider codec baselines, learned compression and held-out functional
fidelity campaigns remain outstanding.
