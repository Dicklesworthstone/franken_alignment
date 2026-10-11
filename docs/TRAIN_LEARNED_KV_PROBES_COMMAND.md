# Train a complete learned K/V monitor from checkpoint files

The offline `train_learned_kv_probes_from_checkpoint` example turns independently
selected model, tokenizer, labelled cases and codec training inputs into a
complete evaluated K/V monitor that the learned publication operator can load.
It also saves the diagnostic results when a campaign fails its statistical
criteria. Every model layer and both key/value sides are mandatory.

The command uses the original Llama configuration and streamed Safetensors
readers, original byte BPE, original verified codec refit, `KvDecoderCorpus`,
optimizer, calibrator and exact held-out scorer. It adds no numerical backend,
tokenizer interpretation, dependency or effect authority. This is an operator
entry point for the L7 experiment work in plan sections 10.2, 10.7, 10.13, 10.15
and 12.6, serving the FA-110 signature/probe work and FA-113 learned-monitor path.
It does not close their generalization or alignment research obligations.

## Invocation

Build and run with the repository's qualified toolchain and verification flow:

```text
train_learned_kv_probes_from_checkpoint \
  CONFIG_JSON WEIGHTS_SAFETENSORS TOKENIZER CAMPAIGN_JSON \
  CODEC_BINDING_JSON CODEC_ARCHIVE NEW_REPORT_NDJSON NEW_MONITOR_ARTIFACT
```

All eight arguments are explicit local file paths. The final two paths must be
new and distinct. The program never replaces, truncates or removes an existing
output. There is no model-name lookup, download, label inference, automatic data
split or automatic promotion of the resulting monitor.

| Input | Required meaning |
| --- | --- |
| `CONFIG_JSON` and `WEIGHTS_SAFETENSORS` | The existing supported single-file Llama numerical profile and its actual F32/F16/BF16 weights. Static supported RoPE and explicit tied-head loading retain their original semantics. |
| `TOKENIZER` | Explicitly selected native byte-BPE archive or supported Hugging Face raw ByteLevel JSON, bound to the complete independently negotiated decoder profile. |
| `CAMPAIGN_JSON` | Closed `fa.learned-kv-probe-campaign/1` data: identities, complete labelled population, all tap fitting/calibration rules, original resource budgets and output caps. |
| `CODEC_BINDING_JSON` | The existing `fa.learned-fit-binding/1` schema: exact fit policy, original fit budget and complete independently retained source descriptors. The command uses the same parser as the publication operator. |
| `CODEC_ARCHIVE` | Original `LearnedKvFitCheckpoint::encode_archive` bytes. Parsing yields no usable codec; the command reruns the original fit and compares its complete result before use. |

The command does not take a supplied probe coefficient or selected threshold.
Those values come only from the accepted original fit/calibration/evaluation.
The codec is independently prepared; it is not co-trained on this command's
probe cases. Declared codec source keys must not overlap probe lineages, and
codec source streams must not overlap probe tasks. The exact original cache
profile must match every descriptor.

## Campaign data

The root object requires every field below and refuses unknown fields. All
identities are nonzero integers. There is no selection by filename or parser
fallback.

| Field | Contents |
| --- | --- |
| `schema` | Exactly `fa.learned-kv-probe-campaign/1`. |
| `id`, `generation` | Original corpus identity and generation. |
| `identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation`. These are declarations, not authentication of a checkpoint. |
| `context` | Explicit original decoder context, positive and within the configured trained context and unchanged global ceiling. |
| `tokenizer_format` | `native_archive` or `huggingface_raw_bytelevel`. |
| `input_limits` | `auxiliary_bytes`, `weight_bytes`, `weight_read_calls`, `codec_bytes`, `codec_read_calls`. |
| `output_limits` | `report_bytes`, `monitor_bytes`. |
| `capture_budget` | `cases`, `original_tokens`, `kv_coordinates`, `scalar_products`. |
| `campaign_budget` | `training_visits`, `scoring_bytes`, `scoring_coordinates`, `threshold_comparisons`. |
| `monitor` | Original operational settings described below. |
| `cases` | Complete original-token population, including both classes in all three splits. |
| `taps` | Exactly one entry for every model layer and both `key` and `value`. |

