# Progressive activation observations and enforced tripwires

## Capability and current qualification

The std-only reference workspace now performs actual finite-binary32 encoding,
source checking, progressive decoding and linear-probe computation. An optional
mandatory lane in `OversightBroker` consumes the results at the existing review,
authorization and dispatch boundaries, including two-key human dispatch. This
is not a new rights ledger or an always-success adapter.

Rust compilation, rustfmt, Clippy and Rust tests have **not** run in this session:
Cargo, rustc and RCH are unavailable. No original acceptance criterion or bead is
closed. Existing frozen-source receipts do not qualify this batch. A separate
Python arithmetic port was executed against Python's exact Fraction arithmetic;
it is evidence about the algorithm, not execution of this Rust implementation.

The numerical code is `action::consequence::activation`, with `probe` and
`monitor` submodules. Its location is the existing reference-control umbrella,
not a new production layer dependency: encoding is L1, checked observations L2,
probes L3, and the owning tripwire consumes them in L4/L5. The numerical modules
import no policy authority, permit, runtime, inference backend or effect adapter.

## Real progressive bytes, not an asserted reconstruction bound

A `SourceFrame` owns immutable supplied finite f32 bit patterns. Its identity
binds tenant, model, model generation, tap, layout generation, stream, sequence
and token position. NaNs, infinities, empty frames and frames above 65,536 values
refuse. Capture metadata and the source's relationship to the real model are
trusted host inputs, not authenticated facts established by these constructors.

Initial blocks retain all sign/exponent bits and a selected mantissa prefix.
Refinement blocks carry only newly disclosed mantissa bits. Both use an explicit
78-byte versioned header, big-endian identity fields, a bounded dimension count,
MSB-first packed bits and zero final-byte padding. The initial size is exactly
`78 + ceil(n * (9 + k) / 8)` bytes; a refinement from a to b mantissa bits costs
`78 + ceil(n * (b - a) / 8)`. Headers can outweigh compression for small vectors;
full refinement costs more bytes than a raw frame because of headers/padding.
No compression-ratio, throughput or network-latency claim is inferred from this.

The capture owner verifies each supplied block by re-encoding its actual source
and comparing every byte, including identity, length and padding. Only that
boundary constructs `VerifiedBlock`. A decoder cannot ingest unchecked bytes.
Fragments from another capture instance, even with identical numeric IDs, cannot
be combined. Refinements must start at the decoder's actual current precision;
duplicates, skipped predecessors and foreign frames refuse without changing it.
This process-local source binding is not a signature, authenticated transport,
content-addressed store, hostile-host defense or cross-process trust mechanism.

Every missing mantissa bit remains an interval, never an observed zero. Sign
reverses endpoint ordering for negative values. Subnormals and signed zero are
preserved. At all 23 mantissa bits, `exact_values` recovers every original finite
f32 bit; before that it refuses. This is exact frame recovery, **not** a complete
KV/RNG/host checkpoint or a restart guarantee.

