# Reconstructing a learned KV codec from its actual fitting inputs

## Capability

`LearnedKvCodec::fit_with_checkpoint` runs the existing bounded centered-covariance
and Jacobi fitter and returns its actual codec plus a `LearnedKvFitCheckpoint`.
The checkpoint intentionally retains the complete immutable training images,
origin keys, original policy and budgets, and comparison-only result bytes.
Ordinary `fit` still discards raw source arrays; its implementation is unchanged.

`checkpoint.replay(budget)` executes that same fitter on the retained inputs and
compares every emitted binary32 mean/basis word and every FitReport field before
returning a newly computed codec. Floating-point comparison is bit-exact,
including signed zero. There is no decoder from result bytes into GroupBasis or
FitReport, no imported coefficient fallback, and no separate optimizer.

The consumer is the original learned monitoring/sidecar path: the reconstructed
codec can be supplied to its existing `LearnedDecoderPolicy` or
`HostedSidecarRequest`, which still perform their own fresh source checking,
monitoring, budgets, helper review and effect admission. A checkpoint itself is
neither a live capture, restartable decoder, quiet monitor result nor permission.
This serves the replayability and sidecar-consumption contracts of plan 10.5,
10.15 and 11, without changing their semantics or founding-idea concordance.

Reconstruction repeats fitting. The returned codec's fit report describes that
fresh invocation; multiple replays incur multiple invocations. The new budget
may narrow the original caps but cannot enlarge them. No physical execution,
training cost or continuation budget is refunded or silently treated as free.
This is a correctness baseline, not a codebook-only fast load or a speedup claim.

Training image storage is intentionally bounded separately (96 MiB of complete
logical images); comparison material is capped at 16 MiB. These are not peak RSS,
allocator or I/O cost measurements. Source descriptors, task labels and train /
evaluation split assignments remain trusted declarations; equality does not
authenticate their provenance, prove independent samples or qualify a detector.

## Source and verification status — September 26, 2026

The implementation is `model/learned/replay.rs`; the parent gains only its module
declaration. Six tests in `tests/learned_fit_replay.rs` use the existing small
nonzero-attention decoder and actual fitter. They compare independent fitting,
exact parameters/report words and held-out compressed bytes; changed external
corpus maps; every replay allowance; retained source exclusions; invalid corpus
admission with a valid control; and repeated fresh work. One compile-fail example
keeps checkpoints distinct from fitted models. All prior tests are unchanged.

**UNEXECUTED.** Required targeted and full RCH commands stop before compilation
because `rch` is absent in this preparation environment (exit 127). Compilation,
Clippy, rustfmt and runtime tests have not run. Selected-source and patch checks
are not execution evidence. No roadmap or Beads item is closed, and no production
feature qualification follows from this source addition.

## Portable archive and actual consumer

`checkpoint.binding()` yields the intended policy, original caps and complete
training-source descriptors. `checkpoint.encode_archive(byte_limit)` writes a
new `FAKVFIT/1` input archive. Its complete source directory precedes all scalar
payloads, and a separate capped opaque witness holds the expected result words.
The binding is intended to be retained independently; it is not a signature or a
hash of the source payloads. Archive authenticity still belongs to the operator.
Changing both the training data and its resulting witness consistently can form
a different valid fit under the same metadata; no cryptographic guarantee is
claimed. An unchanged witness rejects a different recomputed result.

`LearnedKvFitArchive::decode(bytes, &binding, byte_limit)` returns an explicitly
unverified input object, not a fitted codec or the earlier checkpoint type. It
validates exact format/version, canonical source order, complete expected
inventory, original caps, descriptor sizes, row/value totals, complete lengths
and absence of a trailing tail before materializing any source scalars. Original
ModelKvImage decoding owns all per-layer scalar and descriptor interpretation.
Only `archive.replay(budget)` runs the actual fitter, compares all output words,
and returns the newly computed codec and a newly verified checkpoint.

`LearnedKvFitArchive::read(&mut file, &binding, &mut io_budget)` accepts an
operator-selected reader. It performs no path inference, model download, fitting
or live effect. One non-cloneable read budget accounts returned bytes and every
read attempt across failures/retries; interruptions and EOF probes consume calls.
At least one byte of remaining capacity is needed to attempt the EOF probe, even
though a successful EOF returns no bytes. I/O errors, missing tails and exhausted
budgets never expose a partial archive or a usable codec. No production runtime
or asynchronous executor is introduced.

The full archive cap is derived from the 96 MiB original-image cap, 16 MiB
comparison cap and at most 64 directory entries. These bound logical retained
inputs, not transient parser copies or peak memory. This is deliberately a
recomputation baseline rather than a fast parameter-only model import.

Six additional functions in `tests/learned_fit_archive.rs` cover exact independent
header bytes and size boundaries; all truncated prefixes and appended tails;
metadata substitutions; changed result words and independently recomputed changed
training data; shared reader budgets, interruptions and I/O errors; and a real
file roundtrip whose refitted codec drives the original learned monitor/sampler.
The latter compares actual continuation tokens, random state, cache words and
work against the original fit, and pairs it with a fresh monitor that holds the
same model. Successful fit replay never substitutes for that new monitor verdict.
These six functions and the earlier six remain **UNEXECUTED**, as do both
compile-fail examples. There is no service/CLI or durable effect-journal integration
in this change, and the earlier codebook-only export still has no import path.
