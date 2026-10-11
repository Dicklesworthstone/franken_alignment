# Checkpoint files to original learned K/V fitting artifacts

`train_learned_kv_codec_from_checkpoint` produces the two original codec inputs
required by the [K/V probe training command](TRAIN_LEARNED_KV_PROBES_COMMAND.md)
and [learned publication operator](LEARNED_OPERATOR_PUBLICATION.md): an
independently retainable `fa.learned-fit-binding/1` JSON file and the original
`FAKVFIT/1` archive. It also saves a bounded diagnostic report containing the
declared token corpus and actual original capture/fitting work.

The original invocation imports an explicitly supplied Llama configuration and
one complete SafeTensors weight file. An explicit `--checkpoint-source` switch
also admits a complete indexed shard set, a static saved LoRA adapter, or both
through the [shared checkpoint source contract](CHECKPOINT_SOURCE_MANIFEST.md).
It drives `DecoderModel::begin_codec_training`,
`CodecTraining::advance` and `finish`. Every training document starts in its own
empty original decoder cache. The owner captures the complete original K/V
images, runs the existing centered-covariance/Jacobi fitter, and returns its
verified fitting checkpoint. The command does not accept supplied cache arrays,
fitted coefficients, saved success records, or detector labels.

This implements the operator entry point for the existing plan §10.5 linear/PCA
baseline and §§10.15–11 replayable sidecar workflow (FA-029/FA-113; FI-I03,
FI-I07). It changes no fitting arithmetic, archive interpretation, monitor
decision, effect authority or research acceptance criterion.

## Invocation

From a repository build:

```sh
cargo run --locked -p fa-reference --example train_learned_kv_codec_from_checkpoint -- \
  /operator/model/config.json \
  /operator/model/model.safetensors \
  /operator/codec-training.json \
  /operator/new-codec-binding.json \
  /operator/new-codec.fakv \
  /operator/new-codec-report.ndjson
```

There are exactly six positional arguments, with one optional leading switch.
With `--checkpoint-source`, the second operand names a
`fa.checkpoint-weights/1` source manifest instead of a weight file:

```sh
cargo run --locked -p fa-reference --example train_learned_kv_codec_from_checkpoint -- \
  --checkpoint-source \
  /operator/model/config.json \
  /operator/model/checkpoint-source.json \
  /operator/codec-training.json \
  /operator/new-codec-binding.json \
  /operator/new-codec.fakv \
  /operator/new-codec-report.ndjson
```

Without that switch the original weight-file interpretation is unchanged. File
extensions, contents, model names and failed parsers never select another mode.
The training-plan schema and every existing field retain their meaning.
All paths are explicit operator
inputs. Inputs must be complete regular files; observed final-component symlinks
and nonregular files refuse. Paths and parent directories must remain under
operator control during the command. Metadata checks do not authenticate an
archive or guarantee resistance to adversarial path replacement.

The three output paths must be distinct and new. Existing files are never
overwritten. Output files use exclusive creation and mode `0600` on Unix.
Successful writes are synced. The three files are separate writes, so a later
failure can leave a complete binding and an incomplete or absent archive. A
nonzero exit does not delete these files or repeat fitting. Retain the diagnostic
report and choose new output paths for an explicitly requested new run.

Stdout contains bounded progress and completion records, each with
`"permission":"not_issued"`. Exit zero means fitting completed and the two
artifacts, report and final status writes succeeded. Every other outcome exits
nonzero. Closing the progress sink during capture cancels the original owner
before further numerical work; its partial report retains both attempted and
unstarted documents. There is no interrupted-fit continuation or automatic retry.

## Training plan: fa.learned-kv-codec-training/1

Every field below is required. Unknown or duplicate fields refuse. Counts,
identities and token IDs are unsigned integers. This is a token-document
interface: no tokenizer, prompt template, normalization, or text-to-token guess
is applied. The `tokenizer_generation` is an explicit model-profile declaration;
the command cannot infer which tokenizer produced the supplied IDs.

