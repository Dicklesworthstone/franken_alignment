# Static plain-LoRA checkpoint ingestion

The original reference decoder can merge one explicitly supplied, complete
plain-LoRA adapter into a fresh immutable dense model. The original
`fa-native-helper` consumes that model through manifest `fa.native-worker/4`.
No alternate inference runtime or dependency is introduced.

This is a bounded import profile. All existing decoder shape, vocabulary,
context, tensor inventory, numerical and generation limits remain in force.
It does not admit production-sized models by raising those limits.

## Numerical and identity contract

For each declared projection, saved matrices have shapes
`A = [rank, input]`, `B = [output, rank]`. Each output weight is computed as:

```text
sum = 0.0_f64
for k in ascending order 0..rank:
    sum += f64(B[row, k]) * f64(A[k, column])
merged[row, column] = round_f32(f64(base[row, column]) + (alpha / rank) * sum)
```

The original finite f32 rounding function rejects nonfinite results. F32, F16
and BF16 adapter storage is normalized by the original SafeTensors scalar reader.
This defines the reference numerical result; it does not promise bit identity
with a framework performing the merge in another precision. Targeted signed
zeros can change through the documented addition even when the delta is zero.

The caller supplies a new `DecoderIdentity`. Tenant, model ID and tokenizer
generation must equal the base. **Both model generation and numerical profile
generation must strictly increase**, including for a zero delta. Shape,
epsilon, theta and the complete static rotary profile are preserved exactly.
The API performs no implicit generation increment.

The base model and its sessions remain owned and unchanged. The new model uses
the original `DecoderModel` constructor and a distinct immutable owner; no live
cache or checkpoint is edited. Old cache, capture and probe coordinates cannot
be reused under the new generations. Untargeted weights are copied; targeted
weights follow the equation above.

Declared IDs and a merge receipt do not authenticate parameter bytes or prove
training lineage. The host supplies the correct base and adapter. Existing
learned recipe bindings retain the complete merged parameter inventory, including
weights that do not affect the observed prompt. Existing sampled archives still
verify their own profile/history contract by original numerical recomputation;
they are not a replacement for full recipe parameter binding.

## Admitted PEFT export

The configuration is a strict JSON object, at most 65,536 bytes. This minimal
example requests both projections in every decoder layer:

```json
{
  "peft_type": "LORA",
  "task_type": "CAUSAL_LM",
  "inference_mode": true,
  "r": 2,
  "lora_alpha": 8,
  "target_modules": ["q_proj", "v_proj"]
}
```

Rank is an integer from 1 through 64. Alpha must be finite, positive, no greater
than 1,000,000, and must retain a nonzero alpha/r value. Targets are unique exact
short names: `q_proj`, `k_proj`, `v_proj`, `o_proj`, `gate_proj`,
`up_proj`, `down_proj`. Each selected target is required in **every** layer;
module regular expressions, selected-layer adapters and inferred targets refuse.
GQA key/value matrices use the existing cache width.

The exact saved names are:

```text
base_model.model.model.layers.N.self_attn.q_proj.lora_A.weight
base_model.model.model.layers.N.self_attn.q_proj.lora_B.weight
base_model.model.model.layers.N.mlp.down_proj.lora_A.weight
base_model.model.model.layers.N.mlp.down_proj.lora_B.weight
```

Substitute the declared projection and numeric layer. These are saved PEFT
keys without a runtime adapter name; `.lora_A.default.weight` refuses. Every
expected A/B tensor must exist with its exact shape, and extra tensors refuse.
The original header, coverage, finite-scalar and true-EOF checks apply.

Optional fields accept only the pinned plain-export meanings:

| Fields | Admitted value |
| --- | --- |
| `fan_in_fan_out`, `use_rslora`, `use_dora`, `lora_bias` | `false` |
| `bias` | `"none"` |
| `lora_dropout` | finite value in [0, 1); inactive during this declared inference merge |
| `init_lora_weights` | boolean or `"gaussian"` |
| `rank_pattern`, `alpha_pattern`, `loftq_config` | empty object |
| `modules_to_save`, `layers_to_transform`, `layers_pattern`, `megatron_config`, `layer_replication`, `exclude_modules`, `eva_config`, `auto_mapping` | `null` |
| `megatron_core` | `"megatron.core"` |
| `base_model_name_or_path`, `revision` | string or `null`; provenance label only |

Absent optional fields select these plain defaults. Unknown fields, PiSSA,
OLoRA, LoftQ and other initialization modes, DoRA, rank-stabilized scaling,
biases, trained embeddings/heads and extra saved modules refuse. Provenance
labels never select a path, trigger a download or authenticate a model.

## API and retained work

The public module is
`action::consequence::activation::tensor::kv::decoder::safetensors::lora`.

- `LoraConfig::decode` admits the configuration; `estimate(profile)` returns
  adapter scalars, updated dense parameters and planned B*A scalar products.
