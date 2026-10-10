# Static RoPE in the original native decoder

`DecoderProfile::with_rotary_scaling` adds explicit linear and Llama3 positional
frequency scaling to the existing Llama/SafeTensors inference path. This is L7
reference progress toward FA-025/FA-026, serving FI-I05/FI-I07 and the exact
model/profile bindings in FA-INV-011/017. It introduces no serving backend,
dependency, authority, new journal, or alternate checkpoint-restoration path.

## Numerical meaning

The public immutable `RotaryScaling` variants are `None`, `Linear` and `Llama3`.
Use `RotaryScaling::linear(factor)` or
`RotaryScaling::llama3(factor, low_freq_factor, high_freq_factor,
original_max_position_embeddings)` to validate their private parameters, then
freeze the result into a decoder profile before constructing the model. Factors
must be finite and at least one; Llama3 requires positive ordered frequency
factors and a positive original context. Every resulting inverse frequency must
remain finite and positive at profile admission, including underflow checks.

For pair index `i` and head width `d`, the original inverse frequency is
`f = theta^(-2i/d)`. Linear scaling uses `f/factor`. Llama3 compares wavelength
`2π/f` to `original_context/high_freq_factor` and
`original_context/low_freq_factor`: short wavelengths keep `f`, long wavelengths
use `f/factor`, and the middle band interpolates between those frequencies.
Both threshold boundaries belong to the middle band. The interpolation weight is
`(original_context/wavelength - low_freq_factor) / (high_freq_factor - low_freq_factor)`.

The original kernel applies the resulting angle to its existing half-split Q/K
pairs, with the same f64 arithmetic, f32 boundaries, GQA mapping, complete causal
attention and capture ownership. Frequencies are static throughout a cache's
lifetime. The definitions were checked against the pinned
[Transformers v4.50.0 RoPE implementation](https://github.com/huggingface/transformers/blob/v4.50.0/src/transformers/modeling_rope_utils.py).
This does not promise PyTorch/GPU bit identity. The native arithmetic contract
remains authoritative. `None` retains the original calculation exactly, while a
factor-one declaration still has distinct profile and archive identity.

Dynamic NTK, YaRN, LongRoPE, partial rotary transforms and attention-amplitude
rescaling remain unsupported. No configuration is approximated by another mode,
and an existing live model or cache cannot be rebound to different frequencies.

## Checkpoint configuration

`LlamaConfig::decode` admits the static modes through either the legacy
`rope_scaling` object or a flat modern `rope_parameters` object. Legacy scaling
accepts `type` as the alias for `rope_type`; simultaneous aliases must agree.
Modern parameters also require their explicit `rope_theta`. If a legacy theta
or a second scaling declaration is present, its numerical bits and mode must
agree exactly. Unknown fields, missing static parameters, contradictory
declarations and unsupported modes refuse before any weight reader is consumed.

The original context named by Llama3 must not exceed the configuration's declared
`max_position_embeddings`. An independently selected execution context may be
shorter than that original context, but must still fit the declared maximum and
all existing decoder limits. Scaling does not enlarge the admitted shape,
vocabulary, parameter, cache or product ceilings. In particular, the existing
16,777,216-parameter and 65,536-token vocabulary limits remain; accepting Llama3's
frequency formula does not admit production-sized Llama3 checkpoints.

## Independently configured helper executable

The original `fa-native-helper` executable accepts `fa.native-worker/3` to pin
these semantics independently of the checkpoint. It retains /2's mandatory
`tokenizer_format`, explicit single/sharded weight object, original asset/work
allowances, salt, inherited private socket and one worker lifetime. Its `decoder`
object additionally requires exactly one explicit `rotary` object:

```json
{"kind":"none"}
```

```json
{"kind":"linear","factor":4}
```

```json
{"kind":"llama3","factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":8192}
```

These are decoder-field fragments, not complete worker manifests. /1 and /2
retain their exact original field sets and unscaled meaning; they reject an
unexpected `rotary` field. Changing only a schema number cannot add or remove the
required /3 declaration. The same validated public constructors admit the
operator profile, and the original native bootstrap compares it to the imported
Llama configuration before weight or index access. Native tokenizer archives must
also bind that exact profile. A declaration cannot silently override a changed
checkpoint, downgrade to unscaled, or select a different tokenizer parser.

`native_worker_rotary` launches the actual binary over its original socket with
input-dependent synthetic models. It checks both static modes, native/JSON
tokenizers, single/sharded sources, legacy manifests, exact commit/reveal,
profile mismatches before missing weights, held/incomplete output and retained
step exhaustion. These numerical fixtures test consumer integration and
configuration custody, not trained-model calibration or hostile-process isolation.

## Interchange and replay

Every existing owner binds the complete scaling declaration. Unscaled archives
retain their original bytes. Scaled modes carry a fixed 40-byte extension: five
big-endian u64 words for mode, factor bits, low-frequency-factor bits,
high-frequency-factor bits and original context. Linear mode requires zero in
all unused words. The extension cannot encode `None`; that has only its original
legacy representation. Parameter bytes are compared before numerical replay or
tokenizer vocabulary allocation where an independent profile is supplied.

| Original owner | Scaled representation |
| --- | --- |
| Sampled checkpoint archive | `FASDCP` version 2; extension after the original 84-byte profile |
| Byte-BPE archive | `FABBPE03`, or `FABBPE04` with named controls; extension after the original 120-byte header |
| Learned-generation recipe | `FALGRCP` version 2; extension after theta |
| Monitored sampled replay | `FANREP` version 2; extension after theta |
| Durable decoder configuration | Reserved invalid tenant zero, `FAROPC` version 1 and the extension, followed by the original body |

The durable envelope composes with the existing independent, tied-head and
sharded event discriminators. Older readers reject scaled representations;
legacy readers are never asked to reinterpret a changed numerical profile as the
old one. The additional metadata consumes the original byte budgets rather than
raising them. Complete model/profile equality, original parameter custody, fresh
recovery fences and the existing pause/resume rules remain mandatory.

The public `decoder_rotary_scaling` harness exercises imported Q captures against
an independent scalar oracle spanning all Llama3 wavelength regions, actual later
attention differences, original-token continuation, stochastic archive replay,
tokenizer and learned-recipe bindings, and durable recovery through publication.
Malformed configurations are paired with admitted static counterparts, and
profile substitutions are tested even when observed tensors would match. Exact
executed checks and remaining qualification limits are recorded in
`IMPLEMENTATION_STATUS.md`; a parsed model is neither authenticated nor a claim
of trained-model usefulness or empirical detector quality.