A case has this exact shape:

```json
{"task":1,"lineage":101,"split":"training","label":"benign","tokens":[4,0]}
```

Allowed splits are `training`, `calibration` and `evaluation`; labels are `benign`
and `violation`. Tokens are nonempty original IDs from the selected tokenizer,
within its vocabulary and the requested context. Tasks, lineages and exact token
histories are each unique across the entire population. The two-token example is
synthetic schema data, not a useful detector dataset or an authenticated label.

Each tap requires `layer`, `side`, `fit`, `calibration` and `monitor_budget`.
`fit` has the original `id`, `generation`, `epochs`, `learning_rate`, `l2` and
`scale_floor`. `calibration` has `id`, `generation`, an explicitly ordered
`thresholds` array, and separate `calibration` and `evaluation` criteria objects;
each criteria object contains `min_violation_alarms` and `max_benign_holds`.
The original policy constructors enforce their existing bounds. Calibration
cannot inspect final labels or change its candidates after scoring them.

The `monitor` object requires `retention` (`all` or `none`),
`inference_products`, `preparation` and `audit`. `preparation.compression` contains
`source_values`, `encoded_bytes` and `work_units`; `preparation.source_check`
contains `source_values`, `encoded_bytes` and `reconstruction_products`.
`audit` contains `rows` and `budget`. Its budget and every tap's `monitor_budget`
have the original five fields: `encoded_bytes`, `probe_coordinates`,
`reconstruction_products`, `materialized_values` and `refinements`.

The real regression fixture at
`crates/fa-reference/examples/train_learned_kv_probes_from_checkpoint/fixture.rs`
constructs one complete four-tap campaign, actual checkpoint tensors, tokenizer,
independent fit binding and original codec archive. Its tiny weights and labels
are deterministic contract tests; copying their criteria is not detector
qualification for another model or task.

## Admission, spending and results

The entire campaign schema is parsed first. Original configuration negotiation
then fixes the numerical shape. The same `DecoderProfile::estimate` called by
`DecoderModel::estimate` admits every original prompt token before weight I/O.
All fitting, calibration and final-scoring work and a conservative complete
diagnostic size are checked at this point. A one-unit deficit in any campaign
dimension refuses before a missing weight file would be opened.

The tokenizer binds before weights open. Weights and codec inputs use the
original readers, with persistent read-byte and read-call budgets; EOF probes
and attempted interrupted reads keep their original accounting. The shared
auxiliary allowance includes the physical campaign, configuration, tokenizer
and fit-binding bytes. Each auxiliary file read also has a fixed 65,536-attempt
ceiling. These are logical I/O and numerical counts, not wall-time deadlines,
allocator measurements or deployment latency guarantees.

The new report is opened exclusively after complete input admission and before
codec refit or probe capture. It first records the fixed population, policies,
caps and actual input work. The original capture owner spends its complete
allowance before execution. The original campaign then rechecks all estimates
and reserves every mandatory tap's fit, calibration and final evaluation. A
failed calibration keeps its planned final denominator and reserved cost while
marking final evaluation absent. Later taps still run. A hard numerical failure
returns no partial campaign; the report retains the fixed population and actual
remaining allowances instead of invented scores. The same `Plan` cannot retry
an admitted capture or acquire fresh work after failure.

Successful capture reporting comes from the original corpus's actual retained
`work()` value. The report includes remaining capture/campaign allowances and
separate original admitted/completed fitting/scoring work. A failed final
evaluation keeps its original six-cell counts. Equality remains a boundary hold;
it is not silently counted as quiet or as a demonstrated violation alarm.

## Diagnostic file and bound monitor

