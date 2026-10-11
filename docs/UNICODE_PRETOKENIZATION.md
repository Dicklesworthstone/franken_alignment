# Exact Unicode pretokenization for the original native decoder

The existing `ByteBpe` now has two explicitly selected Unicode 17.0.0 input
profiles. They supply the finite GPT-2 and Llama-3 pretokenization grammars to the
original bounded BPE engine. Original vocabulary IDs, ranked merges, named
controls, cooperative cursor, numerical decoder, monitoring, sampler and native
helper verdict parser remain their existing owners. This serves the original
token/replay contract in FA-025 and plan sections 7.1, 10.6, 11.2 and 17.1.

## Select exact input semantics

| `PreTokenization` | Input | Admitted Hugging Face pretokenizer |
| --- | --- | --- |
| `RawBytes` | Arbitrary original bytes | Original raw `ByteLevel`, with all three flags false |
| `Gpt2Unicode17` | Complete UTF-8 | `ByteLevel`, `add_prefix_space=false`, `use_regex=true` |
| `Llama3Unicode17` | Complete UTF-8 | Exactly `Sequence[Split, ByteLevel]`: the canonical Llama-3 pattern, `behavior=Isolated`, `invert=false`, then `add_prefix_space=false`, `use_regex=false` |

`ByteBpe::new`, `new_with_special_tokens`, `from_huggingface_json` and
`read_huggingface_json` retain their original raw-byte contract. A caller selects a
new profile explicitly:

```rust
let tokenizer = ByteBpe::from_huggingface_json_with_pre_tokenization(
    &profile,
    &json_bytes,
    max_bytes,
    PreTokenization::Llama3Unicode17,
)?;
```

The corresponding bounded-reader method is
`read_huggingface_json_with_pre_tokenization`. A fresh monitored owner can use
`TextDecoder::from_huggingface_reader_with_pre_tokenization`; the original fresh
owner check precedes reader I/O. Programmatic immutable construction uses
`new_with_pre_tokenization(profile, vocabulary, merges, names, mode)`.

No parser retry, filename inference or unknown regex chooses another profile.
The complete version-1.0 schema, original model flags, vocabulary and merge graph,
added-token restrictions, null normalizer, null postprocessor, null padding and
null truncation remain required. Prefix insertion, normalization, chat templates,
arbitrary regular expressions and SentencePiece behavior remain unsupported.

In the Unicode entry points, the pretokenizer's `trim_offsets` and the decoder's
three flags must be present booleans. They do not change these components' input
IDs or byte decoding: `trim_offsets` acts only when ByteLevel is a postprocessor,
which this profile does not admit, and ByteLevel decoding does not consult those
flags. Their inert values therefore canonicalize to the same native archive.
The old raw entry points retain their stricter all-false flag checks.

## Grammar and Unicode meaning

The GPT-2 grammar is:

```text
's|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+
```

The canonical Llama-3 grammar is:

```text
(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+
```

Alternatives keep their written priority and greedy behavior. `L` and `N` are
Unicode General_Category unions, and whitespace is the Unicode `White_Space`
property. They are independent of the compiler's Unicode version. Derived
`Alphabetic` would be wrong here: Greek ypogegrammeni U+0345 is a combining mark,
and Roman numeral characters belong to `N`, even though both can be alphabetic.
The Llama-3 one-to-three number limit counts Unicode scalar values, not bytes.

Llama-3 contractions use default Unicode simple case folding, restricted to the
literal contraction match. U+017F long s therefore matches the `s` alternative;
the original long-s bytes remain in the token. GPT-2 contractions remain case
sensitive. No source text is lowercased, normalized or replaced.

These are named **Unicode 17 profiles**. They do not assert parity with every
upstream regex implementation or Unicode-table version. The grammar names also
do not expand the numerical architecture, model-size or vocabulary limits:
input must still bind an independently supplied compatible decoder profile.

## Original bytes, boundaries and budgets

The full input must fit the existing 65,536-byte ceiling before UTF-8 admission.
An invalid byte anywhere refuses the Unicode request with its admitted input-byte
charge and no partial IDs. RawBytes continues to accept invalid UTF-8 verbatim.

The existing literal recognizer selects named controls first. Each remaining
ordinary span becomes a separate grammar input, so lookahead cannot cross a
control. The scanner marks boundaries in the original byte nodes. The original
pair-offering function excludes a candidate across a boundary; all later ranking,
merging, stale-candidate accounting, cooperative stepping and collection use the
existing implementation. For example, an attractive `a1` merge remains legal in
RawBytes, while both Unicode modes keep `a` and `1` separate. The permitted ` a`
merge remains available in both modes.

`TokenizedInput` still contains exact source bytes, original IDs and a complete
partition of original byte spans. These are native provenance spans, not upstream
character offsets. Decoding remains strict bytes, and decoding a control still
refuses. A native helper must reach an independently configured, fully monitored
stop before its output can become a verdict.

