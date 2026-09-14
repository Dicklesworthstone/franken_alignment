# Learned KV compression connected to original decoder continuation

## Implemented consumer

The learned codebook in `LEARNED_KV_REFERENCE.md` now feeds actual continuation through `DecoderCheckpoint::learned_experiment`, `LearnedDecoderSession`, and the original `Continuation`/`DecoderModel::forward_token` implementation. It is the plan 10.5 linear/PCA baseline connected to the real numerical reference engine, not a second decoder or a reconstruction-only demonstration.

Fit the codec on complete captured training images with explicit origin identities, then pass a separate evaluation origin and original checkpoint to `learned_experiment`. The model/cache profile, whole layer inventory, original stream/batch and prefix positions are checked. Reusing a training origin or source stream refuses. The returned plan retains the original immutable model parameters and token metadata, fitted codebooks, latents and descriptors, but neither the original KV arrays nor the old logit vector. A separate comparison intentionally retains the original baseline; its total memory must not be reported as only the compact image size.

The first continuation token must be explicitly supplied. The checkpoint's old logits were computed from uncompressed state and cannot be relabeled as compressed-state outputs. Every subsequent greedy choice comes from the newly computed logits. Newly appended suffix rows remain full precision, so this is frozen-prefix compression, not an endlessly quantized or retrained streaming cache. Source reconstruction uses the emitted F32 means, axes and latent coordinates from the fitted codec. Approximate values never become `SourceFrame`, a live `ModelKvCapture`, a restored exact checkpoint, or effect authority.

## Actual numerical consequences

The decoder executes the same embeddings, norms, rotary transformations, full-prefix attention, feed-forward layers and vocabulary projection as the original engine. Sparse scalar interventions, int8 prefixes and this learned prefix share the unchanged private suffix owner. Each branch owns its mutable suffix independently; advancing one cannot change the codebook or another branch.

`compare_learned_forced` consumes the same fixed suffix in both arms. `compare_learned_greedy` shares only the first explicit token, then consumes each arm's own greedy choices for the requested horizon. No fitting, rank tuning, policy calibration, EOS inference or early stopping occurs during evaluation. The comparison retains complete paired logit vectors and uses the existing contrast implementation. It reports the first changed logit vector, first changed next-token choice and first changed token actually consumed separately. A next choice beyond the evaluated horizon is only diagnostic, not an executed divergence.

A low-rank representation can reconstruct common training variation while losing a rare decision-relevant coordinate. The synthetic original-model regression trains on variation along one value-cache coordinate, then evaluates a distinct prefix containing the other coordinate. Its rank-one branch erases that component and changes the next-token choice from 1 to 0; its full-rank control preserves the choice. This is a numerical counterexample to using average reconstruction error as a behavioral guarantee, not evidence that token 1 or token 0 is intrinsically harmful.

## Admission and atomicity

The original decoder product budget and the additional reconstruction budget are separate. Full-prefix attention requests every old K/V coordinate per query head; resolving each coordinate costs rank multiply terms. The learned adapter computes the whole requested-horizon bound and also meters each actual reconstruction at runtime. Its bounded result counts successful inference-step reconstruction products, not arbitrary inspection reads, failed attempts, complete CPU instructions, transcendental costs or byte traffic. Skipping query-head duplication in storage does not make the repeated query-head reads free.

Each single-token advance preflights position, original decoder work, additional decomposition work and cumulative counter overflow. The original engine stages all numerical layers and final logits before publishing a suffix token. A failure at the vocabulary projection leaves token history, all cached rows, previous logits and successful-work counters unchanged. Nothing fallible follows the original numerical commit. The paired API admits the full combined decoder budget, extra reconstruction budget, all supplied token IDs, complete compression limits and retained logit capacity before running either continuation arm. Refusal never returns a partial comparison as success.

Representation size is the complete encoded learned image, including codebook and training/source metadata. It is not total resident memory. Reconstruction per scalar can cost more compute than reading an uncompressed cache; reporting that overhead separately is part of this baseline. No production throughput, measured memory savings, wall-clock cancellation, authenticated model/source provenance or complete safety qualification follows from these APIs.

## Source scenarios and verification status

Eight new Rust test functions extend the ten fitting/compression tests from the first increment. They compare every logit word and all old/new KV scalar words against explicit original scalar interventions for 32 teacher-forced and 32 greedy steps; pair rare-signal erasure with a full-rank control; distinguish diagnostic choices from consumed divergences; compare the raw control with uninterrupted original inference; trigger a late final-projection overflow; check exact/one-below resource bounds; drop training/checkpoint owners while continuing independent siblings; and reject stale positions, invalid tokens, training overlap and context overflow. Two new compile-fail examples reject live-session conversion and checkpoint export. Existing attention, inference, capture, suffix commit logic, and original tests are unchanged. The only change to existing comparison arithmetic is its private visibility for reuse.

An independent two-dimensional arithmetic calculation, using explicit F32 rounding at the specified boundaries, gave a control deciding logit of approximately 0.5411918163 and a rank-one deciding logit of 0, selecting 1 versus 0. The full-rank arithmetic control equals the original. This executed Python calculation checks the selected synthetic arithmetic, not Rust compilation or runtime behavior. It is additional to the first increment's 45 covariance/eigensolver comparisons and is not a trained-model experiment.

The combined required command was attempted:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test learned_kv --test learned_decoder
```

It failed before compilation with `rch: command not found`, exit 127. All newly added Rust code, the eighteen Rust test functions across both increments, three compile-fail examples, formatting, Clippy and revision-bound qualification remain unexecuted. No Beads packet or production gate was closed. No dependency, unsafe code, alternate executor or model implementation was added.
