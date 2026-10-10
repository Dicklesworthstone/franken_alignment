# Explicit tokenizer intake for native publication

The `supervise_publication --native-text` and `create-generated` workflows now
accept an explicitly selected Hugging Face raw ByteLevel BPE `tokenizer.json`.
Both use the existing `ByteBpe::from_huggingface_json` importer and the original
native generation, actor submission, helper review, human approval and publication
paths. This completes operator input wiring for the existing FA-025/FA-107 native
text composition in plan sections 8.7, 10.6, 11.2 and 17.1.

## Recipe versions

| Recipe schema | Weight selection | Tokenizer field |
| --- | --- | --- |
| `fa.native-text-service/1` | Original single-file path | Original native-archive path string |
| `fa.native-text-service/2` | Original explicit shard index/map | Original native-archive path string |
| `fa.native-text-service/3` | Original single-file path | Required format/path object |
| `fa.native-text-service/4` | Original explicit shard index/map | Required format/path object |
| `fa.generated-publication/1` | Original single-file path | Original native-archive path string under `files` |
| `fa.generated-publication/2` | Original single-file path | Required format/path object under `files` |

For native service recipes `/3` and `/4`, select the tokenizer with this field:

```json
"tokenizer": {
  "format": "huggingface_raw_bytelevel",
  "path": "tokenizer.json"
}
```

Native service paths retain their existing resolution relative to the recipe.
For `create-generated`, select schema `fa.generated-publication/2` and place the
same object at `files.tokenizer`, using the existing absolute normalized path:

```json
"tokenizer": {
  "format": "huggingface_raw_bytelevel",
  "path": "/operator/tokenizer.json"
}
```

The other admitted format is `native_archive`, which invokes the original
FA-BBPE archive reader. Both object fields are required; unknown fields or formats
refuse. Legacy versions continue requiring a plain native-archive path string.
The format is selected from the recipe, with no extension inference, content
sniffing, conversion fallback or model-name lookup.

## Input and recovery contract

The admitted JSON behavior remains the original strict raw ByteLevel subset:
no normalization, regex splitting, prefix insertion, offset trimming, padding,
truncation or postprocessing. Explicit literal special controls retain their
exact IDs and recognition behavior. Unsupported added-token transformations
refuse. This does not expand the importer to arbitrary Hugging Face tokenizer
pipelines. Prompt bytes, explicit prefix controls and stop IDs retain their
existing checks; successful tokenization supplies no publication permission.

The complete recipe schema and format selection are admitted before referred
files open. The selected tokenizer is then read within its original format limit
and checked against the negotiated complete model profile before any weight file
or shard index opens. Native service recipes charge the physical JSON bytes to
their existing aggregate input allowance. `create-generated` retains its existing
individual bounded-file policy. These input caps do not measure peak memory or
reserve future journal space.

The resulting immutable tokenizer is encoded by the original native archive
writer and pinned by the original durable bootstrap. Recovery compares the full
effective tokenizer, including token IDs, merges, special-control spellings and
model profile. A changed tokenizer refuses before a canonical recovery write,
even if the particular saved prompt would happen to tokenize identically.
Equivalent JSON whitespace or an explicitly selected native archive with the
same complete tokenizer has the same durable semantics. Raw JSON bytes, input
path and outer recipe format are not new journal identity fields.

All existing lifecycle routes consume the same recipe loader. Completed requests
remain receipt-only during recovery: the original generation is not sampled again,
its old deadline is preserved, and source files, helpers and a new human approval
are not needed to retrieve an already recorded outcome. Pending generation keeps
the original acknowledged token, RNG and work history.

## Validation scope

Eleven new example regression functions cover real single-file and sharded model
imports, external tokenizer JSON, actual generation, both human decisions and
canonical publication; changed-tokenizer refusal with byte-identical journal
preservation; exact receipt recovery with source/helper inputs unavailable;
held-output nonpublication; exact aggregate byte limits; legacy recipe identity;
unsupported settings before weight reads; explicit schema selection and absence
of parser fallback. Synthetic weights and same-process test peers exercise the
original enforcement path; they establish no trained-model quality, tokenizer
authenticity, hostile-process isolation or deployment qualification.

Two of these regressions directly pass through the actual new
recipe loaders, stop after the first sampled output token, and reopen through the
same native service or generated publication opener. They check exact retained
intent, numerical state and spent work, then require fresh evidence and explicit
resume before the final monitored stop. The original commit/reveal review and
separate human approve/reject branches exercise final canonical publication.
Premature publication and the old reviewer role both refuse. Changed tokenizer
input refuses without a journal write, and exact receipt recovery works with the
evidence file removed. These direct tests do not exercise socket transport; the
four complete transport tests retain their separate original transport path.

The central verifier owns compilation and execution of these new tests. No
previous qualification receipt validates this addition. The command that selects
both new regression modules is:

```sh
cargo test --locked -p fa-reference --example supervise_publication tokenizer
```

The direct owner tests alone are selected with `huggingface_direct_owner`.
