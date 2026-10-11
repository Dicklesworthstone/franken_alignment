# Explicit checkpoint sources for learned training and publication

The codec and probe training commands can load an explicitly mapped set of
SafeTensors shards, a saved static LoRA adapter, or both. They use the original
decoder, shard reader and adapter merger. The effective model then enters the
same capture, fitting, probe campaign and artifact binding used for a single
checkpoint file.

Select this input with the leading `--checkpoint-source` switch in
[`train_learned_kv_codec_from_checkpoint`](TRAIN_LEARNED_KV_CODEC_COMMAND.md) or
[`train_learned_kv_probes_from_checkpoint`](TRAIN_LEARNED_KV_PROBES_COMMAND.md).
The existing weights operand becomes the manifest path. All other operands,
training/campaign schemas and output formats retain their meaning. Without the
switch, that operand still names a single SafeTensors file. There is no suffix,
content or parse-failure fallback.

The [learned publication operator](LEARNED_OPERATOR_PUBLICATION.md) selects the
same manifest through an explicit weights object in recipe versions `/3` or
`/4`, or in native-roster version `/2`:

```json
{"format":"checkpoint_source","path":"/operator/model/checkpoint-source.json"}
```

Its `files.weights` string form continues to name one SafeTensors file. Older
recipe and roster versions require that string form. `/3` adds the source input;
`/4` also explicitly admits separately reviewed messages within one original
finite generation. The source manifest itself never enables continuation or
changes publication authority.

## Manifest: `fa.checkpoint-weights/1`

The manifest is a closed JSON object, at most 65,536 bytes. Every field shown in
the following table is required; unknown or duplicate fields refuse. Counts and
identity fields are unsigned integers. Paths must be absolute normalized file
paths. An index label is an exact lookup key, never a path to discover or open.

| Object | Required fields and accepted forms |
| --- | --- |
| Root | `schema`, `base`, `adapter`, `limits` |
| `base`, single | `kind: "single"`, `path` |
| `base`, sharded | `kind: "sharded"`, `index`, `shards` |
| `shards` | Nonempty object mapping each exact index label to its explicit file path; at most 128 entries |
| `adapter` | `null`, or an object containing `base_identity`, `configuration`, `weights`, `merge_products` |
| `base_identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation` |
| `limits` | Positive `weight_bytes` and `weight_read_calls`, within the original reader ceilings |

For example, an explicitly mapped base and saved adapter:

```json
{
  "schema": "fa.checkpoint-weights/1",
  "base": {
    "kind": "sharded",
    "index": "/operator/base/model.safetensors.index.json",
    "shards": {
      "model-00001-of-00002.safetensors": "/operator/base/part-a.safetensors",
      "model-00002-of-00002.safetensors": "/operator/base/part-b.safetensors"
    }
  },
  "adapter": {
    "base_identity": {
      "tenant": 1,
      "model": 2,
      "model_generation": 1,
      "tokenizer_generation": 1,
      "profile_generation": 1
    },
    "configuration": "/operator/adapter/adapter_config.json",
    "weights": "/operator/adapter/adapter_model.safetensors",
    "merge_products": 1000000
  },
  "limits": {
    "weight_bytes": 10485760,
    "weight_read_calls": 100000
  }
}
```

These identities, paths and allowances are illustrative. The caller supplies the
independently selected final identity in its existing plan or recipe. The declared adapter
base must satisfy the original static-LoRA identity transition: the same tenant,
model and tokenizer generation, with strictly advanced final model and profile
generations. The shared configuration must negotiate that complete final
profile. A limit must cover the actual configured model and selected adapter;
the example does not establish an adequate budget for an arbitrary checkpoint.

For an unadapted checkpoint, set `adapter` to `null`; its base identity is the
caller's final identity. For a single base, replace the entire `base` object with
`{"kind":"single","path":"/operator/base/model.safetensors"}`. The adapter form
is the same for single and sharded bases. Adapter configuration, supported tensor
targets and merging semantics are described in
[static LoRA ingestion](STATIC_LORA_INGESTION.md).

## Admission, work and failures

The offline training commands first perform their existing numerical and corpus
admission. Probe training also checks its tokenizer and independently retained
codec source descriptors before tensor access. The manifest, adapter configuration and shard
index consume the caller's existing aggregate auxiliary allowance. Adapter
identity, shape and complete merge-work estimates, and exact index-label
coverage, are checked before tensor files are opened.

The loader conjoins the manifest's byte and read-call limits with the caller's
limits. One original `WeightReadBudget` spans every base shard and the adapter;
it is never replenished between files. The aggregate physical file-size ceiling
includes both base and adapter. The original adapter-specific parameter/header
ceiling also applies. A single extra byte in the reader budget permits the
original EOF probe; it does not increase the accepted physical input ceiling.

All explicit base and adapter files pass regular-file and aggregate-size checks
before base scalar reads begin. The original sharded reader checks all base
headers before their scalar bodies. The original adapter reader then validates
and applies the adapter after base loading. This is not an atomic snapshot of
every file: inputs and their parent directories must remain immutable and under
operator control. Observed final-component symlinks and nonregular files refuse.

The fully merged model must match the independently negotiated complete decoder
and cache profiles before fitting or probing begins. Only that effective model
can bind a resulting monitor. Physically repacking identical tensors does not
create a different model, while changing effective weights requires retraining
and a matching complete binding.

Publication intake uses the same prepared source and original decoder. Its
independently configured host actor must pin the final effective model and
tokenizer generations; a manifest cannot update those host pins. Its
existing aggregate asset allowance pays for the manifest, index and adapter
configuration as well as actual base and adapter reads, across the actor and
every native member. The model-specific and delivery-specific byte ceilings
still apply. A failed source retains its spent read work and cannot create a
durable run. Recovery compares the complete effective configuration, including
model weights, tokenizer, codec and generation policy. Equal physical repacking
is accepted; an effective parameter change refuses even if a selected prompt
would happen to produce the same output.

For offline training with a selected manifest, the report's `checkpoint_source`
record retains base file count and adapter parameter, updated-parameter and product reservations.
The input summary retains actual weight bytes and read calls. Loader errors
include the original work consumed before refusal. Adapter parameter and
updated-parameter counts describe the original merge footprint once work is
reserved; on a failed merge they do not claim every parameter was successfully
updated. No partial model or monitor is returned after an input failure.

## Scope and verification

This input route retains the existing finite decoder, supported Llama
configuration and tensor formats, static adapter targets, resource ceilings and
research acceptance criteria. It provides no dynamic adapter switching, online
fine-tuning, tensor-name discovery, remote downloading or new artifact format.
The existing native executable uses its own explicit worker configuration;
its indexed-base adapter route is documented in
[sharded LoRA native workers](SHARDED_LORA_NATIVE_WORKERS.md).

The training command regressions exercise all four single/sharded and
unadapted/adapted combinations. They connect actual codec output files to the
actual probe command and original operator monitor parser, compare complete
model binding and sampled behavior with an independently constructed effective
model, and remove only the final value adapter signal to check refusal while
the other three mandatory taps remain intact. Separate controls cover exact and
one-short byte/read-call allowances, auxiliary charging, pre-tensor refusals,
merge-work admission and unchanged legacy interpretation. Central execution on
qualified `nightly-2026-09-08` passes all 32 codec-example tests, including the
same 16 probe-command tests. A separate probe-example run also passes those 16
functions. These are 32 distinct regression functions, nine added for this
source route; they do not establish trained detector quality or complete project
qualification.
