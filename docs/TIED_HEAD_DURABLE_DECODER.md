# Tied output heads in the durable monitored decoder

## Capability

`FileDecoderConfig::new_with_output_head` freezes an explicit native `OutputHead`
with the original profile, SafeTensors bytes, monitor, sampler and binding limits.
`OutputHead::TiedEmbeddings` uses the existing numerical loader's shared-embedding
contract: only `lm_head.weight` may be omitted. When a head is physically stored,
its normalized f32 bits must exactly equal the embeddings, including signed zero.
There is no head inference from missing data, alternative tensor reader, kernel,
monitor, sampler or authority path. Existing `FileDecoderConfig::new` remains
strictly independent and still refuses an omitted head.

This connects the already implemented numerical ingestion mode to the durable
native model path. It serves the plan's exact model-space identity and monitored
execution contracts (sections 7.10, 10.6 and 11.2); it changes no permit rule.
The actual parameter bytes and arithmetic platform remain operator trust roots.
Tying expands into the original dense representation, so it claims no runtime
memory saving, trained-model compatibility beyond the loader's narrow profile,
or detector accuracy.

## Persistence and recovery

The mode is part of immutable configuration equality and is checked by all
existing model-pinned recovery paths. Even an archive storing two equal matrices
cannot switch between Independent and TiedEmbeddings at recovery merely because
its numerical output would match.

Decoder-event subtag **0** and its complete configuration payload remain the
legacy independent format. New subtag **12** identifies the tied configuration;
its body uses the original bounded layout. Existing tags 1 through 11 are
unchanged. Older readers reject the unknown new subtag, not reinterpret it.
Bounded decoding does not certify parameter semantics: original semantic replay
must still build the model and compare its numerical history before recovery
returns an owner. No state, random stream, budget, approval or key is imported.

## Source and execution status — 2026-09-25

Six regression functions in `decoder/config/tests.rs` cover explicit admission,
physically present matching and conflicting heads, independent legacy bytes,
distinct event roundtrips, truncated/trailing/retagged data, mode-only recovery
substitution without canonical writes, real partial generation versus the
independent numerical control with conserved work, and monitor holds across
recovery. The tests run shared synthetic weights through the original decoder
and assert `ab -> A -> Control`; no precomputed report supplies that result.

The required RCH gate attempt failed before compilation because `rch` is absent
(exit 127). The new tests, Rust compilation, rustfmt and Clippy are UNEXECUTED.
Source and whitespace review do not qualify production execution, and previous
receipts do not validate this change. No Beads item is closed.

## Native executable consumer

The `--native-text` loader (including checked creation and recovery) and the
separate `create-generated` loader consume the original Llama configuration's
`tie_word_embeddings` declaration. Their original schemas, byte caps, tokenizer
checks, prompt bounds and explicit stop requirements remain unchanged. An
independent declaration with an omitted head still refuses; declaring tying
cannot conceal an inconsistent physically stored head. The mode is passed to
`new_with_output_head`, not inferred from the file inventory.

Four additional service tests cover checked creation and recovery with both
human decisions, parameter/declaration conflicts, exact aggregate limits,
expired receipt recovery without live producers/helpers, mode-only substitution,
and held/limited generation. One additional generated-recipe test drives actual
loaded tied weights to a monitored stop without publication. Its prior tied
refusal test now pairs missing-weight refusal with valid tied-file admission,
while retaining the foreign-tenant check. The shared 259-token fixture retains
its original weight values; a parameterized form supplies the separate
257-token generated-recipe fixture. All these tests remain UNEXECUTED; fresh
RCH attempts again stopped before compilation with exit 127.

No changes are made to numerical execution, helper/human approval, witness
validation, actor intake, deadline, stop or receipt reducers. Tied journals
cannot be opened by pre-extension readers; independent journals retain their
original encoding. See NATIVE_TEXT_PUBLICATION_SERVICE.md for command forms.
