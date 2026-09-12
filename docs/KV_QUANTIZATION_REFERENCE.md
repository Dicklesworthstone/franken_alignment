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
