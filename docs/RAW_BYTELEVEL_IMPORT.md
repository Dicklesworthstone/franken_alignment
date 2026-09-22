# Raw ByteLevel tokenizer import

## Status and capability changelog

2026-09-22: unqualified source implementation of `ByteBpe::from_huggingface_json`.
Ten Rust tests are authored but UNEXECUTED. The targeted command
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference hf_`
failed before execution because `rch` is unavailable (exit 127). Rust compilation,
rustfmt, Clippy and the full repository gate remain unexecuted for this source.
No existing receipt qualifies it and no bead is closed.

This is input interoperability for the original FA-025 numerical/token-identity
path. Its consumers are the existing `ByteBpe`, `TextDecoder` and their original
monitored generation/capture APIs. It is not a new tokenizer algorithm, inference
backend, generic Hugging Face pipeline, trained-model result or production gate.

## Admitted external profile

The importer accepts tokenizer JSON version `1.0` with all nine top-level fields
present. Truncation, padding, normalization and postprocessing must be null;
added_tokens must be empty. Both pre_tokenizer and decoder must explicitly be
ByteLevel with add_prefix_space, trim_offsets and use_regex all false.

The model is BPE with all ten fields present. Dropout and unk_token must be null;
continuing_subword_prefix and end_of_word_suffix must be null or empty. fuse_unk,
byte_fallback and ignore_merges must be false. Merges may use a homogeneous list
of legacy `left right` strings or two-string arrays. List order is rank; supplied
vocabulary values, not spelling order or rank, are the original model token IDs.

Every other transformation, missing behavior flag, unknown field, added-token
recognizer or malformed graph refuses. Do not remove unsupported settings from
an export to force admission: that changes the trained tokenization contract.
Ordinary GPT-2/Llama exports using regex splitting, special-token recognition,
normalizers or templates are NOT supported by this entry point.

The external schema and finite byte alphabet were checked against the pinned
Hugging Face tokenizers v0.22.2 primary sources:

- https://github.com/huggingface/tokenizers/blob/v0.22.2/tokenizers/src/pre_tokenizers/byte_level.rs
- https://github.com/huggingface/tokenizers/blob/v0.22.2/tokenizers/src/models/bpe/serialization.rs
- https://github.com/huggingface/tokenizers/blob/v0.22.2/tokenizers/src/tokenizer/serialization.rs

No upstream library, foreign runtime, regex engine or new Cargo dependency is
admitted. No claim is made about uninspected later serialization revisions.

## Native integration and trust boundary

The finite ByteLevel alphabet is inverted to exact bytes. No unknown glyph is
converted by fallback or normalization. All 256 native singleton bytes, token
lengths, total vocabulary bytes, ordered operands, duplicate merge pairs and
reachability are then checked by the ORIGINAL `ByteBpe::new` constructor.

The caller independently supplies the complete DecoderProfile. JSON has no model
identity: matching vocabulary cardinality does not authenticate training semantics
or prove that a tokenizer belongs to given weights. A live TextDecoder already
owns its immutable tokenizer and cannot swap this import into an existing KV
history. Native `to_bytes`/`from_bytes` interchange remains unchanged and binds
the supplied profile in its original header.

For admitted UTF-8 inputs, the intended compatibility is token IDs and exact
content bytes. Native spans partition ORIGINAL bytes, not Hugging Face's Unicode
character offsets. Native output stays exact bytes with strict UTF-8 access; it
does not adopt the external decoder's replacement-character behavior for invalid
UTF-8. The native binary-input extension is not an external-library parity claim.

The explicit caller byte bound cannot exceed `ByteBpe::MAX_HUGGINGFACE_JSON_BYTES`
(32 MiB). Shared strict JSON parsing has independent depth, item and string caps.
Vocabulary cardinality and native bounds are checked before retaining indexes;
merge concatenations are bounded before allocation. Errors expose no partial
admitted tokenizer or partial prompt, and no inference or external effect occurs.

## Authored regression coverage

The fixtures use permuted IDs and manually specified merge ranks. Positive cases
cover original-byte spans, every byte mapping, Unicode, both merge representations
and native archive roundtrips. A near-identical rank-order change legitimately
changes output, detecting accidental sorting by token ID or spelling. An independent
whole-sequence greedy-pair oracle checks all 1,555 strings of length zero through
four over a six-byte alphabet; this is bounded differential coverage, not a proof.

Negative twins cover transformation flags, added tokens, vocabulary IDs, unknown
glyphs, missing singletons, unreachable tokens, forward references, duplicate pairs,
merge shapes, strict JSON syntax and exact/one-under input bounds. These tests
have not run, and there is no performance, memory-allocation or deployment claim.
