# Checkpoint-backed native helper startup

`NativeEvaluator::read_llama_checkpoint` builds a fresh native helper from an
explicit Llama configuration, a SafeTensors reader, the FA-BBPE/1 tokenizer,
all-layer monitor JSON and sampling JSON. `read_llama_checkpoint_shards` accepts
an exact index and an explicitly supplied map of shard readers instead. This is
cold startup of the existing evaluator, not another inference implementation.

`NativeHelperBootstrap` borrows these operator-provisioned assets and a
`NativeHelperPolicy`. The policy's complete decoder profile is the expected
identity, shape, execution context and numerical interpretation. The checkpoint
configuration must agree; it cannot select a different profile. A tokenizer from
another model/tokenizer generation is refused. All helper policy, control-token,
output-capacity and sampler checks retain their existing semantics.

Before weight I/O, startup validates the configuration/profile, exact tokenizer,
native helper policy, sampling configuration and monitor size limit. The original
model-dependent monitor parser runs after loading weights, before returning an
owner. A malformed/mismatched monitor therefore may consume startup I/O but never
returns an unmonitored helper. No default quiet probe, tokenizer, seed, model name,
missing weight, guessed bias, tied head or foreign inference fallback is inserted.

The existing SafeTensors loader checks complete tensor names, shapes, offsets and
finite values. Single and sharded imports share the caller's `WeightReadBudget`;
read attempts, bytes, EOF probes and failure costs survive unsuccessful imports.
The loader neither seeks nor restarts readers. A shard label only selects from
the already supplied reader map and cannot open a filename or download a URL.

Success returns one `NativeEvaluator` and the original `PretrainedReceipt` or
`PretrainedShardReceipt`. Startup computes no prompt token and consumes no random
draw. The evaluator can then use the existing begin/advance/cancel path or be
moved into `NativeHelperClient`. It still requires complete original input and a
reviewed terminal categorical answer before the original commitment is frozen.
The checkpoint receipt describes interpretation and sizes, not authentication,
training provenance, model quality, a helper vote or effect permission.

The supported numerical configuration remains the original bounded dense Llama
reference profile, with its explicit rounding, shapes and parameter ceilings.
FA-BBPE/1 remains an explicit byte profile, not a general tokenizer.json importer.
The host must authenticate and keep source assets stable and must independently
establish that model, tokenizer and probes belong together. Caller-supplied Read
implementations retain their own blocking/scheduling contract. No new runtime,
network dial, production activation or trained-checkpoint qualification is added.

## Verification boundary

Twelve authored tests feed actual serialized synthetic SafeTensors through the
original loader and perform actual native inference. They cover input-dependent
allow/deny outcomes, original-composition equivalence, sharded loading, exact
profiles, malformed tokenizers/samplers/monitors, original monitor holds, truncated
and nonfinite weights, reader-set mismatch, EOF admission and shared budget
exhaustion. Synthetic parameters test integration, not a learned detector's quality.

The required RCH xtask invocation cannot start in the editing environment
(`rch: command not found`, exit 127). Rust compilation, rustfmt, Clippy and all
new tests are UNEXECUTED. Lexical and whitespace checks are not Rust execution.
No roadmap packet or bead is closed by these source changes.
