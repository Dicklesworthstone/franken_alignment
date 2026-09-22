# Literal special-token import and native helper termination

## Status

2026-09-22: authored source extension and eleven Rust regression tests (seven
importer cases, four native-evaluator integration cases). Compilation, tests,
rustfmt and Clippy are UNEXECUTED. The remote-only test command failed before
execution because `rch` is unavailable (exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference hf_special
```

No earlier qualification receipt covers this addition, and no bead is closed.
Together with the ten named-control tests, this series adds twenty-one tests.
This serves the FA-025 original-token/native-helper path, not a production gate.

## Functional connection

The original raw Hugging Face importer admitted no added tokens and produced only
Content tokens. NativeHelperPolicy requires nonempty control-only stop tokens:
a content stop could hide a contradictory suffix. Thus imported data could drive
TextDecoder but could not satisfy the existing native helper's termination policy.

The importer now retains explicitly declared literal special tokens as Control
IDs with immutable input spellings. Existing ByteBpe::read_huggingface_json,
TextDecoder::from_huggingface_reader, native archive loading, and NativeEvaluator
consume this data without another inference path or a relaxed stop policy.

Native evaluation still requires a fully monitored stop, not just the bytes
`allow`. A held answer, a held EOS, token-limit truncation, malformed verdict or
budget exhaustion never becomes a successful vote. Generation consumes the
original draws and retains the original report. There is no reroll, guessed EOS,
new authority token or actor-to-reviewer privilege transfer.

## Admitted external subset

Every added token has exactly seven fields: id, content, single_word, lstrip,
rstrip, normalized and special. special must be true; all other boolean flags
must be false. Non-special additions, normalized matching, whole-word rules,
whitespace stripping, unknown fields and missing flags refuse. Existing raw
ByteLevel pipeline requirements still apply: no regex pretokenizer, normalizer,
prefix-space insertion, padding, truncation, templates or postprocessor.

Added records must have increasing IDs. The model vocabulary is a dense prefix;
added controls may reserve an exact existing entry or extend that prefix. Exact
IDs and final coverage must match the independently supplied decoder vocabulary.
No gap, duplicate, conflicting spelling/ID or implicit renumbering is accepted.
Specials cannot replace a required byte singleton or participate in BPE merges.

Special content is literal UTF-8, not the model's reversible ByteLevel alphabet.
The new native recognizer implements leftmost-longest matching, matching the
pinned upstream default special-recognition mode. This does not implement the
upstream runtime mode that deliberately splits special tokens. Arbitrary pipeline
or model compatibility, training identity, and upstream-library parity remain
unqualified. Original byte offsets differ from upstream character-offset APIs.

The external behavior was checked against primary sources at tokenizers v0.22.2:
https://github.com/huggingface/tokenizers/blob/v0.22.2/tokenizers/src/tokenizer/added_vocabulary.rs
In particular, AddedTokenWithId defines the fields, refresh_added_tokens selects
LeftmostLongest, find_matches applies the optional flags, and serialization orders
records by ID. No upstream crate or new dependency is added here.

The 256-name, 4096-byte individual-name and 65536-byte aggregate-name limits are
checked before retaining unbounded named data. Full JSON parsing retains its
existing independent byte/depth/item/string caps. Native version-two archives
retain exact spelling semantics; unnamed tokenizers keep byte-identical v1 output.
See NAMED_CONTROL_TOKENIZATION.md for the encoding and recognition contract.

## Authored tests and explicit nonclaims

Importer tests compare appended and in-vocabulary controls, permuted ordinary IDs,
raw Unicode special names, overlapping names, byte spans, actual BPE boundaries,
and native archive roundtrips. Negative counterparts cover every unsupported flag,
missing fields, ID gaps/order/conflicts, duplicate spellings, merge participation,
name limits, and full-prompt budget refusal. Existing malformed-added-token
regressions remain unchanged and still refuse rather than invent defaults.

Native tests feed independently constructed ORIGINAL WorkerInput wire frames to
NativeEvaluator. Synthetic model weights compute allow for one prompt and deny
for another through the original attention/capture/monitor/sampler/text path.
One test imports an actual file and removes it before evaluation; another verifies
that a named control in the forced prompt is not a sampled early stop. Separate
cases hold the answer or EOS using real residual probes, exhaust the terminal's
sampling allowance, reject content stops, and reject a different decoder profile.
These are authored integration tests, not trained-model, independent-helper,
cryptographic, sandbox, performance, or executed regression evidence.
