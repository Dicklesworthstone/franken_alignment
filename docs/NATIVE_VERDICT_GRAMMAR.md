# Finite verdict grammar for native helpers

The native helper can explicitly restrict sampled output to the four exact
verdict words `allow`, `hold`, `deny` and `abstain`, followed by a registered
terminal control. This implements the bounded output language from founding
idea FI-A03 and plan section 9.1. A valid word remains an empirical model
judgment; grammar validity does not establish truth or effect authority.

## Select the grammar

The public native evaluator keeps its existing policy structure and default
unrestricted behavior. Select the grammar before admitting any input:

```rust
use fa_reference::action::consequence::oversight::helper_client::native::{
    NativeEvaluator, NativeVerdictGrammar,
};

let evaluator = NativeEvaluator::new(decoder, policy)?
    .with_verdict_grammar(NativeVerdictGrammar::FiniteWordsV1)?;
```

The builder accepts only a fresh evaluator. Once the finite grammar is installed,
it cannot be removed or changed. `verdict_grammar()` reports the selected mode.
The evaluator owns the decoder and exposes no mutable decoder or checkpoint
export. Existing checkpoint/replay owners initialize without grammar and reject
grammar-bearing capture/replay; no archive silently omits a live grammar state.

The executable `fa-native-helper` accepts the new closed manifest
`fa.native-worker/5`. Its required root fields are:

```text
schema, input, decoder, policy, files, stream, salt_file, lifetime,
startup, tokenizer_format, adapter, verdict_grammar
```

Keep the existing complete profile, files and budgets. Relative to a `/4`
manifest, change `schema` to `fa.native-worker/5` and add
`"verdict_grammar": "finite-words-v1"`. An unadapted model must explicitly select
`"adapter": null`; an adapted model uses the existing static LoRA object.
`decoder.rotary` remains explicit, including `{"kind":"none"}` when unscaled.
Single and explicitly sharded checkpoints use the same original loaders.

The only other accepted grammar selection is `"unrestricted"`. Manifests `/1`
through `/4` retain their original closed fields and unrestricted behavior; they
reject the new field. Grammar selection belongs to the independently registered
helper/input profile and evaluation generation because it changes the model's
judgment distribution. An input packet cannot select or change it.

### Learned publication helpers

The learned publication operator can select the same grammar for its in-process
native reviewers using `fa.learned-native-roster/3`. Keep the complete `/2`
member model, files and text budgets, change the root schema to `/3`, and add
`"verdict_grammar": "finite-words-v1"` or `"verdict_grammar": "unrestricted"`
to **each** member object, alongside `model`, `files` and `text`. Different
registered members may select different modes. Every declaration is checked
before any model asset is loaded; omission, unknown values and nontext values
refuse. Roster versions `/1` and `/2` stay unrestricted and reject the new field.

Each loaded member retains its original immutable selection. Every fresh
evaluator in later rounds and message continuations applies that selection
before input; mutable roster files cannot change it midway through a review.
The original member input profile still binds the actual action epoch, and
complete prompt/probe review, all numerical allowances, congress reduction and
independent human approval remain required. Register and qualify the changed
helper/input profile and evaluation generation independently before deployment.
This manifest option does not authenticate registration or calibrate verdicts.

## Sampling and monitoring

The finite grammar is compiled against the actual immutable tokenizer content
bytes and original token IDs. It supports byte tokens, merged tokens and tokens
crossing several word-prefix boundaries. It does not re-tokenize a verdict into
a forced answer, change the prompt, normalize case or Unicode, trim whitespace,
insert a suffix or interpret a control's printed spelling as text.

At each generated token, only tokens that extend a valid word prefix are
eligible. Registered stop controls become eligible after an exact complete
word. Spaces, prose, extra suffixes, newline repairs and early stops cannot turn
into a verdict. Empty support is a failure, never a default answer.

The original sampler still validates and accounts for the entire vocabulary,
including nonfinite logits on excluded tokens. Filtering precedes temperature,
top-k and nucleus selection. The existing rounded arithmetic, original token-ID
tie rule and PRNG are retained, and top-k is bounded by eligible support. Even a
single eligible token consumes its original random draw. The full vocabulary
work allowance is not reduced to the number of eligible tokens.

Each chosen token, including the terminal control, goes through the original
numerical computation and mandatory all-layer monitoring. The grammar state
and PRNG advance with that same numerical token before fallible monitoring, so a
held token cannot reroll or become a vote. Complete prompt review, exact original
input bytes and a reviewed stop remain mandatory. A word without its terminal,
an exhausted budget or a held answer/terminal remains incomplete.

## Evidence and limits

Nineteen regression functions were authored across the sampler, grammar,
evaluator, manifest loader and spawned worker. The decisive worker case gives
invalid `x` a higher output-head score than `allow`. Identical model bytes produce
`InvalidVerdict` in unrestricted mode and a genuinely computed lower-scored
`Allow` commitment/reveal with the grammar selected. Other cases cover all four
words, multi-token spelling, complete prompt review, input dependence, held
answers and terminals, work exhaustion, unchanged legacy sampling, both weight
layouts, both tokenizer inputs, and static LoRA with explicit rotary scaling.

Five additional operator regression functions cover the actual learned native
review followed by an authenticated human peer and durable publication; the
same higher-scored invalid-token comparison through the loaded roster; all
four verdicts and the separate human key; frozen mixed member choices across
input epochs; and complete schema rejection before model reads or store
creation. These new functions passed source and Rust syntax review only.

Source/API review and Rust syntax checks are complete. The required RCH runner
is unavailable, so compilation, runtime execution, formatting, Clippy and
qualification remain pending. Synthetic model fixtures establish intended
integration and refusal behavior; they do not calibrate a deployed helper's
accuracy, independence or resistance to manipulation.