Vocabulary, token, merge, name and input payload limits are unchanged. Scanning
uses a fixed number of forward passes over bounded input and binary searches in
frozen tables; it allocates no regex engine or per-character classification array.
Input validation and segmentation occur during bounded synchronous admission.
Pair lookups and heap pops retain their original independent budgets and count
the BPE work actually attempted. These are logical work bounds, not latency,
allocator or resident-memory measurements.

## Archive and recovery compatibility

Raw tokenizers retain byte-identical `FABBPE01`–`FABBPE04` archives. Unicode
profiles use `FABBPE05`. The original 120-byte model header is followed by exactly
six interpretation bytes, before any existing static RoPE extension:

| Byte | Meaning |
| --- | --- |
| 0 | Algorithm: 1 for GPT-2, 2 for Llama-3 |
| 1–3 | Unicode version: 17, 0, 0 |
| 4 | Existing extensions: bit 0 named controls, bit 1 static RoPE; all other bits zero |
| 5 | Prefix insertion: 0, meaning disabled |

The original vocabulary, merge and optional named-control encoding follows.
Unknown algorithms, Unicode versions, prefix behavior, flag bits, malformed
extensions and trailing bytes refuse. RawBytes has no alternate representation
under version 5. The maximum native envelope adds only these six fixed metadata
bytes; original payload ceilings remain unchanged.

Existing durable text recipes and trained-monitor artifacts already compare the
complete canonical tokenizer archive. They now bind the grammar and Unicode
version through that same comparison. A grammar-only change invalidates the
binding even when a particular saved prompt would yield identical token IDs.
Equivalent accepted JSON formatting and inert component flags retain the same
effective identity. Numerical replay continues to consume original token IDs.

## Source provenance and validation boundary

The fixed tables come from the official [Unicode 17.0.0 character
database](https://www.unicode.org/Public/17.0.0/ucd/): `UnicodeData.txt`,
`PropList.txt` and `CaseFolding.txt`. Exact source SHA-256 values are pinned in
`tokenizer/unicode17.rs` and `tools/generate_unicode17_tables.py`.
The complete Unicode License V3 is retained in `third_party/unicode/LICENSE.txt`.
The offline development generator admits only those exact files; neither build
nor runtime invokes it or downloads data. It adds no runtime dependency.

The grammar sources are the [original GPT-2
encoder](https://github.com/openai/gpt-2/blob/master/src/encoder.py), the
[canonical Llama-3
tokenizer](https://github.com/meta-llama/llama3/blob/main/llama/tokenizer.py), and
the [ByteLevel component at Hugging Face tokenizers
v0.22.1](https://github.com/huggingface/tokenizers/blob/v0.22.1/tokenizers/src/pre_tokenizers/byte_level.rs).
No upstream tokenizer implementation or regex package is linked here.

Four scanner unit tests contain independent expected segmentation examples and
mixed Unicode partition controls. Fourteen public integration tests cover actual
BPE boundaries, IDs/spans, special-token isolation, exact budgets and cancellation,
invalid input before inference, bounded readers, archive and legacy compatibility,
strict pipeline selection and original complete trained-monitor bindings. The
cooperative native path preserves the same original IDs, work and verdict as the
synchronous path, with zero numerical work throughout prompt admission.
A synthetic numerical fixture keeps model parameters and prompt fixed while the
selected tokenizer changes original prompt IDs; the original monitored helper
then computes different verdicts. Separate controls hold the answer or stop and
retain failed work. These are implementation controls, not trained detector
quality, tokenizer provenance authentication or alignment research results.

Central verification commands for this increment are:

```sh
cargo test --locked -p fa-reference --test unicode_pretokenization
cargo test --locked -p fa-reference --lib action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::
```

On 2026-10-11, both selections pass on qualified `nightly-2026-09-08`: all 14
public tests and all 61 tokenizer unit tests. The frozen property tables also
match the independently parsed official source over all 1,114,112 code points
and every admitted simple case-fold mapping.

The actual consumers pass all 13 worker configuration tests, all 11 monitor
training command tests and all 23 codec training command tests (the latter
includes the same 11 monitor command tests). A new generated-publication test
executes both grammars with both human decisions, exact paused-generation
recovery and refusal of a grammar-only change without a journal write. The
final combined public run passes 46 tests across Unicode input, cooperative
native admission, complete trained-monitor binding and durable learned streams;
all 110 native-helper unit tests pass as well.

The broader learned-publication selection passes 32 of 34 tests; its two
existing real Unix-listener tests fail at listener creation with environment
EPERM. Complete workspace qualification still depends on the unchanged original
project gate; these targeted results do not qualify detector quality or all
upstream pretrained tokenizer pipelines. See `IMPLEMENTATION_STATUS.md` for the
integrated gate result.
