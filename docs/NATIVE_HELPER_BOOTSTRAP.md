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

## Explicit local files and an existing worker socket

`NativeEvaluator::from_llama_files` accepts `NativeFileBootstrap`: an independent
policy and numerical stream ID, five explicit `NativeHelperFiles` paths, and
`NativeHelperFileLimits`. The files are the original Llama config, FA-BBPE bytes,
monitor JSON, sampler JSON and one SafeTensors file. It uses the same bootstrap
preflight and original model loader, then returns the original evaluator/receipt.
No filename is guessed from a model label or another asset. Use the explicit
reader-map API for sharded weights; the file API does not follow an index's paths.

The four auxiliary files share a caller-retained `NativeAssetReadBudget`, while
weights use the original caller-retained `WeightReadBudget`. Both preserve read
calls and actual bytes across failures and repeated startup attempts. Interrupted
reads count toward the call ceiling. EOF requires remaining observation capacity;
one extra byte is examined rather than treating a configured limit as real EOF.
Read caps include all format data, and limits are checked before allocation/open
where possible. Metadata and open operations are not counted as read syscalls.
Failures report the asset and stage rather than returning a partly loaded owner.

Final-component symlinks and nonregular files refuse, and the opened handle is
checked again. These are ordinary host-file checks, **not** race-free path
confinement or an atomic multi-file snapshot. The operator must protect parent
components and keep all files stable. Cross-file authentication and model/tokenizer
semantic compatibility are still independent obligations. No per-read call limit
preempts a blocking OS operation or provides a wall-clock startup bound.

On Unix, `NativeHelperClient::from_llama_files` consumes an already provisioned
connected socket plus an independently supplied salt. Salt-length admission
precedes all disk work. No socket bytes are read or written during startup; only
a fully constructed native evaluator enters the existing nonblocking client.
Failure drops the socket without a vote. Success uses the unchanged cooperative
step/drive interface: exact full-input inference, one-token yields, strict terminal
verdicts and the original commitment/reveal framing. There is no new listener,
network dial, executor, shared authority or external side-effect capability.

Twelve additional authored tests cover actual regular files and exact limits,
asset-reader budget conservation, missing/mismatched inputs, directories/symlinks,
malformed monitors, trailing weights, Interrupted/invalid reads, growing input,
and real provisioned Unix sockets. A checkpoint-loaded native helper answers the
original commit/reveal frames; a near-identical monitored hold emits no commitment.
Invalid salts perform no disk reads, and failed startup closes without a response.
All 24 new Rust tests remain UNEXECUTED pending the required RCH verifier.
