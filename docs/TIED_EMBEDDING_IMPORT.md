# Explicit tied-embedding checkpoint ingestion

## Status and capability changelog

2026-09-22: unqualified source support for explicitly tied Llama input/output
embeddings in memory, bounded-reader and regular-file imports. Nine new tests
are authored but UNEXECUTED. The targeted RCH command failed before compilation
because `rch` is unavailable (exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference tied_
```

Compilation, rustfmt, Clippy and the full gate remain unexecuted. No production
or pretrained-model qualification and no broader bead closure is claimed.

## Existing consumer and numerical boundary

The existing FA-025 decoder loader rejects `tie_word_embeddings: true` even
though the original dense decoder can represent the same computation with two
identical immutable matrices. Explicitly supporting that representation removes
an input-compatibility obstruction without adding a numerical engine, dependency,
training update, model-controlled alias, or effect permission. The original
monitored decoder, helper evaluator and replay consumers receive the same
`DecoderModel` type and unchanged profile/parameter/cache encodings.

`LlamaConfig::output_head` retains the declared `OutputHead` mode. A missing flag
still records the existing false default. The raw `from_safetensors` and
`read_safetensors` APIs remain strictly independent. Their explicit
`*_with_output_head` variants also accept `OutputHead::TiedEmbeddings` from the
operator. No tensor name or file metadata can enable sharing by itself.

Only `lm_head.weight` may be omitted in tied mode. `model.embed_tokens.weight`
and every other required tensor remain mandatory. Complete inventory, shapes,
finite values, offsets, non-overlap, byte limits and EOF are still validated by
the original parser/reader. If the head is omitted, a fallibly allocated exact
copy of the normalized embeddings fills the existing output slot. If both
matrices are stored, their normalized binary32 bits must match exactly, including
signed zero. Contradictory copies refuse with `TensorIssue::TiedValues`; neither
matrix silently wins. F16/BF16 expansion uses the existing scalar decoder.

This does not support arbitrary shared-name metadata or an export that retains
only lm_head while omitting input embeddings. It does not reduce the original
model's expanded parameter/memory ceiling. Physical tensor receipts list exactly
what was stored; normalized_bytes counts both expanded matrices. The budget
charges actual bytes/read attempts and is not reset after any refusal.

The configured shard route still explicitly refuses tying in this increment;
it does not silently execute it as independent weights. Its original untied
path is unchanged.

## Tests and contract migration

Nine new tests compare actual original decoder logits, nonzero KV images and
work against an independently constructed equal-matrix model across six tokens.
They cover both valid storage shapes, the independent default, malformed flags,
bitwise-conflicting copies including signed zero, exact F16/BF16 expansion,
missing required tensors, nonfinite values, wrong shapes, fragmented/Interrupted
reads, shared I/O budgets, strict EOF, late reader failures, invalid configuration
before I/O, and exact/one-under real-file size bounds followed by source removal.
Fixtures are synthetic parameter data, not trained-model or serving-host evidence.

Two old blanket tied-config negatives now use still-unsupported attention bias
or malformed tying types. The original no-read assertions and all unrelated
negatives remain. New positive and contradictory-head tests replace the obsolete
claim that all explicitly tied configurations must refuse.

## Primary-source context

The checkpoint-sharing convention and Llama output-head inventory were checked
against primary sources; no code or runtime from them is imported:

- https://huggingface.co/docs/safetensors/torch_shared_tensors
- https://github.com/huggingface/transformers/blob/v4.57.1/src/transformers/models/llama/modeling_llama.py

That convention does not authenticate supplied weights or prove compatibility
with uninspected architectures. Existing operator identity, tokenizer, monitor,
context, execution-profile and authority contracts still apply.
