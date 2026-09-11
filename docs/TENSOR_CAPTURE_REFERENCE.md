# Checked host tensor capture

## Implemented scope

`activation::tensor` reads actual borrowed CPU byte slices into the existing
immutable `SourceFrame`. Logical axes are explicitly `[batch, token, cache_head,
channel]`; physical byte strides can permute those axes or include padding.
Only the selected token's head/channel coordinates are copied, in canonical
head-major order. The source survives subsequent reuse of the borrowed buffer.

The fixed contract binds capture profile, binary32/binary16/bfloat16 encoding,
byte order and head/channel counts. Half-width finite values expand exactly to
binary32; signed zero and subnormals survive, nonfinite selected values refuse.
Buffer identity/generation and source relationship are supplied host assertions,
not authenticated provenance or proof of current device execution.

Layout construction checks products, offsets and enclosing byte extents, and
proves disjoint element spans by sorting active physical strides. This sufficient
certificate accepts ordinary dense permutations and padded layouts; it is not
a complete solver for arbitrary interleaved strides. Uncertifiable layouts,
non-singleton broadcast strides, empty dimensions and overflow refuse. Singleton
strides may be zero. Strides are nonnegative byte counts. Byte-slice origins may
be unaligned because decoding uses bounded byte copies, not pointer casts.

The entire declared layout must fit the supplied backing slice, bounded at
1 GiB. Captures contain at most 65,536 values. Layout validation is rank-bounded;
value copying and conversion visit only the selected coordinates. Receipts state
bytes actually read and normalized bytes retained, separately from the backing
extent. This is not a measured peak-memory or throughput result. The conversion
uses a temporary f32 vector and the original SourceFrame's owned bit copy.

The existing progressive codec and exact probe consume these captures without
an alternate numerical implementation. This is not GPU synchronization, a
serving-host adapter, native Asupersync admission, authenticated capture, or a
complete model restart. The public view cannot accept device pointers or execute
callbacks. The caller must establish that host bytes are ready and correspond to
the declared tensor before presenting them. Only selected values are scanned for
finiteness; untouched coordinates are not represented as observed evidence.

## Required tensor ingress at the effect boundary

`enable_tensor_activation_tripwire` freezes the monitor, tensor contract, stream,
selected batch and lifetime capture quota before proposals. In this profile,
`record_activation` refuses arbitrary pre-flattened SourceFrame input; callers
must use `record_tensor_activation`. This is an API boundary, not protection
against a host that fabricates both its metadata and byte buffers.

The shared activation admission path checks actor/input revisions, the current
policy epoch, stage, deadline, last actor token position and monotone sequence.
An admitted recapture invalidates the old quiet result and empirical approval
BEFORE reading bytes. Truncation, nonfinite values, changed head/channel shape,
encoding mismatch or exhausted numerical capacity then leave that new basis held.
Errors after admission may advance input_revision; callers must inspect it before
retrying. Wrong batches, foreign/stale contexts and rolled-back buffer generations
refuse before admission. Invalid descriptors that cannot be constructed must be
reported through the existing explicit activation_unavailable operation.

Per-buffer generation floors never decrease, including after capture loss and
actor reset. The bounded map has at most one entry per admitted capture job, so
it inherits the original 512-capture ceiling. Equal generations remain usable:
the value is a supplied buffer incarnation, not an authenticated write counter.

A successful fresh capture does not restore congress approval. The action still
needs a fresh review against the new basis; when human authorization is enabled,
its old second key is stale too. The existing automatic reservation is not copied
or refunded by recapture. After a new valid review, an unchanged reserved action
can dispatch through the original permit. Actor advancement requires capture of
the new last-token row, not an old row with a relabelled source sequence.

The common positive-path check already covers congress application, authorization,
ordinary dispatch and human two-key dispatch. Tensor observations confer no
permission alone. Cancellation, fence installation and endpoint reconciliation
remain available during capture outages or quota exhaustion. Previously disclosed
stream text and charged unknown effects retain their original history.

`tensor_capture_receipt` exposes the layout/encoding/selection of the retained
numerical observation. It is historical metadata, not a currentness certificate,
and older DecisionArchive formats do not thereby acquire a tensor proof.

## Verification and change record

Three kernel, six public capture and eight oversight integration tests are source
only. They cover every binary16 and bfloat16 word, small-domain overlap checks,
all 24 physical axis orders, padding, recycled storage, exact probe integration,
truncation, selection, nonfinite values, extent overflow and encoding mismatch.
Oversight cases pair actual reference two-key publication with raw-ingress bypass
refusal, failed-recapture holds, same-dimension head/channel swaps, buffer-generation
rollback, actor advancement, stale human keys, preserved reservations, retained
disclosure and capacity exhaustion. Existing activation tests remain unchanged.

Rust compilation, rustfmt, Clippy and tests have NOT run: no configured Rust/RCH
runner is available. No bead is closed and historical execution receipts do not
qualify this source. First the checked capture/normalization path landed; then the
owning oversight ingress and its positive/negative integration cases.

This is bounded reference progress toward FA-085/FA-024 and plan 10.6/10.8,
serving FI-A05 and FI-I05. Native-host comparison, asynchronous buffer ownership,
GPU cancellation and real serving-host qualification remain open. No dependency,
unsafe code, foreign numerical runtime or second rights ledger was added.