Rust's binary32 representation and bit-preserving conversions are specified by
[the primary Rust documentation](https://doc.rust-lang.org/std/primitive.f32.html).
The codec uses safe `to_bits`/`from_bits`, not unsafe transmutation or FFI.

## Exact score intervals, including catastrophic cancellation

The probe arithmetic profile evaluates the mathematical expression
`sum(weight[i] * activation[i]) + bias - threshold` for the exact values of the
supplied binary32 coefficients. It does not emulate a GPU/BLAS reduction order,
float32 accumulation, or a model's native classifier implementation. Calibration
and registration must be appropriate to this fixed arithmetic profile.

Every binary32 product is an integer multiple of 2^-298. The implementation
maintains separate positive and negative nine-word unsigned accumulators and
subtracts them exactly at the end. At the declared dimension cap the largest
possible sum needs at most 570 bits, within the 576-bit storage. Checked carry
paths still refuse overflow. Scores expose canonical sign/magnitude words and
the explicit unit exponent, not rounded display values or confidence scores.

Each coordinate's source-containing interval is multiplied by its weight with
sign-aware endpoint selection. Exact accumulation yields lower and upper score
bounds. A strictly positive lower bound certifies Alarm; a strictly negative
upper bound certifies Quiet; a crossing requests refinement. Exact equality is
AtThreshold, never quiet. Certification establishes fidelity of this registered
linear decision only, not detection accuracy, harmlessness or deceptive intent.
The tests specifically retain a 2^-298 positive residual between cancelling
maximum-magnitude products, under all six term orders.

## Bounded refinement is consumed by the effect path

`RefinementMonitor` freezes up to sixteen probes, strictly increasing precision
levels ending at exact recovery, encoded-byte capacity and probe-coordinate
capacity. It checks the next block and probe pass against those capacities
before performing them. Previously certified quiet probes need not be rerun
when their source intervals shrink. Reports retain every actual step, exact
score interval, probe identity, encoded-byte count and coordinate count. The
encoder, source verifier's extra encoding pass and decoder are reported
separately from probe coordinate work; none is called elapsed CPU or wire time.
Alarm, exact threshold equality, unavailable refinement and budget exhaustion
cannot become NoAlarm by skipping a required probe or pretending a raw read ran.

`enable_activation_tripwire` freezes a monitor, capture stream and lifetime
capture cap before proposals. It has no disable or coefficient-edit operation.
Unconfigured brokers preserve their existing explicit bootstrap profile. In the
enabled profile every effect requires a current NoAlarm observation **in addition
to** the existing whole-input congress, exact policy, permit and optional human
key. NoAlarm never issues a permit or supplies an empirical approval itself.

`record_activation` checks expected input and actor revisions, the frozen model
profile and stream, tenant, action epoch/deadline, and exactly the last token
position of the current supplied full-prefix actor state. Sequences increase
across captures and resets. This is a metadata/ownership contract, not proof that
a real tap measured that position or that the caller disclosed every host input.

An admitted recapture advances the shared input-basis revision and clears prior
approval, even if helper text stayed identical. Old pending reviews and human
requests therefore cannot silently approve changed activation data. Once its
context is validated, capacity/computation failure intentionally leaves that
new basis held: the previous quiet result is removed before work begins. Callers
can inspect `input_revision` after an error. Bad foreign/stale contexts refuse
before admission. Explicit `activation_unavailable` handles capture failures
that prevented construction of a valid SourceFrame in the first place.

Actor advancement, changed helper inputs, policy/credibility changes and reset
invalidate reuse through the bound revisions/epoch. All positive paths recheck
it; the human second key has no bypass. Restrictive reviews, reservation
cancellation, fencing and endpoint reconciliation remain available during
activation outages or exhaustion. Unknown effects are never refunded by loss
of monitoring. At most 512 capture jobs and 1,048,576 cumulative retained exact
score words are admitted; these are reference work/retention bounds, not peak
memory measurements. Reset does not replenish them.

This is a fixed tripwire lane. It does not yet deliver sidecar tensors to an
actual LLM helper, train/calibrate probes, infer new signatures, or authenticate
capture. Numerical reports remain separately inspectable in memory; older
DecisionArchive/ObservedReceipt formats do not export their full source closure.
There is no claim of durable activation evidence or production gate admission.

## Tests and algorithm cross-check

The batch adds 25 Rust test functions: five codec, seven exact-probe, four
refinement-monitor and nine public effect-path tests. Positive publication is
paired with rare residual alarms, exact threshold equality, exhausted budgets,
source/stream/position mismatch, stale actor/input revisions, rejected old
reviews, human-key bypass attempts, reset and reconciliation during an outage.
Codec tests include manual byte vectors, every truncation and single-bit mutation
of a selected block, all 24 precisions, source mixing and exact cap boundaries.
These Rust tests are source only, not passed execution evidence.

The executed Python port used seed 731901, compared 384 finite dot products with
an independent Fraction oracle and checked 9,216 nested interval inclusions.
It also checked the 65,536-coordinate maximum and the smallest positive product
surviving huge cancellation. There were zero mismatches in that bounded check.
Its script/result are retained under artifacts/execution with an explicit
`NOT Rust execution` scope. It supplies no compiler, memory-safety, performance,
host-capture, detector-quality or whole-system correctness evidence.

This implements scoped numerical-baseline progress toward FA-028, FA-029 and
FA-030 and the numerical portion needed by FA-113, following plan 10.3-10.6,
10.15 and 14.9. Founding roots
are FI-A05 (intermediate activation oversight), FI-A06 (evidence-backed holds)
and the introspection essay's economical sidecar observation mechanism. The
original native-host, admitted foundation and held-out campaign obligations
remain mandatory and open.
