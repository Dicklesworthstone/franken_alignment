# Native text from the original learned generator

## Original input and output, not an output override

`DecoderModel::observed_learned_text_generation` composes the existing native
`ByteBpe` tokenizer and original `ObservedLearnedGeneration`. This serves plan
sections 7.2, 8 and 10.15: supervised output must be tied to the computation whose
learned evidence is reviewed. The original token-ID API remains unchanged.

The complete tokenizer profile must equal the decoder profile. Text is encoded
by the original tokenizer, including its explicitly configured special-token
semantics, with no inferred template, normalization or alternate tokenizer.
The exact original prompt and successful tokenization work remain inspectable.
The sampler, stop IDs, learned monitor, generation horizon, numerical allowances
and aggregate telemetry ceilings freeze before inference. No existing advanced
owner or caller-provided output can be adopted through this API.

`text_message` requires a normally completed, still-live generation. It decodes
only accepted continuation IDs, never the prompt or a held candidate. A declared
control ID is suppressed only when it is the actual final accepted stop. Other
control IDs fail decoding; ordinary byte-bearing stop IDs are refused at setup.
`StopRequired` rejects a token-limit result, while `StopOrTokenLimit` explicitly
admits that finite horizon. Both retain the actual stop classification.

Output must be nonempty, within the frozen byte cap and valid UTF-8. No
truncation, replacement characters, whitespace changes or partial-message
success is possible. The byte cap cannot exceed the existing action payload
limit. The message retains the original learned evidence and exact tokenizer;
its stop token remains in that evidence even though it is absent from the text.
Cloning historical output does not keep its numerical owner live, authorize an
effect, or allow another owner with identical numeric IDs to validate it.

## Costs and limits

This path uses the existing tokenizer and numerical engines, not a parallel
implementation. Tokenization has its original bounded work contract. Failed
preparation can do bounded tokenization without returning a successful work
receipt; the public constructor reports the original error, not a complete
failed-work ledger. Each message capture performs bounded decoding and copying
again; there is no zero-copy or constant-time claim. The existing evidence caps
still govern the retained original source, tokens and score limbs. These are
logical limits, not peak RSS or lifetime global research-budget enforcement.

The latest-position learned audit is not a complete historical activation trace.
These APIs implement neither an external tokenizer authenticity check nor a
qualified language model, detector, helper or production runtime. Message data
alone do not constrain an unrelated generic effect broker or grant permission.

## Authored checks; execution pending

Six new regression functions use the original fitted codec, monitors and a tiny
nonzero-attention decoder with a 259-ID native byte vocabulary. They compare
native text to a separately run original generator, require a real BPE merge,
check exact sampled words and probability bits, and cover stop versus finite
horizon, UTF-8 split over token boundaries, malformed UTF-8, unexpected controls,
empty output, exact/one-less byte caps, holds, source-check failures, tokenizer
profile mismatch, input-work refusal and evidence-owner/retention boundaries.
Two compile-fail examples reject permission conversion and mutable output.

All Rust tests, compilation, rustfmt and Clippy are unexecuted. The required
remote-only RCH targeted test and full xtask commands cannot start here because
`rch` is absent (exit 127). Source/hash checks do not qualify the new behavior;
no roadmap packet or Bead is closed. This is an unqualified reference addition.