| Object | Required fields |
| --- | --- |
| Root | `schema`, `identity`, `context`, `input_limits`, `output_limits`, `policy`, `budget`, `documents` |
| `identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation` |
| `input_limits` | `auxiliary_bytes`, `weight_bytes`, `weight_read_calls` |
| `output_limits` | `binding_bytes`, `archive_bytes`, `report_bytes` |
| `policy` | `id`, `generation`, `rank`, `sweeps` |
| `budget` | `tokens`, `source_values`, `decoder_products`, `fitting` |
| `budget.fitting` | `source_values`, `parameter_values`, `scratch_values`, `work_units` |
| Each `documents` entry | `origin`, `stream`, `tokens` |

For example, the following describes two tokens for a small two-layer model.
The IDs and numerical limits must be chosen for the actual independently
provisioned checkpoint and corpus; this example is not a pretrained-model
configuration or a useful research dataset.

```json
{
  "schema": "fa.learned-kv-codec-training/1",
  "identity": {
    "tenant": 1,
    "model": 2,
    "model_generation": 3,
    "tokenizer_generation": 4,
    "profile_generation": 5
  },
  "context": 8,
  "input_limits": {
    "auxiliary_bytes": 1048576,
    "weight_bytes": 1048576,
    "weight_read_calls": 10000
  },
  "output_limits": {
    "binding_bytes": 1048576,
    "archive_bytes": 1048576,
    "report_bytes": 1048576
  },
  "policy": { "id": 1, "generation": 1, "rank": 1, "sweeps": 8 },
  "budget": {
    "tokens": 2,
    "source_values": 16,
    "decoder_products": 1000000,
    "fitting": {
      "source_values": 16,
      "parameter_values": 16,
      "scratch_values": 16,
      "work_units": 1696
    }
  },
  "documents": [
    { "origin": 1010, "stream": 1000, "tokens": [0, 1] }
  ]
}
```

Origins and streams must be unique and nonzero. At most 64 documents and
16,384 total original token rows are admitted; every document is nonempty and
fits the actual decoder context and vocabulary. The complete corpus must have
at least two rows. Documents are processed in canonical origin order. Repeating
a source under another origin does not establish statistical independence, and
the source-ID checks do not authenticate the documents' provenance.

## Admission and work accounting

The plan is parsed before checkpoint access. Configuration negotiation precedes
weights. Before opening the weight file, the command admits all of the following:

- The sum of original per-document inference work, using
  `DecoderProfile::estimate`, and every token/context/vocabulary bound.
- The complete captured K/V value count and retained original-image bytes.
- All fitting source, parameter, scratch and fixed-loop work dimensions, using
  `LearnedKvPolicy::estimate_fit`. The original fitter delegates to this same
  arithmetic; the command does not use a second numerical cost model.
- Conservative complete binding, archive and diagnostic-report reservations.
  These include descriptors, source data, parameters, comparison material,
  per-group diagnostics and framing. A allowance equal to a reservation is
  sufficient; a smaller allowance refuses before weights even when a particular
  final encoding would be smaller than that conservative reservation.

`DecoderProfile::cache_profile` derives the exact original cache-contract
metadata without allocating parameters or captured values. The imported model
must equal the negotiated decoder profile and derived cache profile, including
the original identities and GQA layout. Static RoPE remains in the decoder
profile and actual inference; the existing cache profile does not gain a new
RoPE identity field implicitly.

`auxiliary_bytes` bounds the sum of training-plan and configuration bytes.
When a checkpoint source is explicitly selected, that same allowance also pays
for its manifest, shard index and adapter configuration. The manifest's byte
and read-call limits are conjoined with the existing `weight_bytes` and
`weight_read_calls`; every base shard and the adapter share those single
allowances. Metadata, complete physical tails and the original scalar reader
retain their existing checks. A new file never receives a fresh quota.

Source preparation validates the exact shard inventory and original LoRA
configuration/work before any tensor path opens. The plan's identity names the
effective model. An adapter supplies a separate base identity, with the original
strict increase in model and numerical-profile generations. The original static
merger alone constructs the effective model; its manifest-declared product
allowance is neither inference nor codec-fitting work. Its reservation remains
spent if merging fails. A source-load error retains its actual reader/merge work
and produces no training artifact. Capture begins only after the complete
effective model matches the independently negotiated original profile.

Weights have their original independent byte/read-call allowance, including
the original reader's complete-tail check. Binding JSON retains the existing
2 MiB consumer ceiling. Fitting inputs retain their existing 96 MiB logical
image ceiling, and comparison material its existing 16 MiB ceiling. The full
archive retains `MAX_FIT_ARCHIVE_BYTES`; no archive or model ceiling is enlarged.
The command bounds its training plan to 4 MiB and its report to 64 MiB.