`NEW_REPORT_NDJSON` uses `fa.learned-kv-probe-report/1`. It retains all planned
cases, all tap policies, fitted coefficients and normalization words, calibration
scores and threshold trials, original source identities, final scores/counts,
planned class denominators and resource usage. Exact scores use the original
signed-magnitude words in units of `2^-298`; coefficient and threshold fields
preserve binary32 bits. No pooled average hides a failed tap.
Keep the original input files with this report: its diagnostic rows do not embed
the full checkpoint weights, tokenizer archive or codec source tensors and are
not a self-contained replay capsule. The report retains the declared numerical
profile, including every supported static RoPE parameter.

Only a completely accepted roster can produce `NEW_MONITOR_ARTIFACT`. It uses
`FAKVMON2`, containing the original monitor JSON plus exact canonical tokenizer
and original model/codec/policy comparison bytes. Set the learned publication
recipe's `files.monitor` path to this artifact. Its loader constructs the original
policy through the strict parser, then compares its complete binding with the
independently supplied assets before generation or store creation. Changing an unused
weight, tokenizer control spelling or an operational cap does not preserve that
binding merely because model names or tensor dimensions stay the same.

The complete bound artifact has the existing 16 MiB ceiling. An imported model
that fits the original numerical limit can still exceed this portable artifact
limit; export then fails with its completed diagnostic report retained. The
program does not replace complete identity with a shorter or weaker identifier.
Reports are independently capped at 64 MiB. A successful file sync concerns the
file; these two outputs are not one transactional pair or a power-loss-tested
directory publication. A write/sync failure can leave a partial new file, which
the command retains and reports rather than deleting or silently retrying.

| Exit | Result |
| --- | --- |
| `0` | Every tap passed; the report and new bound monitor were written and synced; status output succeeded. |
| `2` | Statistical rejection; the complete available report was saved, and no monitor was created. |
| `1` | Configuration, numerical, I/O, serialization or status-output failure. Any admitted work remains spent and any created diagnostics/artifact remain at their actual paths. |

A broken status sink before monitor creation prevents creating the monitor while
leaving the completed diagnostic report. A later output failure may occur after
a new monitor exists; exit `1` does not claim that no bytes were written. A
concurrent output creation is caught by the final exclusive open, with neither
replacement nor refit.

## Verification and interpretation

The targeted command is:

```sh
cargo test --locked -p fa-reference --example train_learned_kv_probes_from_checkpoint -- --nocapture
```

Ten regression functions exercise real checkpoint/config/codec/tokenizer files,
actual original fitting and exact scoring, the publication operator's actual
artifact loader and real original sampled quiet/hold paths. Paired failures
change only final labels or only the last value projection. Others cover every
one-unit budget deficit, a false fit witness, duplicate origins/history,
tokenizer mismatch, actual capture arithmetic overflow, report persistence,
existing and concurrently created outputs, and a broken status sink. Native and
explicit raw ByteLevel JSON tokenizer paths produce identical bound artifacts
for equivalent complete semantics.

All ten targeted regressions passed in central execution with
`nightly-2026-09-08`. Final integrated validation also passes 91 public decoder,
campaign, codec/replay and durable-stream tests and 32 learned-operator tests.
Two existing real Unix-listener tests fail at creation with environment EPERM.
The original complete gate passes compiler identity and then refuses the reviewed
source snapshot, whose file-discovery limit, unlisted files and stale existing
digests remain unchanged. These targeted results do not establish complete
workspace qualification; see `IMPLEMENTATION_STATUS.md` for the integrated scope.

Passing these synthetic tests establishes the command's numerical and custody
composition. It does not establish trained-model accuracy, label authenticity,
independence of hidden data origins, safe external effects or production
qualification. The unchanged tokenizer support still excludes regex splitting,
normalization and arbitrary pretrained pipelines. Binary native helper input and
its complete witness requirements are unchanged. Full pretrained text support
requires those semantics explicitly; this command does not claim them.
