# SafeTensors parameters for the original-token decoder

The consumer is `DecoderModel::from_safetensors` in `activation::tensor::kv::decoder::safetensors`. It turns actual supplied checkpoint bytes into the existing immutable DecoderModel, then uses the original inference, captured attention, checkpoint restore and recomputation paths. This is FA-025/FA-026 reference progress in L7, serving FI-I05 and FI-A05. It adds no inference runtime, executable deserializer, Cargo dependency, model downloader or production authority.

## Interpretation contract

The caller supplies the existing validated DecoderProfile. The loader recognizes one explicit Hugging Face Llama tensor-name layout: `model.embed_tokens.weight`, `model.norm.weight`, `lm_head.weight`, and nine named parameters per layer (input/post-attention norm, Q/K/V/O, gate/up/down). Every required tensor must be present, including the separate vocabulary head. Missing norms, inferred transposes, biases, extra adapter/rotary tensors, implicit weight tying and alternate naming schemes refuse. This is not an architecture guesser or universal Llama checkpoint loader.

The SafeTensors framing is the documented little-endian u64 header size, bounded UTF-8 JSON object, and row-major little-endian data. Tensor offsets are relative to the data buffer. All shapes and descriptor fields, the exact roster, and complete gap-free/nonoverlapping data coverage are checked before any scalar decoding. Header ordering and physical tensor order need not match. The optional `__metadata__` entry must contain only string values; it is informational, not a source of numerical defaults or authentication. Duplicate keys use the original strict JSON parser's refusal.

Only F32, F16 and BF16 are admitted. Half-width finite values expand exactly through the existing tensor capture decoder, preserving signed zero and subnormals. Nonfinite parameters refuse; the SafeTensors format itself permits them, but the original numerical model does not. Scalars, zero-extent tensors, higher-rank arrays and other dtypes are outside this decoder's required inventory. A shape with the correct flat count but wrong axes refuses. No missing tensor is filled with zeroes.

A load returns an ordinary model plus a WeightLoadReceipt retaining the exact decoder profile, tensor shapes and encodings, file/header/data bytes and normalized binary32 parameter bytes. These are data and interpretation counts, not a signature, digest, host attestation, training claim or measured peak memory. Loaded weights obey the original reference arithmetic profile, not a promise of PyTorch/GPU bit identity. The original tokenizer IDs remain caller-supplied; the loader does not tokenize, insert BOS, interpret EOS or execute remote model code.

## Bounds and publication

The header is capped at 1 MiB, each JSON string at 4 KiB, and tensor inventory at 1,155 (nine per supported layer plus three). Shape-dependent parameter and KV limits remain the existing decoder's, including 16,777,216 normalized scalar parameters. The maximum single file is 68,157,448 bytes; actual data must also fit four bytes per requested parameter. These bounds do not admit typical billion-parameter checkpoints. Smaller compatible trained models can be supplied; none is bundled or qualified here.

The parser borrows scalar payload slices during validation. It allocates only the bounded parameter vectors needed for the admitted model and their descriptor metadata. A failure in the final scalar returns no partially usable model and cannot change an earlier model, session or permit. Earlier temporary allocations and computation are not rolled back or presented as free work. Input bytes, parsed header strings, expanded arrays and the original model's validation/capture metadata coexist; logical byte counts are not total allocator/RSS measurements. Allocation aborts are outside Result-level recovery.

Cloning a loaded model shares the original immutable parameter object. Loading another file with identical names, values or numerical identities creates a different object and cannot restore the first model's checkpoint. A valid file is not authenticated; finite payload edits can change the resulting model. The host must pin and authenticate model/configuration/tokenizer provenance independently.

## External format references

The framing follows [SafeTensors' published format](https://github.com/huggingface/safetensors/blob/main/README.md#format). Tensor roles and axes were checked against the [Transformers v4.50.0 Llama implementation](https://github.com/huggingface/transformers/blob/v4.50.0/src/transformers/models/llama/modeling_llama.py). No source from either dependency is linked into the product. The narrow reference numerical contract remains authoritative over arithmetic, supported architectures and resource limits.

## Tests and qualification

The first increment adds twelve public Rust tests. Positive controls load complete binary32 and mixed-precision weights, compare original-token logits and all cache bytes to an independently constructed model, and restore/continue an imported model for eight greedy steps. Negatives cover all truncations of a selected archive, unindexed suffix, missing output/norm/layer weights, extra bias and alternate names, same-count transposes, malformed/overlapping offsets, duplicate fields, nonstring metadata, unsupported dtypes, oversized headers, and nonfinite late parameters. A malformed final shape must be rejected before an earlier NaN is decoded, checking whole-roster preflight rather than merely eventual rejection.

`tests/fixtures/decoder_mixed.safetensors` was produced independently by the installed `safetensors.torch.save` (0.7.0; Torch 2.10.0+cpu), using deterministic synthetic exact-eighth parameters. Its 21 tensors, 308 parameters, all shapes and all scalar values were round-tripped through that library during editing. `decoder_mixed.provenance.json` records its SHA-256 and counts. This is format-fixture evidence, not Rust execution, trained-model performance, numerical-equivalence evidence or a new product dependency. The fixture is consumed by the Rust loader test and is not regenerated by the test to force agreement.

All twelve Rust tests remain UNEXECUTED. Cargo/rustc/rustfmt and the required RCH runner are unavailable in this editing environment. Compilation, Clippy and the full revision-bound gate remain pending. No Beads task or production qualification is closed, and historical gate receipts do not qualify these changes.
