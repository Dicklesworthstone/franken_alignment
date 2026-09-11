# Causal attention replay from captured queries and KV images

## Implemented consumer

`KvImage::replay_attention` computes scaled dot-product attention over the existing owned cache values and an actual `TensorCapture` of the query. It calculates every visible score, stable softmax weight and weighted value output. It does not return an asserted result supplied by a callback. The returned evidence retains the full immutable query capture, exact cache descriptor, frozen attention contract, numerical outputs and work description.

The contract binds Q and K/V tensor profiles, model generation, head/channel semantics, scale and causal window. It uses the existing GQA/MQA query-to-cache-head mapping without duplicating stored K/V arrays. Query channels must match key channels; value channels may differ. Q, K and V have distinct registered taps. Model, stream, source batch, absolute query position and token sequence must agree. Matching dimensions alone are insufficient.

This is a reference attention computation, not a complete transformer or serving-host adapter. The input Q and K must already have their host's positional transforms applied. This fixed profile includes no additive attention bias, arbitrary/padding masks, dropout, output projection, residual connection, feed-forward network or sampling. It cannot advertise support for a host requiring those operations. Caller-supplied provenance, capture readiness and host-profile qualification remain assumptions.

## Causal coverage, not invented zeros

FullPrefix requires every cache position from zero through the query position. Sliding explicitly registers a positive window width and requires every position in that calculated window. A captured suffix does not silently become the full context. Missing coverage refuses before numerical work; no absent row becomes a zero key, zero value or masked placeholder.

An image may retain later positions, but a query never reads any position after its own. The numerical result lists its exact first position and count. An empty cache or unavailable query position refuses. Sliding-window support does not infer a model's actual attention policy from which data happens to be available.

## Numerical meaning

The named profile is Binary64SequentialStableSoftmaxV1: normalized finite binary32 values are promoted to binary64, dot products accumulate by ascending channel, softmax subtracts the row maximum, and weighted value sums accumulate by ascending position. Outputs stay binary64; there is no silent cast back to the host's scalar representation. The positive scale is bounded at 1,000,000. The row containing the maximum supplies exp(0), so a valid nonempty window cannot have a zero softmax denominator.

This is NOT the exact integer linear-probe oracle. Floating sums can lose residuals, exp and normalization can underflow small weights, and transcendental/rounding behavior is not promised identical across targets. The result counts normalized zero weights explicitly. No certified interval, host-logit equivalence, exact_restart grade, calibrated detector decision or future safety follows from this computation. All source coordinates remain actual supplied finite data even when a weight numerically becomes zero.

## Costs and bounds

At most 128 query heads, 4,096 visible positions and 65,536 output coordinates are admitted. The source images retain their original total-value bounds. Before output allocation or scalar reads, the computation checks the whole selected problem against the supplied budget and fixed ceilings: 134,217,728 scalar product terms, 4,294,967,296 source-resolution steps and 16 MiB of numerical workspace.

Work records count dot-product terms plus weighted-value terms, repeated GQA accesses, softmax exponentials, a conservative base/node resolution-visit bound, and f64 weight/output storage. They do not count all instructions, BTreeMap comparisons, immutable source storage, metadata, allocator capacity or caller-held copies. The weight table is retained in head-major order to support downstream numerical investigation. No N-by-N full-sequence attention matrix is built: this replays one supplied query token across its heads.

Budget exhaustion returns no partially computed result and changes no capture frontier, branch history, actor state, effect ledger or permit. Budgets are per call, not a lifetime research-resource ledger. Allocator aborts and OS crashes are not recoverable transactions in this profile. No measured throughput, peak memory, native inference cost or compression ratio is claimed.

## Paired nonlinear KV experiments

`KvBranch::compare_attention` runs the same kernel twice on a control branch and an intervened branch, holding the exact query capture, attention contract and causal window fixed. Both branches must share the identical pinned base instance through the existing private identity check; equal descriptors or numerical experiment IDs cannot substitute a foreign control. The method does not re-encode edited cache values into exported captured SourceFrames.

Key interventions can change the softmax weights and downstream outputs. Value-only interventions keep weights unchanged while changing the mixture. Future rows and rows outside an explicitly declared sliding window have no effect. An unchanged control or an exactly undone intervention recovers the baseline numerical calculation. A rebase preserves those results while reducing the accounted lookup depth; it does not erase historical edits or replenish their quota.

The combined products, source-resolution bounds and retained workspace of BOTH legs are checked before either calculation starts. A budget sufficient for one leg is not sufficient for the pair. Sparse coordinate resolution uses the original branch lookup and does not copy a full edited cache. Source-resolution bounds charge the base lookup plus at most depth+1 parent-node visits per scalar; ordered-map comparisons and metadata accesses are not falsely reported as measured CPU cost.

`KvAttentionComparison` records both branch bases, the actual query, contract, complete reference and candidate outputs/weights, combined work and numerical output-difference accessors. It cannot become a Permit. These are local, fixed-Q numerical interventions: upstream Q changes, on-manifold validity, subsequent layers, output logits, token choices and long-horizon generation are NOT simulated. A change in attention values is not a demonstrated change in real model behavior or a proof of unique causation.

The public restoration regression also stages both branches through the existing paired CPU writer, commits their actual K/V bytes, recaptures those buffers through the checked tensor API and recomputes attention from the resulting images. Both results must equal the sparse-branch calculations while the original captured image remains unchanged. Those recaptured edited bytes are explicitly synthetic fixture data, not falsely attributed outputs of a model.

## Verification and work-packet relationship

The first implementation increment adds ten public integration tests using actual tensor capture and image serialization APIs. Positive cases include analytic two-token attention, uniform means, GQA/MQA, causal-prefix reuse, explicit sliding windows, large stable scores and portable-image replay. Negative twins cover missing prefixes, future-token exclusion, foreign same-shaped queries, batch/stream/sequence mismatch, invalid masks/scales/head contracts and every one-under work budget.

The second implementation increment adds nine paired-experiment tests, one paired-restoration/recapture test and one compile-fail permission-conversion doctest. Cases cover key versus value interventions, unchanged/undone controls, group-local changes, causal masking, equivalent rebases with different work, combined budgets, foreign bases and pinned data surviving dropped owners. No test fabricates a successful capture, writer, softmax or model-inference callback.

These twenty Rust test functions and the doctest have not been compiled or executed in this session: there is no configured Rust/RCH runner. Formatting, Clippy and the complete repository gate are pending. No bead is closed and no prior test receipt is reused. Source code and test source are not execution evidence.

This is scoped data/numerical-reference progress for FA-024, FA-027 and FA-090, serving FI-A05/FI-I05 and plan 10.6, 11.4 and 11.7. The base replay is a reference numerical consumer; the paired comparison belongs to the capability-free experiment layer. This does not complete original-token recomputation, native continuation, asynchronous serving, dependency admission or empirical causal qualification. The product dependency universe, original gate and existing tests are unchanged. The existing branch-base identity check's visibility is widened only to its containing KV module so both numerical consumers use the same binding law.
