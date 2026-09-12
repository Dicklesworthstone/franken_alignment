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

## Configuration negotiation and direct file consumer

The second increment adds `safetensors::pretrained::LlamaConfig`, `DecoderModel::from_llama_safetensors` and `DecoderModel::from_llama_files`. Core shape fields, `model_type=llama`, trained context and RMS epsilon are required. Configuration uses the original strict JSON parser under a 64 KiB/2,048-item/4 KiB-string limit. Unknown fields refuse rather than being silently dropped. The independently supplied DecoderIdentity is not obtained from an unauthenticated model-name string.

Supported optional behavior is explicit: missing/null KV-head count selects the documented MHA default; missing head_dim selects hidden/query-head width; missing theta selects 10,000; optional activation selects SiLU; bias and tying flags default false; tensor-parallel degree defaults one. All defaulted field names appear in LlamaConfig. Legacy `rope_theta` and the newer `rope_parameters` default-RoPE object are accepted, but conflicting declarations refuse. Scaled RoPE, partial rotary transforms, nonzero dropout, tensor parallelism, biases, tied heads, custom architecture/auto_map/quantization and other unknown fields are not approximated.

A narrow list of informational/training/format metadata fields is validated by type and recorded as ignored. Dtype labels do not override the actual tensor encodings or the reference binary32 arithmetic. BOS/EOS/padding metadata never inserts tokens, masks supplied IDs, or decides when generation stops. `use_cache=false` is an ignored storage hint, not a request to change the reference computation's numerical meaning. An explicitly selected execution context may be shorter than, but never longer than, the declared trained maximum. That does not relax the original parameter or cache bounds or certify tokenizer/position compatibility with another backend.

File loading takes TWO explicit paths and configurable lower byte ceilings. It validates configuration before opening the weight file and applies an additional shape-dependent weight bound. Selected symlinks/nonregular files, missing inputs, truncation and limit overruns refuse. Opened-file metadata is checked again and reads stop after at most limit+1 bytes. No directory traversal from a shard index, implicit file search, network download, pickle fallback, executable import or output write occurs. The operator must own both files and their ancestors and publish immutable versions; this is not a race-proof filesystem sandbox, authenticated checkpoint bundle, atomic read of a config/weight pair, memory mapping or sharded loader.

`examples/decoder_from_checkpoint.rs` is an offline file consumer. Its token-request JSON contains exactly the five declared identity integers and the original u32 token array. It checks the total prefix-plus-continuation context and product-term budget BEFORE inference, then uses the existing incremental decoder for an explicitly bounded number of greedy steps. It prints one JSON result only after all requested steps succeed; it does not decode tokens to text, invent special-token behavior, emit a partial tool call or construct any production authority.

After compilation in a qualified checkout, the retained synthetic example inputs can be consumed with:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference --example decoder_from_checkpoint -- \
  crates/fa-reference/tests/fixtures/decoder_llama_config.json \
  crates/fa-reference/tests/fixtures/decoder_mixed.safetensors \
  crates/fa-reference/tests/fixtures/decoder_original_tokens.json 16 8 1000000
```

The repository's required RCH wrapper and verifier-owned execution policy still apply to building/running this command. It was NOT executed here. The input files are synthetic fixtures, not trained-model evidence.

The second increment adds twelve public configuration/file tests (one Unix-only) and three example-parser tests. They cover the supported legacy/new RoPE declarations, explicit defaults, unsupported config refusal before weight access, ignored metadata without token insertion, trained-context ceilings, duplicates/truncation/limits, real temporary-file loading, exact/one-under file budgets, symlink/nonregular refusal, source deletion after import, complete original IDs and bounded CLI requests. Together with the first increment there are twenty-seven new Rust test functions, all UNEXECUTED. The successful independent Python SafeTensors fixture round-trip does not validate this Rust implementation. No existing tests, compiler gates, dependencies or production qualification were weakened.
