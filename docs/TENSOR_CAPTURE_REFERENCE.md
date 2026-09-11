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
the declared tensor before presenting them.

## Verification and change record

Three kernel and six public capture tests are source only. They cover every
binary16 and bfloat16 word, small-domain overlap checks, all 24 physical axis
orders, padding, recycled storage, exact probe integration, truncation, selection,
nonfinite values, extent overflow and encoding mismatch. Rust compilation,
rustfmt, Clippy and tests have NOT run: no configured Rust/RCH runner is available.
No bead is closed and historical execution receipts do not qualify this source.

This is bounded reference progress toward FA-085/FA-024 and plan 10.6/10.8,
serving FI-A05 and FI-I05. Native-host comparison, asynchronous buffer ownership,
GPU cancellation and real serving-host qualification remain open. No dependency,
unsafe code, foreign numerical runtime or second rights ledger was added.