The original owner admits the whole corpus, then reserves each original token
attempt before execution. A failed token may have performed work absent from
its successful-work counters; `all_inference_reported` remains false in that
case. Fitting is one bounded synchronous call and is not preemptible between
Jacobi sweeps. No failure, cancellation, diagnostic error or later artifact
write replenishes these allowances. Logical bytes and work counts are not
latency, peak memory, a convergence proof or an alignment result.

## Files and existing consumers

The binding JSON uses the unchanged `fa.learned-fit-binding/1` schema: selected
policy, original fitting caps, and complete original source descriptors keyed
by origin. The command checks its output through the existing learned operator's
binding parser before writing it. Descriptors come from the completed original
checkpoint; planned metadata cannot stand in for a captured source.

The archive comes directly from `LearnedKvFitCheckpoint::encode_archive`.
Its source payloads and opaque comparison witness retain the original format.
Reading it with `LearnedKvFitArchive` produces an unverified input object; only
the original `replay` fitter can produce a usable codec after comparing every
result word. This repeats the fitting work and does not load saved coefficients
as executable model state. See [the fitting replay contract](LEARNED_FIT_REPLAY.md).

Pass the new binding and archive to the corresponding arguments of
`train_learned_kv_probes_from_checkpoint`. That command still requires separate
training, calibration and held-out probe cases, and rejects overlap with the
codec's declared origins or source streams. Its accepted `FAKVMON2` monitor binds
the actual model, tokenizer, fitted codec, probes and operational settings.
The learned publication recipe can then reference all three original assets
through `files.fit_binding`, `files.fit_archive` and `files.monitor`.

The report records the actual decoder identity, shape, static RoPE parameters,
input read counts, fixed policy and budgets, every supplied token document,
preflight reservations, completed and unstarted document work, original fit
statistics and exact binary64 diagnostic words, and artifact write outcomes.
For an explicitly selected checkpoint source, a `checkpoint_source` row also
records admitted base-file count and the original reserved adapter-parameter,
updated-parameter and merge-product footprint. The report does not embed the
manifest, base weights or adapter; retain those independently selected inputs.
It preserves failed or cancelled numerical outcomes separately from completed
fitting. None of these records supplies a monitor decision, helper vote, human
key, live decoder owner, effect permit, or authenticated training provenance.

## Verification scope

Twelve new example tests are authored. The principal positive creates actual
codec files from an imported two-layer GQA checkpoint, passes those same files
through the existing probe-training command, and loads its resulting monitor
with the actual publication operator parser. An independent original sampled
decoder supplies the next-token oracle: the quiet sample reaches an audited
stop, while the matched violating sample retains the original cache, logits
and random state and exposes no candidate output.

Other tests compare the emitted archive against independently recomputed
training images, check per-document empty-cache boundaries and completed fit
work against the original estimate, underfund every work/output dimension
before missing weights, alter real training tokens under unchanged descriptors,
challenge the old comparison witness and model identity, reject codec/probe
origin overlap, retain numerical and fitting failures, cancel on a closed
progress sink, preserve existing/raced outputs, reject incomplete or nonregular
inputs, and exercise static RoPE with the actual imported model.

Four additional source-selection regressions exercise all four single/sharded
and absent/nonzero-adapter combinations through actual codec files, the actual
monitor command and original operator binding with independent sampled
quiet/hold oracles. They also alter only the final value adapter, underfund
either manifest or caller byte/read-call limits, underfund the same auxiliary
and merge allowances, reject metadata before missing tensor paths, and retain
the original unselected-file behavior. The nonzero rank-two adapter restores an
independently specified model from a base with half-sized K/V projections; the
test oracle never merges the adapter itself.

Central execution on qualified `nightly-2026-09-08` passes all 32 tests in the
codec example, including the same 16 probe-command regression functions. Running
the probe example separately also passes all 16; those repeated functions are
not counted as additional independent tests. Nine of the 32 functions exercise
the new manifest route (four codec and five probe tests).

These are synthetic functional controls. They do not establish detector quality,
source authenticity, independent labels or deployment qualification. The original
complete project gate remains a separate requirement; its reviewed source
snapshot and enforcement are unchanged.
