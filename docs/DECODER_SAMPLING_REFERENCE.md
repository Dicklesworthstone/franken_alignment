# Stochastic decoder sampling and continuation

## Numerical selection contract

`activation::tensor::kv::decoder::sampling` supplies the missing stochastic selector for original-token continuation (FA-025/FA-026/FA-027, plan 11.1-11.3). Its consumer is the existing numerical decoder, not an effect gate, live audit selector or source of authenticated randomness. This is L7 reference computation; native host qualification remains separate.

SamplingPolicy freezes ID, generation, vocabulary, temperature, top-k and top-p. Temperatures are finite in [1e-6, 1e6]; top-p is finite and strictly positive through one. top-k zero explicitly means the whole vocabulary. Temperature zero is refused rather than silently selecting greedy behavior. The original deterministic greedy API remains separate. No EOS, padding suppression, repetition penalty, forbidden-token mask or tokenizer behavior is inferred.

All original logits must be finite, including those top-k would discard. Bounds and exact vocabulary length are checked before scanning/sorting. Candidates sort by descending numerical logit with lower original token ID breaking ties, including signed-zero ties. After top-k truncation, stable softmax subtracts the maximum BEFORE temperature scaling. Nucleus truncation keeps the shortest descending prefix whose unnormalized cumulative mass reaches top-p times the top-k mass. top-p one retains the entire top-k set, including any rounded-zero tail. Sampling renormalizes only the retained set. Boundary equality keeps the candidate that reaches the threshold; this order is part of V1, not inferred framework parity.

Arithmetic is sequential binary64 with platform exp; probabilities and cutoff decisions are rounded numerical results, not directed-error certificates. Underflowed exponentials are explicitly counted, not described as missing logits. The random uniform uses the upper 53 bits of one PRNG word on [0,1), then a strict cumulative comparison. Rounding at the upper endpoint falls back to the last strictly positive candidate, never a zero-mass tail. This finite-grid implementation does not claim ideal-real exact sampling or cross-platform bit identity.

## Complete random state

V1 uses the public-domain xoshiro256** transition with four SplitMix64 outputs expanding the supplied u64 seed. The algorithm sources are [Blackman/Vigna xoshiro256**](https://prng.di.unimi.it/xoshiro256starstar.c) and [Vigna SplitMix64](https://prng.di.unimi.it/splitmix64.c). Every successful sample consumes exactly one word, even with singleton support; there is no variable rejection loop. Stream identity is an explicit positive label, not a mathematical independent-stream guarantee. Distinct labels with the same seed intentionally have the same draws. This generator is NOT cryptographic and must not select secret audits, nonces, commitments or production security decisions.

SamplerSnapshot retains policy, stream label, successful draw count and all four state words. Its fixed 96-byte version-one format uses an eight-byte domain, big-endian policy ID/generation, u32 vocabulary/top-k, then binary64 temperature/top-p bits, stream, draw count and state words. Parsing checks exact length/domain, the independently supplied full expected policy, positive stream and nonzero generator state. Truncation, suffix bytes and an incompatible policy refuse. Counter exhaustion cannot wrap. The format exposes the entire random state and is neither authenticated nor anti-rollback protected; a valid altered state can decode as different replay data. It is not itself a numerical decoder checkpoint or authority capability.

The standalone sampler prepares all fallible distribution computation and the next state before publishing its draw. A failed score, budget or counter check leaves it unchanged. Explicit snapshot restoration replays numerical state, never resource rights. There is no mutable-state clone method or hidden seed replacement.

## Bounds and costs

Vocabulary is bounded by the original 65,536-entry decoder limit. A SamplingBudget supplies a smaller per-call vocabulary ceiling; it does not renew production rights or account all inference. Sorting is O(V log V) with one bounded candidate vector; only the top-k subset needs exponentials. SamplingWork reports scanned logits, exponentials, retained candidates and zero exponentials before nucleus truncation. It excludes comparisons, allocations, allocator overhead and the decoder's separately measured product terms. Failed work may consume computation and allocate temporary space; no free-work, cancellation or latency guarantee is implied.

## Change and verification record

First increment: the kernel, exact sampler-state framing and thirteen test functions (twelve public plus one internal endpoint/grid-boundary test). Tests include literal seed-zero PRNG words and initial state, independently calculated uniform nucleus cardinalities, two-token temperature probabilities, exact top-k/top-p ties, signed zero, extreme finite logits, underflow, all truncations, every policy-header byte substitution, zero-state refusal, exact full-vocabulary limits, counter exhaustion and no-draw refusal. Literal PRNG words were calculated separately with Python modular integers; that calculation is not Rust execution or a proof of statistical quality.

All Rust sources/tests remain UNCOMPILED and UNEXECUTED. This session has no configured Cargo/rustc/RCH verification path; rustfmt, Clippy and the revision-bound repository gate remain pending. No Beads task or production gate is closed. No dependency, alternate executor, existing assertion or gate was changed.
