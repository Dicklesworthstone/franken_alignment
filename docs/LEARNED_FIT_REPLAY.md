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
