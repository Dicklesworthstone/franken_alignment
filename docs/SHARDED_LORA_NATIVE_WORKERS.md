# Native workers from sharded base checkpoints and a static LoRA adapter

## Existing consumer and completed connection

`NativeEvaluator::from_llama_lora_shard_files` connects the original native
helper to an explicitly registered set of base-checkpoint shards and one saved
plain LoRA adapter. The actual `fa-native-helper` executable selects this path
when its existing `fa.native-worker/4` manifest contains a sharded
`files.weights` declaration. One original worker, input contract, monitored
sampler, private socket protocol, deadline and step allowance remain responsible
for the resulting judgment.

The existing single-file adapter startup API and `/4` single-file declarations
retain their behavior. `/1`–`/3` do not gain an implicit adapter. There is no file
extension inference, checkpoint fallback, concatenation of shard files, model
download, adapter selection from actor input, new numerical engine or new
dependency. Model, vocabulary, file, parser and work ceilings are unchanged.

The new public request is `NativeLoraShardFileBootstrap` in
`native::bootstrap::files::lora`. It holds the original
`NativeShardFileBootstrap`, an independently supplied base identity, explicit
adapter configuration and weight paths, and the adapter weight-byte ceiling.
Its result retains both the original `PretrainedShardReceipt` and the original
`LoraMergeReceipt`; these describe interpretation and actual costs, not artifact
authenticity or training quality.

## Explicit executable declaration

Keep the normal `/4` adapted decoder, tokenizer, monitoring and adapter fields.
The existing checkpoint selector now also accepts:

```json
"weights": {
  "kind": "sharded",
  "index": "/operator/base.index.json",
  "shards": {
    "part-0.safetensors": "/operator/registered-a.bin",
    "part-1.safetensors": "/operator/registered-b.bin"
  }
}
```

This object is the `files.weights` value, not a complete manifest. The index
names only labels in the complete operator-supplied map. Labels need not match
path basenames. Missing, extra, malformed or disagreeing assignments refuse
before any shard weight read. The index never supplies a directory or authority
to open an unregistered path.

The resulting worker uses the existing explicit `tokenizer_format` choices,
including the named Unicode-17 profiles. The tokenizer, helper policy, sampler,
monitor and frozen input profile are provisioned for the **adapted** model.
The base and adapted identities must preserve tenant, model and tokenizer
lineage; both the model generation and numerical-profile generation must
strictly increase, even for a zero-valued adapter. Identical shapes do not make
old tokenizer or monitor bindings compatible.

## Admission order and retained work

1. Validate all request ceilings and the base-to-adapted identity law before
   asset I/O.
2. Read bounded configuration, tokenizer, monitor, sampler and adapter
   configuration through the same caller-owned auxiliary budget. Negotiate the
   complete final decoder profile and tokenizer/policy; decode the original
   adapter configuration and require its entire original merge allowance.
3. Read the index through that same auxiliary budget. Use the original
   checkpoint index parser to check the complete label inventory and configured
   tied-output-head law.
4. Open and bound every registered base shard, checking their aggregate file
   size, then open and bound the explicit adapter file. No base scalar has yet
   been read. Final-component symlinks and nonregular files refuse through the
   original file checks.
5. Run `DecoderModel::read_safetensors_shards_bounded` with the original base
   profile. It validates all base tensor directories before any base scalar,
   checks the aggregate declared extent, reads the actual tensors and probes
   actual EOF for every shard.
6. Run the original `read_lora_safetensors` on that complete base model. It
   validates the adapter directory, scalars and EOF, reserves the complete
   planned merge work, and constructs the new immutable model. The original
   monitor/sampler/native-helper admission runs before an evaluator is returned.

All base shards **and** the adapter consume one caller-owned
`WeightReadBudget`. Index and adapter configuration bytes share the original
auxiliary allowance; choosing shards does not refill it. Base `weight_bytes`
limits the sum of base shard files, while `adapter_weight_bytes` limits the
separate adapter. Actual read counts, failed attempts and EOF-probe capacity
remain governed by the common read allowance.

This ordering preserves the original two numerical owners. It is not an
all-base-and-adapter-header transaction: the adapter header is read after the
base model is complete. A late adapter-header, scalar or trailing-byte failure
therefore retains all consumed base reads. A merge overflow or subsequent
monitor failure retains the merge reservation. Exhausted read or merge budgets
cannot be reused as fresh allowances, and no unadapted helper or partial model
is returned on any failure.

After successful startup the original evaluator owns the admitted immutable
weights. Removing the source files does not change inference. The original
complete-input binding, all-layer monitor, exact verdict parser and
commit–reveal protocol still determine whether a judgment can leave the worker;
missing coverage, an invalid input profile, a held output, an unfinished answer
or a spent process-step allowance produces no commitment.

## Source tests and qualification boundary

`tests/native_worker_lora_sharded.rs` adds six public bootstrap tests and three
actual executable-worker tests. Synthetic rank-one and rank-two adapters operate
on the original numerical fixture. With the same base weights and input, a zero
adapter preserves Allow/Deny and a nonzero adapter reverses them through the
actual down-projection computation. Tests cover both native archives and the
original Hugging Face ByteLevel importer. The monolithic base file is removed
before every positive so a single-file fallback cannot pass.

Controls exercise complete and missing/extra shard rosters; wrong base identity;
old tokenizer and malformed configuration; one-short merge, aggregate file,
index, auxiliary-byte, shared weight-byte and read-call allowances; missing or
nonregular later files; a malformed later base directory before earlier tensor
bodies; later adapter-header and EOF failure; finite adapter values that overflow
the original merge; and monitor failure after the reservation. Read and merge
counters survive failure and a later explicit retry. The real binary also
exercises exact frozen-input mismatch, held monitoring, incomplete generation,
and process-step exhaustion with no commitment.

On 2026-10-11, qualified `nightly-2026-09-08` passes all nine new tests and all
15 existing executable LoRA, sharded-checkpoint and static-RoPE regressions.
The original native-helper unit selection also passes all 110 tests on this
source increment. The 134 selected tests establish the described bounded
composition, not complete workspace qualification. The original source-snapshot
and release gates remain unchanged; historical failures remain historical facts.

This profile is still one bounded CPU Llama decoder and one static plain LoRA
adapter. It does not support a sharded adapter, adapter stacks, live switching,
DoRA, quantized adapter execution, large-model admission, independent provider
authentication, a race-free filesystem sandbox or a cross-file atomic snapshot.
Operator-controlled immutable paths, asset authenticity and trained judgment
quality remain separate obligations. The tests establish executable numerical
and protocol composition on their fixed fixtures, not detector quality or
physical deployment isolation.