- `adapted_profile(base, identity)` checks the exact identity rule.
- `DecoderModel::with_lora_safetensors` consumes configuration and adapter byte
  slices plus a caller-owned `LoraMergeBudget`.
- `DecoderModel::read_lora_safetensors` consumes the same configuration, an
  already authorized reader, the original `WeightReadBudget`, and the shared
  merge budget. It returns a new model and `LoraMergeReceipt`.

At most 1,048,576 adapter scalars and 1,073,741,824 planned scalar products are
admitted. The original tensor-count ceiling also applies. The adapter file
ceiling is eight prefix bytes plus the original maximum header and four bytes
per admitted adapter scalar. A caller can choose a smaller work allowance.

Identity, configuration, inventory-size and aggregate product checks precede
any adapter read. Invalid input retains actual original read bytes/calls, but
reserves no products before scalar/EOF admission. After complete adapter
admission, the **entire planned product count is reserved before allocating or
computing the merged model**. Allocation failure, numerical overflow or later
consumer refusal cannot refund it. There is no automatic retry/reset.
An EOF probe needs a spare byte of remaining read allowance even when it returns
zero. Product counts describe B*A terms, not FLOPs, memory or elapsed time.

## Original native-helper consumer

`fa.native-worker/4` retains the explicit decoder rotary object and tokenizer
selection of version 3 and requires this additional root object:

```json
"adapter": {
  "base_identity": {
    "tenant": 1,
    "model": 2,
    "model_generation": 3,
    "tokenizer_generation": 4,
    "profile_generation": 5
  },
  "configuration": "/operator/adapter_config.json",
  "weights": "/operator/adapter_model.safetensors",
  "merge_products": 1000000
}
```

The root `decoder.identity` declares the adapted model, for example generations
6 and 7 with the same tenant, model and tokenizer. `files.weights` must be
`{"kind":"single","path":"/operator/base.safetensors"}`. Sharded version-4
manifests refuse during manifest parsing before opening assets. Versions 1–3
retain their previous contracts and do not accept an adapter object.

The public file consumer is
`NativeEvaluator::from_llama_lora_files`, with
`bootstrap::files::lora::NativeLoraFileBootstrap`. It takes independently
declared base and adapted identities. Base checkpoint configuration is loaded
under the base identity; tokenizer, monitoring and evaluation policy bind to the
adapted decoder. The full-input binding remains an independently provisioned
namespace; its epochs are not coerced to decoder generation numbers.

Adapter configuration shares the original auxiliary-asset allowance. Base and
adapter tensor reads share one original weight allowance. No original ceiling
increases. Identity and adapter work preflight precede weight-file opening; the
original model configuration, tokenizer, policy and sampling preflight also
precede both weight files. Monitoring admission completes before inference.

The original helper lifetime starts before salt/assets/base/adapter startup and
is retained by the same worker. Successful startup still computes no token.
The normal packet binding, monitored generation, complete categorical output,
commit and requested reveal are the only route to a vote. Host-controlled
immutable files/directories remain required by the original file-opening
contract; this does not claim atomic cross-file snapshots.

## Verification scope

New public tests are in `decoder_lora.rs` and `native_worker_lora.rs`.
Their small synthetic checkpoint/adapter fixtures exercise the original paths;
no trained artifact is claimed to have been exercised. They specify rank-one/rank-two independent matrix and query oracles, mixed
storage types, original inference and replay, zero-delta coordinate refusal,
unused-parameter recipe binding, retained read/product spending, and actual
original-helper socketpair commit/reveal with causal verdict changes.
Matching inputs accompany malformed configuration, stale identity, budget,
incomplete output and monitoring refusal cases.

These additions were authored and reviewed while the executor was unavailable;
they had not been executed at authoring time. Subsequent validation on 2026-10-10
with qualified nightly-2026-09-08 passes all seven `decoder_lora` tests and all
six `native_worker_lora` tests, including actual helper-process commit/reveal.
The helper refusal oracles now name the original model-identity and input-profile
checks at which rejection actually occurs, retaining zero-work and full-input
assertions. These synthetic execution results do not establish production-model
quality or deployment qualification. The unchanged full project gate still
refuses its reviewed source snapshot before later workspace checks.

## Primary compatibility references

The narrow data contract is pinned to PEFT v0.14.0:

- [LoRA layer and delta equation](https://github.com/huggingface/peft/blob/v0.14.0/src/peft/tuners/lora/layer.py):
  plain A/B orientations and alpha/r scaling.
- [LoRA configuration](https://github.com/huggingface/peft/blob/v0.14.0/src/peft/tuners/lora/config.py):
  optional defaults and distinct unsupported variants.
- [Adapter save/load format](https://github.com/huggingface/peft/blob/v0.14.0/src/peft/utils/save_and_load.py):
  plain bias-free export and removal of the runtime adapter name from saved keys.
