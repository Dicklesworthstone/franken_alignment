# Explicit whole-piece-first byte BPE

The original native tokenizer now has an explicit `MergePolicy::WholePieceFirst`
profile for the BPE `ignore_merges=true` operation. This closes an input
compatibility gap: a complete ordinary pretoken may have an original vocabulary
ID even when ranked merging would produce different IDs or no rule constructs
that token. The existing `RankedOnly` profile, constructors, JSON selectors and
FA-BBPE/1 through /6 archives retain their original behavior and bytes.

This serves original-token execution and replay in FA-025 and plan sections 7.1,
10.6, 11.2 and 17.1. It uses the original native decoder, monitor, sampler and
recovery owners. Model-size, vocabulary-size, numerical, control and source
limits are unchanged; selecting a Llama-3 grammar does not admit arbitrary
Llama-3 checkpoints or authenticate their tokenizer.

## Exact operation

Literal named controls are selected first. Each ordinary span is segmented by
the independently selected raw-byte, GPT-2/Unicode-17 or Llama-3/Unicode-17
profile. For each complete ordinary piece, the tokenizer looks up the exact
original bytes in its immutable vocabulary. A hit retains that original token
ID and its complete byte span. A miss uses the existing ranked BPE pass,
including its rank order, leftmost tie break and work accounting.

The match covers the whole piece. If `abc` exists but `abcd` does not, a request
for `abcd` does not greedily select `abc` followed by `d`. Neither lookup nor
merging can cross a grammar boundary or named control. The separate fixed BOS
profile still inserts exactly one configured control with a synthetic `0..0`
span; all source bytes retain their original provenance after it.

