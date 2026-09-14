# Trained low-rank KV reference codec

## Capability and plan binding

`activation::tensor::kv::model::learned` implements the PCA/linear-bottleneck arm requested by plan 10.5, on the existing complete `ModelKvImage` capture format. It complements, rather than replaces, raw/lossless state and the existing int8 baseline. It is experimental numerical compression, not a trained harmfulness detector, nonlinear sidecar, exact restart profile or effect authority.

`LearnedKvCodec::fit` learns a separate centered mean and rank-r basis for each layer, stored KV head and key/value side. GQA query heads do not duplicate stored cache groups. The complete training set is passed as an origin-keyed map and checked before scalar work: original profile/encoding/tap/model generations must agree, no source can be empty, at least two rows are required, and repeated source stream/batch identities refuse. Origin keys are explicit caller-supplied task/lineage identities, not authenticated labels. Train/test assignment before derivative generation remains the operator's obligation.

The implementation makes two passes over source coordinates, retains only a bounded group covariance/eigenvector workspace, and uses fixed-count cyclic symmetric Jacobi sweeps. The mean, covariance and rotations use binary64; emitted means and axes are binary32. A stable half-difference/hypot rotation avoids division by a tiny covariance off-diagonal. Rank, sweeps, group order, eigenvalue tie order and eigenvector sign selection are explicit. Remaining off-diagonal covariance and the emitted axes' orthogonality error are reported; bounded iteration is not a convergence certificate or a claim of optimal rank selection.

This is a reference-sized numerical algorithm in the already-admitted std-only package. It adds no executor, BLAS/FFI, dependency, model loader or alternate capture implementation. Optimized donor-backed numerical integration requires its own foundation admission; this implementation makes no such claim.

## Real compression and separate evaluation

`compress` computes latent coordinates using the emitted coefficients and scores reconstruction using the emitted binary32 latents and reconstruction. It does not report the unrounded optimizer's smaller error. `evaluate_held_out` accepts a separate origin and refuses training origins or overlapping source streams. Neither call updates the fitted coefficients. This protects declared finite source separation, not undisclosed near-duplicates or maliciously relabeled provenance.

A `LearnedKvImage` owns only the fitted codebook, latent vectors and original source/fit descriptors. It retains no original KV arrays or training arrays. Its `bits` method reconstructs an explicitly approximate scalar at the original layer, side, absolute position and stored head. It cannot be converted into a live capture, lossless image or permit.

Reports include changed scalar words, erased nonzero values, signed-zero changes, maximum absolute error and squared-error sums per group. Those are descriptive numerical losses, not certified bounds or evidence of decision preservation. A rare out-of-training-subspace signal can be lost while common data reconstructs well. That failure is a required control, not a reason to hide out-of-distribution rows.

The complete export has domain `FAKVLR\0\x01`, fixed big-endian header fields, every training origin plus original descriptor, the target descriptor, and the entire mean/basis/latent group inventory. `encode()` emits the bytes whose complete length the budget and report count. Codebook and provenance overhead are not amortized away; a short prefix or full-rank representation can expand. `original_scalar_bytes` refers to the original F16/BF16/F32 representation, not automatically four bytes per value. Export is not authenticated provenance. There is intentionally no import-to-fitted-codec conversion in this increment; parsing bytes cannot assert that this fitter executed.

## Resource and numerical contract

Training is bounded to 64 sources, 16,384 total rows, 4,096 groups, width 128, 32 Jacobi sweeps, 1,048,576 codebook scalars and the existing 16,777,216 input-value ceiling. The complete fixed fitting work and per-group scratch slot requirements are admitted before reading values. Encoding separately admits source values, complete representation bytes and its arithmetic work model before constructing output. Unsupported width/rank/profile combinations refuse instead of silently changing the requested policy.

Reported work units are conservative loop-cost admission, not wall-clock latency, total floating-point instructions, allocator traffic or peak RSS. Caller-held images, metadata, outputs and codebooks coexist with scratch. The bounds do not describe a production-throughput result. Nonfinite arithmetic or binary32 overflow returns no partial fitted codec or image and leaves sources unchanged. Ordinary allocator abort remains outside Result-level recovery.

## Added source scenarios and verification

Ten new Rust test functions cover off-axis rank-one learning, unseen points in its subspace, erasure of a rare coordinate with a full-rank permitted control, actual complete byte shrinkage versus metadata expansion, declared held-out separation without retuning, each exact/one-below budget, different learned head subspaces, the original mixed F16/BF16/F32 multilayer fixture, source-owner lifetimes, projection overflow, profile/rank refusal and deterministic constant-data encoding. One compile-fail boundary prevents conversion of a lossy image into a source image. The original capture and quantization tests and implementations are unchanged except for the module declaration.

A separate Python/NumPy arithmetic experiment executed 45 synthetic covariance cases at widths 2, 3, 4, 8 and 16 and scales 1e-30, 1 and 1e30. Its sequential Jacobi result was compared to an independent eigensolver; maximum relative eigenvalue discrepancy was about 7.1e-16 and maximum emitted-projector discrepancy about 6.8e-8. Its rare-signal example mapped [1,2,0.25] to [1,2,0], with squared error 0.0625. This checks the selected mathematical formulation, NOT the Rust source, byte export, capture APIs or continuation integration.

The required invocation `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_kv` was attempted and failed before compilation with `rch: command not found` (127). Rust compilation, formatting, Clippy, tests and revision-bound qualification remain unexecuted. No Beads task or production gate is closed. No trained serving model or independent safety label was evaluated.
