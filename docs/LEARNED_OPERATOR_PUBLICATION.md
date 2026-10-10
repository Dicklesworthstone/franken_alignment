# Operator command for learned K/V publication

`create-learned-generated` connects explicitly imported model and tokenizer
assets, a verified learned K/V fit archive, registered probes, native helper
models, and independent human review to the original durable publication owner.
It generates one complete message ending in an admitted control stop, runs one
native sidecar congress, and publishes only through the existing two-key path.

The native helper models execute **inside the supervising process**, with separate
original evaluator owners and fixed input contracts. They are not helper OS
processes. Their configured names, models and cohorts do not establish statistical
independence or process isolation. The reviewer and stop clients use the existing
Linux peer-checked socket protocols.

The implementation is
[`workflow/actor_service/learned.rs`](../crates/fa-reference/examples/supervise_publication/workflow/actor_service/learned.rs).
The schema parsers are its `learned/recipe.rs`, `fit.rs`, `monitor.rs`,
`native.rs` and `fields.rs` modules.

## Commands and ordinary operation

Use the repository-built `supervise_publication` example on Linux:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE --continue-generation
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume
```

Fresh creation requires a new authority store. The creation and continuation
forms require the independently provisioned reviewer profile as their fourth
argument. They do not accept a credibility activation option or an unchecked
reviewer mode. The receipt-only form accepts `--resume` in that argument's place
and contacts no reviewer.

For example, start the supervisor with actual operator files:

```sh
supervise_publication create-learned-generated /operator/host.json /operator/learned.json /operator/reviewer.json
```

After the native congress permits review, the command announces
`SOCKET_DIRECTORY/review-REQUEST.sock` on stderr. In the independently configured
reviewer terminal, select the recipe's request ID:

```sh
supervise_publication review-peer /operator/reviewer.json 7
```

The existing console displays the original complete offer and requires the exact
explicit decision, request ID and session nonce. The learned command has no
automatic human-approval option. The same reviewer profile also permits the
independent stop client:

```sh
supervise_publication stop-peer /operator/reviewer.json 7
```

The stop endpoint, `SOCKET_DIRECTORY/stop-REQUEST.sock`, is claimed before the
authority store is created or reopened and remains available during generation,
native review, human review and publication. Existing socket names are not
deleted to force a bind. Normal cleanup removes only this invocation's socket.
Provision the socket directory and kernel identities according to
[peer-checked publication](PEER_CHECKED_PUBLICATION_CLI.md): the existing directory
must have the configured supervisor ownership and no group/other write access.
The logical reviewer/scope and the connection-time UID/GID, plus PID when selected,
must match their independent profiles.

Stdout contains only the original actor-wire response. A successful command
requires its original `Known(Executed)` result and completed cleanup. A held,
denied, cancelled or unknown result is not converted to success. A nonzero exit
or lost stdout receipt does not prove that no publication occurred; inspect or
recover the original request rather than submitting a new effect.

The publication is the original protected journal's framed stream payload.
This command does not copy it to another destination or add a network send.
It appends one message and does not issue a separate stream-finish action.

## Files to provision

All referenced learned asset paths must be absolute, normalized file paths.
They are explicit operator inputs; filenames and model metadata never discover
other assets. The regular-file reader refuses symlinks at the final component
and bounds bytes before accepting the complete file.

| Input | Required content |
| --- | --- |
| `CONFIG` | Existing supervised-publication version-1 configuration: authority, target, policy, congress, helper contracts, human policy, source policy and timing |
| `LEARNED_RECIPE` | `fa.learned-publication/1`, described below |
| Primary model configuration and weights | Original admitted Llama configuration and one complete SafeTensors file |
| Primary tokenizer | Explicit `native_archive` or `huggingface_raw_bytelevel` selection |
| Prompt and sampling file | Actual UTF-8 prompt bytes and original `fa.decoder-sampling/1` sampling configuration |
| Fit binding and fit archive | Independently retained `fa.learned-fit-binding/1` plus original `FAKVFIT/1` archive |
| Learned monitor | `fa.learned-kv-monitor/1`, with every original layer and both K/V sides |
| Native roster | `fa.learned-native-roster/1`, with actual imported model/tokenizer/monitor/sampling files for every committee member |
| `REVIEWER_PROFILE` | Existing peer profile matching the host's reviewer, scope and Unix-millisecond clock |
| `CONFIG.evidence_path` | Fresh policy-only evidence with the exact complete committee member keys and **empty helper contexts** |

The tokenizer object always has exactly `format` and `path`:

```json
{
  "format": "huggingface_raw_bytelevel",
  "path": "/operator/primary/tokenizer.json"
}
```

`native_archive` selects the original ByteBpe archive. The JSON choice retains
the [existing raw ByteLevel contract](OPERATOR_TOKENIZER_INTAKE.md), including its
refusals of unsupported normalization, regex splitting and postprocessing.
There is no format sniffing or fallback. Both primary and native models retain
all original shape, context, vocabulary, scalar and numerical limits. This first
command uses single-file weights; it has no sharded-weight or adapter field.
[Static LoRA ingestion](STATIC_LORA_INGESTION.md) is a separate explicit input
consumer and supplies neither publication key.

### Reusing the existing CONFIG

Use the complete schema and fixture documented in
[the supervised-publication command](SUPERVISED_PUBLICATION_COMMAND.md), including
[`supervised_publication.json`](../crates/fa-reference/fixtures/supervised_publication.json).
The new command does **not** define a reduced host configuration.

In particular, every `helpers.<name>` still requires `cohort`, `weight`,
`profile`, `projection_id`, `question_hex` and `program`.
The `program` block still requires `executable`, `directory`, `arguments`
and `environment` because the same version-1 parser is used. **This learned
command does not launch that programs map.** Its native roster supplies the
in-process numerical evaluators; it never falls back to those programs.

The helper names must match the native roster and policy-only evidence roster
exactly. The configured full-input profiles and questions remain the original
committee contracts. Model identity labels do not replace those contracts.

Set `initial_payload_hex` to the empty string and match the primary model tenant
to the configured effect scope. The `actor` block remains the original
`audit_only` metadata; the imported learned recipe supplies the actual numerical
owner. Policy predicates must admit the original framed message, target and
complete charge, not an assumed unframed prompt.

Budget the original journal and source histories for the complete operation.
Generation persists intents and outcomes, source refreshes retain their original
events, and native review performs further source/protocol transactions.
The shared host configuration still admits at most 4,096 journal events and
16 MiB of journal bytes, with the original terminal recovery reserve retained.
Exhaustion refuses further work; it does not select a cheaper approval route.

## Primary recipe: fa.learned-publication/1

Every listed field and nested grouping is required. Unknown fields and duplicate
JSON keys refuse. Counts and IDs are unsigned integers; the original constructors
apply their additional bounds.

| Group | Exact fields and meaning |
| --- | --- |
| Root | `schema`, `request`, `ttl_ms`, `asset_bytes`, `model`, `publication_stream`, `binding`, `recovery_floor`, `files`, `text`, `sidecar`, `native_review` |
| `model` | `identity`, `context`, `stream`, `evaluation_origin`, `monitor_generation` |
| `model.identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation` |
| `publication_stream` | `id`, `generation`, `max_messages`, `max_message_bytes`, `max_total_bytes`; passed to the original stream profile |
| `binding` | `token_ids`, `score_words`, `encoded_bytes`; exact cumulative binding and this command's per-capture limits |
| `recovery_floor` | `journal_revision`, `control_sequence`, `authority_epoch`; independently retained minimum recovery state |
| `files` | `model_config`, `weights`, `tokenizer`, `sampling`, `prompt`, `fit_binding`, `fit_archive`, `monitor`, `native_roster` |
| `text` | `max_new_tokens`, `max_output_bytes`, `stop_tokens`, `completion`, `tokenization`, `decoder_products`, `vocabulary_scores`, `telemetry` |
| `text.tokenization` | `input_bytes`, `pair_lookups`, `heap_pops` |
| `sidecar` | `identity`, `budget` |
| `sidecar.identity` | `object_id`, `generation`, `transform_id`; nonzero original sidecar identity |
| `sidecar.budget` | `rounds`, `residual_bytes`, `committee_bytes` |
| `native_review` | `polls`, `probe_coordinates`, `reconstruction_products`, `evaluations`, `scalar_products`, `sampling_entries` |

`request`, model stream/origin/monitor IDs and relevant native IDs must be
nonzero. `ttl_ms` is 1 through 3,600,000. A fresh action's lifetime is the
smaller of this TTL and `CONFIG.timing.runtime_ms`; the real review window must
fit inside it. The workflow uses actual Unix-millisecond clock observations
and a separate monotonic waiting bound.

**`binding.token_ids` must be 1 through 4,096.** The general cumulative library
binding permits a broader inventory, but this one-message command also uses the
value for one original learned capture. A larger value is rejected at recipe
admission, before model weights or durable startup. Other evidence and numerical
bounds remain enforced by their original constructors.

`completion` must be `"stop_required"`. Stop IDs are unique, nonempty, explicit
tokenizer controls. An ordinary content token cannot silently become a successful
stop. The prompt and sampled output must fit the actual decoder context; a held
sample, exhausted generation budget or output without its admitted stop cannot
supply the message.

`sidecar.budget.rounds` is exactly 1. Residual and committee byte allowances are
nonzero and bounded by the original sidecar limits. This runner selects an empty
residual-priority list: the one fixed native round receives the original coarse
sidecar packet. Need-more, failure or abstention does not start a replacement
round or become Allow.

The `text.telemetry` object contains these eleven exact cumulative work fields:

| Work grouping | Fields |
| --- | --- |
| Compression | `compression_source_values`, `compression_encoded_bytes`, `compression_work_units` |
| Source checking | `source_check_values`, `source_check_encoded_bytes`, `source_check_reconstruction_products` |
| Monitoring | `monitor_encoded_bytes`, `monitor_probe_coordinates`, `monitor_reconstruction_products`, `monitor_materialized_values`, `monitor_refinements` |

For the native review, `evaluations`, `scalar_products` and
`sampling_entries` must admit the complete independently loaded roster's
original policy reservations. `polls` is nonzero and cannot exceed the original
native review ceiling. Probe limits bound complete queried coordinates and
reconstruction products. These values are work allowances, not votes or timing
measurements.

The policy source's retained-event allowance is a separate lifetime budget,
with an original ceiling of 1024 events. Each native poll captures one current
policy observation for the entire roster; generation, human review and
publication also capture observations. Budget complete prompt lengths, probes
and protocol work against that history. Increasing `native_review.polls` does
not enlarge it. Source-history exhaustion remains restrictive.

### Asset accounting

`asset_bytes` is a nonzero aggregate allowance no greater than **64 MiB**.
The same allowance covers the primary model files, tokenizer, sampling, prompt,
fit binding/archive, learned monitor, native roster and every native member's
referenced files. A repeated path read is charged again. Smaller per-file parser
and model bounds also apply.

The recipe itself has a 64 KiB bound; fit-binding, monitor and native-roster JSON
each have a 2 MiB bound. Primary weight reading is additionally bounded by the
negotiated model size, the original checkpoint ceiling and the host's journal
byte limit. Fit archives retain their original archive limit but must also fit
the remaining aggregate asset allowance.

This allowance counts completed file bytes loaded by this command. It does not
measure peak memory, guarantee future journal capacity or replace the original
numerical/read budgets. Source refreshes during operation use the existing
separate source policy. On receipt-only recovery, skipped native assets consume
no asset allowance.

## Independent fit binding: fa.learned-fit-binding/1

The fit archive is produced by the original fitting checkpoint interface.
Retain `checkpoint.binding()` independently when provisioning the archive and
serialize its intended values into this JSON schema. The checkpoint's
`encode_archive(byte_limit)` produces the separate original archive.
See [learned fit replay](LEARNED_FIT_REPLAY.md).

| Group | Required fields |
| --- | --- |
| Root | `schema: "fa.learned-fit-binding/1"`, `policy`, `budget`, `sources` |
| `policy` | `id`, `generation`, `rank`, `sweeps`; original `LearnedKvPolicy` |
| `budget` | `source_values`, `parameter_values`, `scratch_values`, `work_units`; original `FitBudget` |
| Each `sources` entry | `origin`, `descriptor_hex` |

There must be 1 through 64 source entries with unique nonzero origins.
`descriptor_hex` is the complete lowercase hexadecimal encoding of the
original `ModelKvDescriptor`, within its original descriptor limit. It is
not a filename or a free-form shape declaration.

The importer checks the complete expected policy, budgets and source inventory
against the original archive framing, then reruns the original fitter and
compares its result. It does not deserialize claimed basis coefficients as a
usable codec, invent a training corpus or substitute a default fit. The returned
codec must bind the imported primary model's exact K/V profile before monitoring
can start.

Descriptor/identity equality is not authentication of source scalars, model
origin or a train/evaluation split. The operator authenticates those artifacts
and retains the expected binding independently of the archive it checks.

## Probe configuration: fa.learned-kv-monitor/1

This is explicit learned K/V monitoring of the primary model. It is separate
from each native helper's ordinary decoder-monitor JSON.

| Group | Required fields |
| --- | --- |
| Root | `schema: "fa.learned-kv-monitor/1"`, `retention`, `inference_products`, `preparation`, `audit`, `taps` |
| `preparation` | `compression`, `source_check` |
| `preparation.compression` | `source_values`, `encoded_bytes`, `work_units` |
| `preparation.source_check` | `source_values`, `encoded_bytes`, `reconstruction_products` |
| `audit` | `rows`, `budget` |
| Each `taps` entry | `layer`, `side`, `budget`, `probes` |
| Each probe | `id`, `generation`, `weights`, `bias`, `threshold` |
| Every monitoring `budget` | `encoded_bytes`, `probe_coordinates`, `reconstruction_products`, `materialized_values`, `refinements` |

`retention` is explicitly `"all"` or `"none"`. It selects original learned
residual retention; it does not change this command's one-round disclosure policy.
`inference_products` is the original decoder work allowance used by the learned
policy.

Register **every original model layer and both `"key"` and `"value"` sides**.
Use layer IDs from `model.cache_profile()`, not guessed names from a checkpoint
export. Taps must be unique and have nonempty probe rosters. Extra/missing layers,
missing K/V sides, duplicate probes, wrong dimensions and mismatched fit profiles
refuse through the original constructors.

Each probe has nonzero ID and generation, a nonempty finite-f32 weight vector,
and finite-f32 bias and threshold. Its weight count must equal the original tap's
flattened dimensions, including GQA's actual cache width. The imported model
supplies the exact tensor profile; the JSON cannot replace it.

The same explicitly registered probes are used for the corresponding rows of
the actual native sidecar query. A quiet probe result admits original numerical
input to the helper evaluator; it is not the helper's final categorical judgment
and is not either publication key.

## Native model roster: fa.learned-native-roster/1

`members` is an object keyed by the exact helper names already registered in
`CONFIG.helpers`. Its keys must equal the complete committee roster.

| Group | Required fields |
| --- | --- |
| Root | `schema: "fa.learned-native-roster/1"`, `members` |
| Each member | `model`, `files`, `text` |
| Member `model` | `identity`, `context`, `stream` |
| Member `model.identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation` |
| Member `files` | `model_config`, `weights`, `tokenizer`, `monitor`, `sampling` |
| Member `text` | `max_new_tokens`, `max_output_bytes`, `stop_tokens`, `tokenization`, `scalar_products`, `sampling_entries` |
| Member `text.tokenization` | `input_bytes`, `pair_lookups`, `heap_pops` |

Each tokenizer is the same explicit format/path object described above.
`files.monitor` is the original `fa.decoder-monitor/1` configuration bound to
that member's own decoder identity. `files.sampling` is its original
`fa.decoder-sampling/1` configuration. Each member needs nonzero stream and
output bounds and explicit controls accepted by its original tokenizer/evaluator.

The primary recipe uses `decoder_products` and `vocabulary_scores` for its
learned generation. The native member uses `scalar_products` and
`sampling_entries` for its ordinary native generation. These are distinct
original budget types; do not interchange their field names.

The loader closes the schema and paths for all members before loading their
models. It checks each tokenizer, monitor, sampler and stop policy with an empty
original evaluator before durable startup. That preflight computes no helper
token. At review start, a fresh owned evaluator binds to that member's actual
original full-input profile. Fixed rosters and work reservations are installed
before the round begins; a failed member is not replaced after its result is
known.

## Original transitions and recovery

Creation first admits the external recipe and native inputs. The first original
store image includes the stream, exact learned generation recipe, mandatory
sidecar provenance and policy-only source requirement. It then retains the
original terminal recovery reserve.

Each generation step acquires current policy through the original source owner
and observes time before and after acquisition. The original write-ahead
numerical intent and acknowledged outcome retain tokens, cache state, sampler
state, telemetry and spent work. Only a complete quiet monitored control stop
permits source-only message submission.

The original gateway derives the message frame and charge. The runner creates
one source-bound sidecar plan and transfers the same supervising driver into
the original native sequence. Actual probes and native model inference produce
the original commit/reveal congress. It advances at most the original cooperative
quantum per poll; when only a protocol deadline can advance, it waits against
the real clock, retaining the same poll budget and stop service.

After terminal numerical handoff, only the original permitting congress result
can reach the human-request stage. Authenticated human approval supplies the
separate human key. Fresh policy-only captures, current learned provenance,
automatic reservation, first-publication checks and endpoint reconciliation
remain owned by the original driver. Neither fit success, probe silence, model
output nor a LoRA merge supplies approval.

### Continue generation: request must still be absent

```sh
supervise_publication create-learned-generated /operator/host.json /operator/learned.json /operator/reviewer.json --continue-generation
```

Use this after interruption before the request was admitted. Reopening requires
the independently supplied primary numerical recipe, complete mandatory guard
inventory, effective policy and external `recovery_floor`. Matching precedes
cleanup and the original recovery fence. The exact effective model parameters,
tokenizer, fitted codec, probes, prompt, sampler and numerical limits are bound;
path spelling or metadata labels do not authenticate their contents.

Recovery reconstructs original acknowledged operations and pauses generation.
Continuation acquires fresh policy and time, resumes the same numerical state,
and completes any retained pending intent through the original path. It does
not reset the sampler, replenish numerical work or revive old effect keys.
Native inputs and the reviewer profile are needed because this route can still
perform a new review and publication.

If that request already exists, this option refuses and directs the operator
to `--resume`. It cannot replace a denied/cancelled/unknown request or append a
second message under its key.

### Resume a recorded receipt: no forward generation

```sh
supervise_publication create-learned-generated /operator/host.json /operator/learned.json --resume
```

This still needs the exact independently supplied primary recipe and fit/probe
assets to replay and validate the existing learned journal. Historical numerical
reconstruction and refitting can perform work; this is not a computation-free
receipt parser.

It skips the native roster and all native helper assets, reads no live policy
evidence, opens no reviewer/stop listener, requests no human key and starts no
new generation. The outer recipe still has its closed required fields, including
the `native_roster` path, but that path is not opened.

The original retained action supplies the deadline for the exact source-only
ticket retry; changing `ttl_ms` cannot renew it. The stored stream must match and
the action must be a message release. Only the original receipt/reconciliation
path may resolve Dispatching or Unknown. There is no effect resend or fresh
admission on an absent key. Repeated recovery consumes the original finite
recovery capacity.

The generic raw `inspect`/resume commands are not substitutes for the explicit
learned recipe recovery path. This command does not offer next-message,
stream-finish, identity-activation, qualification or sharded-model modes.

## Verification status

The implementation and its integration with the original APIs were independently
source-reviewed. Tests are authored against external synthetic checkpoint,
tokenizer, fit, probe and native-model fixtures, with original generation,
recovery, native congress and human/publication paths. Their assertions are
implementation evidence to be executed, not recorded outcomes.

**The new command and its tests were unexecuted at authoring time because the
execution service was unavailable.** Earlier tokenizer, rotary, decoder or helper
test passes do not validate this command. No fixture is presented as a passing
deployment or evidence of trained-model judgment quality.