The operation follows the upstream BPE model's
[`tokenize_with_cache`](https://docs.rs/tokenizers/latest/src/tokenizers/models/bpe/model.rs.html)
implementation: an enabled `ignore_merges` first checks the complete sequence in
the vocabulary, then falls back to merging on a miss. The official
[Python binding's BPE parameter documentation](https://github.com/huggingface/tokenizers/blob/main/bindings/python/src/models.rs)
describes the same selection. These sources were inspected on 2026-11-04; the
Docs.rs release was 0.23.2. No upstream implementation or new dependency is
linked into the native runtime.

All 256 unique byte tokens remain mandatory. Vocabulary spellings remain unique,
bounded and nonempty. Merge pairs remain unique, refer to earlier-defined
operands and concatenate to the exact declared result. Only WholePieceFirst
admits additional content without a constructing merge, because that content
has the explicit whole-piece path. RankedOnly still rejects such an inventory.
The admission extension does not relax unrelated merge-graph requirements.

## Explicit API and operator selection

Programmatic construction uses `ByteBpe::new_with_merge_policy`, or
`new_with_bos_token_and_merge_policy` for the existing fixed BOS operation.
`merge_policy()` exposes immutable descriptive data. A tokenizer inside an
existing numerical history cannot be changed through these constructors.

JSON import uses `from_huggingface_json_with_merge_policy`, with an independently
selected grammar and merge policy, or
`from_huggingface_llama3_bos_json_with_merge_policy`. The JSON boolean must match
the caller: `ignore_merges=false` for RankedOnly and `true` for WholePieceFirst.
Missing, mistyped or conflicting declarations refuse. The old entry points
still require false; parser failure never triggers another policy. Every other
original model, grammar, added-token and postprocessor restriction remains.

The finite-reader counterpart is `read_huggingface_json_with_merge_policy`.
`TextDecoder::from_huggingface_reader_with_merge_policy` applies the same
selection after the original fresh-owner check and before numerical execution.

| Consumer | No automatic BOS | Fixed automatic BOS |
| --- | --- | --- |
| Native helper checkpoint configuration | `huggingface-llama3-unicode17-whole-piece` | `huggingface-llama3-unicode17-bos-whole-piece` |
| Publication and monitor-training JSON | `huggingface_llama3_unicode17_whole_piece` | `huggingface_llama3_unicode17_bos_whole_piece` |
| Rust `NativeTokenizerFormat` | `HuggingFaceLlama3Unicode17WholePiece` | `HuggingFaceLlama3Unicode17BosWholePiece` |

These selectors use the original bounded asset loaders, profile checks and
complete tokenizer admission before any weight read. The native archive selector
also accepts the new format. Raw-byte and GPT-2 whole-piece profiles are available
through the explicit Rust APIs and native archive; no JSON content sniffing adds
an operator format implicitly.

## Bounds and cooperative work

The additional vocabulary index contains only original `u32` token IDs sorted by
their existing content bytes. It does not duplicate the spelling payload.
Index capacity is admitted before construction and remains bounded by the
original vocabulary ceiling. Each ordinary piece receives at most one binary
search, and pieces exceeding the maximum content-token length skip the search.

Lookup runs during bounded synchronous prompt admission, after the original
named-control and Unicode passes. Byte comparisons are bounded by the existing
input, token and vocabulary ceilings. This is not a constant-time or measured
latency claim. The cooperative candidate and collection passes request no new
buffer capacity and expose no unfinished IDs.

`TokenizationWork.input_bytes` still charges the admitted original source.
`pair_lookups`, `heap_pops` and `merges` still count only BPE operations actually
attempted. A direct whole-piece hit performs no internal BPE pair examinations
or merges; lookup byte comparisons are not claimed as BPE work. A miss retains
the original budget behavior. Cancellation and sticky failures retain spent work
without returning a successful prefix or refunding numerical work.

## Canonical persistence and semantic binding

WholePieceFirst uses `FABBPE07` (FA-BBPE/7). After the unchanged 120-byte complete
model header, it stores exactly eight interpretation bytes:

| Byte | Meaning |
| --- | --- |
| 0 | Grammar: 0 raw bytes, 1 GPT-2, 2 Llama-3 |
| 1–3 | Unicode version: `0,0,0` for raw bytes, otherwise `17,0,0` |
| 4 | Extensions: bit 0 named controls, bit 1 static RoPE; other bits zero |
| 5 | Fixed BOS: 0 absent, 1 present; present requires named Llama-3 controls |
| 6 | Merge policy: exactly 1, WholePieceFirst |
| 7 | Reserved: exactly zero |

The optional original `u32` BOS ID follows, then the existing static RoPE
extension, vocabulary, ordered merges and named controls. Unknown tags, policy
values, versions, extension flags, malformed inventories and trailing bytes
refuse. RankedOnly has no alternate /7 representation: its canonical output
remains /1 through /6.

Durable recipes and trained-monitor artifacts already bind the complete
canonical tokenizer archive. The new policy therefore participates in those
same exact comparisons. Changing only the merge policy invalidates a retained
binding even if a particular prompt happens to have identical IDs. Numerical
replay continues to consume the original retained IDs.

## Authored checks and execution boundary

`tests/whole_piece_bpe.rs` and its companion modules contain causal exact-ID
controls, whole-only vocabulary admission, unknown-piece fallback, grammar and
control isolation, BOS provenance, precise source/BPE budgets, cooperative
parity and cancellation, strict JSON selection, finite-reader EOF, independent
wire expectations for /1 through /7, and malformed archive controls.

The original worker configuration tests load single and sharded checkpoints
with both whole-piece selectors, use a token that has no constructing merge,
and retain its exact monitored prompt IDs before producing the original helper
verdict. Publication intake tests exercise its actual selector and canonical
archive path. Monitor-training tests preserve original-token campaign cases and
reject a policy-only replacement through the original complete artifact binding.

These tests are authored and source-reviewed, not executed. The required RCH
runner and Rust compiler are unavailable in this environment. Syntax parsing
does not establish Rust type correctness, runtime behavior, trained-model
quality, hostile-process isolation, packet closure or production qualification.
