# Indexed weight sources for the original decoder

Consumer: a host loading a compatible exported model split across SafeTensors
files into the existing immutable DecoderModel. This extends the concurrent
single-file loader; it preserves that API, its fixtures, scalar implementation,
model architecture, parameter bounds and exact checkpoint owner. It does not
introduce another inference implementation or claim model quality/authenticity.

## Complete indexed admission

DecoderModel::from_safetensors_shards accepts the independently supplied validated
DecoderProfile, an index document, and an exact map of source labels to borrowed
file bytes. The index follows the [documented weight_map structure](https://huggingface.co/docs/transformers/main/big_models#shard-metadata).
It may also contain metadata with a total_size field, which is checked against
the actual aggregate raw tensor bytes. No other top-level or metadata fields are
interpreted. Empty metadata is permitted. Duplicate JSON keys refuse.

The index must contain the ENTIRE original tensor inventory, assigning every
parameter to exactly one named shard. The source set must exactly match the
referenced shard set. Every file must contain exactly its assigned subset:
missing, duplicated, misplaced, unindexed or unknown tensors cannot override an
earlier value. The shared original descriptor checker verifies all shapes,
dtypes, relative offsets and gap-free/nonoverlapping payload coverage before
scalar decoding. The single-file path uses that same checker and constructor.

Index shard names are literal bounded ASCII basenames ending in .safetensors;
URLs, absolute paths, parent components, control characters and drive prefixes
refuse. This API never opens a path or downloads a file. Labels select only the
explicit supplied source map. Header metadata cannot select a model profile,
tokenizer, executable or authority. Receipts retain each actual source file's
header/data bytes and tensor interpretation rather than inventing one combined
source file or claiming authentication from numerical IDs.

The index is capped at 1 MiB, the set at 128 shards, and the sum of source-file
bytes at MAX_WEIGHT_SET_BYTES (the existing single-file global bound). This is
an aggregate cap, not a fresh allowance per shard. DecoderProfile retains its
original normalized parameter and context limits. No model is returned until
all assigned files, actual total_size and original DecoderModel admission pass.
A failure cannot modify a previously admitted model or its pinned checkpoints.

## Regression source and limits

Eight new scenarios compare indexed mixed-precision shards against single-file
and direct-model inference, including every layer's full KV image, complete
logits and free-running greedy continuation. They cover one tensor per shard,
missing/extra sources, misplaced/duplicated parameters, hostile labels, duplicate
index keys, false total_size, exact/one-over index bounds, late nonfinite data and
checkpoint continuation after source loss. Deterministic fixture parameters are
not evidence of learned capability. The original single-file tests and official
format fixture remain unchanged.

The Rust changes have not been compiled or executed in this editing environment;
Rust and RCH are unavailable. Previous execution evidence does not qualify these
changes. No bead is closed, no dependency is admitted and no historical test
result is promoted. Architecture/tokenizer compatibility, signatures, trusted
source identity, pretrained-model evaluation and serving-host qualification remain
outside this reference import boundary.
