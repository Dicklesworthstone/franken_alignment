# Operator command for learned K/V publication

`create-learned-generated` connects explicitly imported model and tokenizer
assets, a verified learned K/V fit archive, registered probes, native helper
models, and independent human review to the original durable publication owner.
Each message request generates complete text ending in an admitted control stop,
runs the explicitly provisioned native sidecar congress, and publishes only through the
existing two-key path. Recipe version 1 retains one coarse round. Version 2 can
purchase retained exact residuals and run fresh reviews under a fixed bounded
schedule when the original congress records uncertainty. Version 3 adds explicit
single/sharded checkpoint sources with optional saved static LoRA adapters.
Version 4 also permits finite continuation after a confirmed message, using the
same admitted numerical owner and original remaining allowances.
Version 5 requires independently provisioned raw-residual or owned-K/V forecasts
before sampling each message, with a separate forecast for finish. See
[required operator forecasts](LEARNED_OPERATOR_FORECASTS.md) for the artifact,
runtime and recovery contract and its pending execution qualification.

Version 6 explicitly selects separately launched process reviewers using a fixed
per-round program schedule. It retains finite continuation and supports either
explicitly absent or required forecasts. See
[process-backed learned publication](PROCESS_LEARNED_PUBLICATION.md) for its
recipe, original process protocol, cleanup and remaining containment boundaries.

In versions /1 through /5, native helper models execute **inside the supervising process**, with separate
original evaluator owners and fixed input contracts. They are not helper OS
processes. Their configured names, models and cohorts do not establish statistical
independence or process isolation. The reviewer and stop clients use the existing
Linux peer-checked socket protocols.

The implementation is
[`workflow/actor_service/learned.rs`](../crates/fa-reference/examples/supervise_publication/workflow/actor_service/learned.rs).
The schema parsers are its `learned/recipe.rs`, `weights.rs`, `fit.rs`, `monitor.rs`, `forecast.rs`,
`native.rs`, `process.rs`, `process/config.rs` and `fields.rs` modules. Checkpoint sources use the shared
[`support/checkpoint_source.rs`](../crates/fa-reference/examples/support/checkpoint_source.rs)
loader and the original decoder, indexed reader and static adapter merger.

## Commands and ordinary operation

Use the repository-built `supervise_publication` example on Linux:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE --continue-generation
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume
supervise_publication create-learned-generated CONFIG NEXT_RECIPE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST
supervise_publication create-learned-generated CONFIG FINISH_RECIPE REVIEWER_PROFILE --finish-after-message PREVIOUS_REQUEST
supervise_publication create-learned-generated CONFIG FINISH_RECIPE --resume --finish-after-message PREVIOUS_REQUEST
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION
```

Fresh creation requires a new authority store. The creation and continuation
forms require the independently provisioned reviewer profile as their fourth
argument. Explicit continuation and finish can independently requalify their
helpers as described below. There is no unchecked reviewer mode. The receipt-only form accepts `--resume` in that argument's place
and contacts no reviewer.

### Enforce topology during learned publication

For authenticated learned actor ingress, `--actor-profile ACTOR_PROFILE` and
`--topology-profile PATH` may appear in either order. Both independently selected
profiles pass through the complete command admission before any original store
or transport is created. Duplicate, missing and conflicting option selections
remain errors.

Add `--topology-profile PATH` to select the original topology custodian for
learned generation, native or process review, and checked publication. This
composes with ordinary learned generation, required raw-residual forecasting,
and required owned-K/V forecasting. The existing learned recipe, numerical
limits, separate predictor role, native review and independent human key remain
the selected contracts.

| Invocation | Topology input | Independent history anchor |
| --- | --- | --- |
| Fresh creation | `fa.mediated-create/1` or `/2` | Input forbidden; a new output may be retained |
| `--continue-generation`, `--after-message`, `--finish-after-message` | `fa.mediated-submit/1` or `/2` | Required before opening the store |
| `--resume`, passive or reconciled receipts, receipt service, `--recover-stop` | `fa.mediated-submit/1` or `/2`, with no checkpoint output | Required; complete historical requirements remain checked |

For example, using independently provisioned files and a fresh output path:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE --topology-profile CREATE_TOPOLOGY --retain-history-anchor FIRST_ANCHOR
supervise_publication create-learned-generated CONFIG NEXT_RECIPE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST --topology-profile NEXT_TOPOLOGY --history-anchor FIRST_ANCHOR --retain-history-anchor NEXT_ANCHOR
supervise_publication create-learned-generated CONFIG FINISH_RECIPE REVIEWER_PROFILE --finish-after-message PREVIOUS_REQUEST --topology-profile FINISH_TOPOLOGY --history-anchor NEXT_ANCHOR
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume --receipts-only --topology-profile SAVED_TOPOLOGY --history-anchor SAVED_ANCHOR
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION --topology-profile SAVED_TOPOLOGY --history-anchor SAVED_ANCHOR
```

The input formats are the existing [creation profile](TOPOLOGY_SUPERVISED_CREATION.md),
[continuation profile](TOPOLOGY_SUPERVISED_CONTINUATION.md) and
[version-2 source checkpoint profiles](TOPOLOGY_CHECKPOINT_OPERATOR.md).
No second graph parser, checker or learned recipe version is introduced.
Version 1 requires independently retained original/current graphs, availability,
spent source work and operation floors. Version 2 takes the current contract and
spent work from the original source checkpoint. Its nested anchor must describe
the same cut as the separately selected learned history anchor, and the
canonical event count must still equal that retained checkpoint's extent.
Keep history and source outputs at distinct, fresh paths.

Creation installs the original terminal reserve, complete guards, numerical
generation and mediation in one first canonical image. The unique source is
attached to the original supervised driver before live preparation. After any
required fresh helper qualification, the original native cut must be verified
before learned inference, a forecast, a subsequent message or Finish. The same
driver retains source custody through both native and process review and checks
the source again at final publication. Missing inventory or a new bypass at that
late check cannot acquire permission from earlier native or human approval.
Already-dispatched uncertainty retains its original charge and outcome rules.

Each live recovery checks the complete original numerical, predictor, guard,
topology, reserve, policy, anchor and floor requirements before releasing roles.
Its fresh source must advance both graph and inventory generations, retain the
exact source/checker limits, and carry forward spent checks, reserved read bytes
and operation floors. A recovery fence does not recertify the graph or replenish
source allowances. Output failures cannot undo an acknowledged publication or
request another send.

Historical operations load only the independently selected original/current
topology requirements; they do not open `next` live topology files or fresh
native helper/evidence inputs. They refuse a requested new source checkpoint
because no active source is being returned. Passive receipts consume no clock,
fence or spare journal capacity. Explicit reconciliation retains the original
`Fence -> Time -> Sweep`, including for an already-known receipt. Offline stop
uses the original terminal tail directly. These are distinct finalizers, each
protected by the same complete recovery checks.

After a fence or acknowledged stop withdraws topology availability, the next
independent historical profile must describe that saved unavailable state.
Reusing `available: true`, a stale source checkpoint or an unretained successor
cut refuses without silently changing expectations. A matching completed stop
retry remains clock-free and adds no new fence. This bridge does not add the
separate plain publication-witness profile to learned stream bootstrap.

Four operator regression functions cover authenticated actor/native/human
message publication and Finish across all three predictor modes, anchored
source continuity, offline receipts and stop, loss of the source after dispatch,
an initial bypass before numerical work, and closed option parsing. Five public
receipt tests additionally cover full capacity, exact envelopes, independent
recovery requirements, real settlement versus unknown liability, and canonical
changes. Source review and nonmutating syntax parsing passed; compilation and
runtime execution remain pending because the required RCH daemon is unavailable.

### Requalify helpers after recovery

An original held-out qualification is invalidated by the recovery fence. An
explicit `--continue-generation`, `--after-message` or `--finish-after-message`
invocation may supply fresh independent evidence with the existing option:

```text
supervise_publication create-learned-generated CONFIG NEXT_RECIPE REVIEWER_PROFILE --after-message PREVIOUS_REQUEST --credibility-activation CAPSULE
```

The capsule uses the original bounded `CredibilityActivation` reference format,
committee comparison and independently bound held-out evidence contract from
[supervisor credibility](SUPERVISOR_CREDIBILITY.md). Its control predecessor and
authority epoch must match the recovered owner, including the new recovery
fence. Native activation must advance the authority epoch exactly once and
restore current qualification; replaying an old activation cannot renew it.
Initial creation refuses this option before configuration or durable startup.
Receipt-only `--resume` ignores an optional capsule without opening it.

Qualification occurs before new inference or request admission, within the same
deadline and authenticated stop service. The following numerical or finish step
must still acquire fresh policy evidence and time. Activation preserves paused
K/V state, pending numerical intent, sampled draws, finite work allowances and
forecast evidence. It cannot repair lost pending-forecast coverage or provide a
native review or human publication key. Every new message and finish still need
their original reviews and independent human approval.

Omitting required qualification retains the existing early refusal and allows a
corrected retry; an invalid message predecessor also refuses before live
preparation. A supplied capsule that fails during live preparation follows the
ordinary failure stop/drain path. A later deadline or source failure can therefore
leave an acknowledged qualification and a stopped workflow, without more tokens
or another publication. Inspect the original state before retrying.

Seven regression functions cover original native/human publication after fresh
qualification in all three learned modes, historical and malformed capsules,
pending forecast and numerical intent preservation, deadline/source/stop ordering,
the actual continuation caller and receipt command. Their campaigns and models
are deterministic enforcement fixtures, not calibrated or authenticated live
evaluation evidence. Execution and the required RCH qualification gate remain
pending; this operator bridge does not close the full FA-105 research packet.

An explicit finish uses an existing store and a new recipe request ID after the
exact original message's confirmed receipt. It preserves the numerical recipe,
runs a fresh native review and requires another independent human decision.
See [learned stream finish](LEARNED_STREAM_FINISH_OPERATOR.md) for predecessor,
capacity and exact finish-recovery rules.

For a stream admitted with recipe version 4 or 5, `--after-message` continues after
the latest receipt-confirmed message under a new request ID. It resumes the same
finite generation and requires fresh native and human review. Existing version
1–3 histories cannot be upgraded to this mode. See
[finite learned text stream continuation](LEARNED_TEXT_STREAM_CONTINUATION.md)
for the complete predecessor, stop-state and remaining-capacity contract.

Offline `--recover-stop OPERATION` reconstructs the original learned history and
applies its native stop/fence/drain transaction without a live reviewer, helper
model or current evidence file. Completed retries need no new clock or journal
capacity. It reports original unresolved liabilities without repeating effects.
See [offline learned stop recovery](LEARNED_STOP_RECOVERY.md) for the sealed
completion, stop identity, output and finite-capacity contracts.

Every recovery form can also require a separately retained exact history prefix
with `--history-anchor INPUT`. `--retain-history-anchor NEW_OUTPUT` exports a
checked successor from the final acknowledged owner, including after receipt
recovery. Offline stop accepts only the input option and rejects successor export
before I/O; initial creation accepts only the output option. See
[operator history anchors](LEARNED_OPERATOR_HISTORY_ANCHORS.md) for private
custody, bounded admission, new-file retention and failure semantics.

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
authority store is created or reopened and remains available during recovery, generation,
native review, human review and publication. Existing socket names are not
deleted to force a bind. Normal cleanup removes only this invocation's socket.
Provision the socket directory and kernel identities according to
[peer-checked publication](PEER_CHECKED_PUBLICATION_CLI.md): the existing directory
must have the configured supervisor ownership and no group/other write access.
The logical reviewer/scope and the connection-time UID/GID, plus PID when selected,
must match their independent profiles.

Recovery checks the same deadline and stop admission before opening, between
original journal events and once more before its fence. An authenticated peer
can interrupt replay without changing the journal or staged files; this returns
a recovery error and **does not apply or acknowledge a durable Stop**. The
receipt-only `--resume` forms instead use an elapsed replay budget while retaining
the original request deadline and terminal no-new-logical-clock behavior. See
[cooperative recovery](COOPERATIVE_LEARNED_OPERATOR_RECOVERY.md) for the command
contract, synchronous-operation limits and pending execution qualification.

The publication and receipt forms emit only the original actor-wire response;
offline stop emits the original native stop report. A successful publication command
requires its original `Known(Executed)` result and completed cleanup. A held,
denied, cancelled or unknown result is not converted to success. A nonzero exit
or lost stdout receipt does not prove that no publication occurred; inspect or
recover the original request rather than submitting a new effect.

The publication is the original protected journal's framed stream payload.
This command does not copy it to another destination or add a network send.
Ordinary creation appends one message. The explicit finish option separately
reviews and closes that confirmed prefix without generating another message.

## Files to provision

All referenced learned asset paths must be absolute, normalized file paths.
They are explicit operator inputs; filenames and model metadata never discover
other assets. The regular-file reader refuses symlinks at the final component
and bounds bytes before accepting the complete file.

| Input | Required content |
| --- | --- |
| `CONFIG` | Existing supervised-publication version-1 configuration: authority, target, policy, congress, helper contracts, human policy, source policy and timing |
| `LEARNED_RECIPE` | `fa.learned-publication/1` for one coarse round, `/2` for bounded residual refinement, `/3` for explicit checkpoint sources, `/4` for finite message continuation, or `/5` for required forecasts across finite messages and finish |
| Primary model configuration and weights | Original admitted Llama configuration, plus a complete SafeTensors path or an explicitly selected `fa.checkpoint-weights/1` source |
| Primary tokenizer | Explicit `native_archive`, `huggingface_raw_bytelevel`, `huggingface_gpt2_unicode17` or `huggingface_llama3_unicode17` selection |
| Prompt and sampling file | Actual UTF-8 prompt bytes and original `fa.decoder-sampling/1` sampling configuration |
| Fit binding and fit archive | Independently retained `fa.learned-fit-binding/1` plus original `FAKVFIT/1` archive |
| Learned monitor | `FAKVMON2` bound artifact or legacy `fa.learned-kv-monitor/1`, with every original layer and both K/V sides |
| Required forecast (recipe `/5`) | Independent closed `fa.learned-forecast/1` artifact selected by `files.forecast`; exact original predictor, model/source/stream contracts and lifetime allowances |
| Native roster | `fa.learned-native-roster/1` for single weight paths, or `/2` for explicit checkpoint-source selection; actual imported model/tokenizer/monitor/sampling files for every committee member |
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
the [explicit ByteLevel contracts](OPERATOR_TOKENIZER_INTAKE.md), including the
original raw mode and two named Unicode-17 grammars. Normalization, arbitrary
regex, prefix insertion and postprocessing remain unsupported. The complete
grammar and Unicode version are bound in the native archive and compared during
monitor loading and durable recovery. Unicode profiles require valid UTF-8;
they do not convert a binary native-helper sidecar into text.
There is no format sniffing or fallback. Both primary and native models retain
all original shape, context, vocabulary, scalar and numerical limits.

### Explicit checkpoint sources

Primary recipe versions 3 through 5 accept either the existing absolute string path
in `files.weights`, or this closed selection object:

```json
{
  "format": "checkpoint_source",
  "path": "/operator/primary/checkpoint-source.json"
}
```

The path names a separate [checkpoint-source manifest](CHECKPOINT_SOURCE_MANIFEST.md),
with schema `fa.checkpoint-weights/1`. Its `base` selects a single file or an
explicit index and complete label-to-file map; `adapter` is `null` or a saved
static LoRA declaration. The manifest supplies its own bounded `weight_bytes`,
`weight_read_calls` and, when applicable, `merge_products` allowances. An index
label is a lookup key; it never discovers a file. Unknown fields, unsupported
formats, missing mapped files and mismatched index coverage refuse. Neither a
filename suffix nor a parse failure changes the selected format.

The independently configured `CONFIG.actor.model_generation` and
`CONFIG.actor.tokenizer_generation` must match the final effective decoder
identity, including for an `audit_only` actor. Loading a manifest cannot update
those host pins. A correctly merged model paired with a stale host generation
refuses before store creation.

Versions 3 through 5 retain the complete progressive version 2 layout, including
required `sidecar.priority` and `native_review.round_ids`. For one coarse round,
use an empty priority, one nonzero round ID and `sidecar.budget.rounds: 1`.
Version 3 otherwise retains the earlier one-message generation behavior;
versions 4 and 5's continuation opt-in is described separately above. Primary recipe
versions 1 and 2 continue to require a single absolute weight path and reject
the selection object.

Native roster version 2 accepts the same string-or-object choice independently
for each member's `files.weights`. Its other fields are unchanged. Roster version
1 continues to require a single absolute weight path. The roster's version
controls its own weight selection, independently of the primary recipe version.

The primary or native `model.identity` is the independently provisioned **final
effective identity**. For an adapter, the manifest also supplies its base
identity: tenant, model and tokenizer generation must match, while final model
and profile generations must strictly advance. The original adapter constructor
checks the complete transition and supported tensor targets. With `adapter: null`,
the base uses the caller's final identity. See
[static LoRA ingestion](STATIC_LORA_INGESTION.md) for the admitted adapter contract.

The effective model passes through the same learned codec/probe binding,
tokenizer checks, native evaluator construction and publication owners as a
single-file import. Provision the codec and any bound monitor for that effective
model and final profile. Loading an adapter does not retrain a codec, replace its
independent fit binding, activate a monitor or supply either publication key.

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
binding permits a broader inventory, but this command also uses the
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
referenced files. Source manifests, shard indexes, adapter configurations, all
base shards and adapter tensors consume that same allowance. A repeated path
read is charged again, including a source shared by several native members.
Smaller per-file parser and model bounds also apply.

The recipe itself has a 64 KiB bound; fit-binding, monitor and native-roster JSON
each have a 2 MiB bound. A complete `FAKVMON2` monitor artifact has a 16 MiB bound
within the same aggregate allowance. Primary weight reading is additionally bounded by the
negotiated model size, the original checkpoint ceiling and the host's journal
byte limit. Fit archives retain their original archive limit but must also fit
the remaining aggregate asset allowance.

For each explicit source, the physical weight allowance is the smallest of the
manifest's `weight_bytes`, the caller's original model/weight ceiling and the
remaining aggregate asset allowance. One original `WeightReadBudget` spans that
source's entire base and adapter, with the manifest's bounded read-call ceiling;
it is not replenished between shards or before the adapter. The reader receives
one extra byte of capacity for its original EOF probe, without increasing the
accepted physical file-size ceiling. Actual streamed weight bytes are charged
to `asset_bytes` even when loading fails. Loader errors retain consumed reads
and any original merge-work reservation; a failure supplies no partial model.

The manifest, adapter configuration, complete identity/merge estimate and exact
index-label roster are checked before tensor files are opened. Every selected
base and adapter file then passes regular-file and aggregate-size admission
before base scalar reads. The original sharded reader validates every base
header before base scalars; the original adapter reader validates its header
after base loading. These checks do not form an atomic filesystem snapshot;
the operator must keep the inputs and parent directories controlled and stable.

The asset allowance measures loaded bytes, including streamed tensor work
retained on refusal. It does not measure peak memory, guarantee future journal
capacity or replace numerical/read budgets. Source refreshes during operation
use the existing separate source policy. On receipt-only recovery, skipped
native assets consume no asset allowance.

## Progressive recipe: fa.learned-publication/2

Version 2 keeps all version 1 fields and original numerical contracts. It adds
exactly two required fields, both closed and bounded:

| Group | Added required field |
| --- | --- |
| `sidecar` | `priority`: ordered array of exact residual groups |
| Every `sidecar.priority` entry | `layer`, `side`, `position`, `head` |
| `native_review` | `round_ids`: the ordered, fixed native congress round IDs |

`side` is `"key"` or `"value"`. Layer IDs come from the imported original K/V
profile; positions and heads are zero-based. Groups must be unique and within
the negotiated model shape. A priority may name at most 256 groups. At actual
plan creation, **every named residual must already exist in the current checked
learned source**. Current sidecar capture describes the latest accepted position;
an earlier retained generation event is not an implicit raw-window source. An
absent group or unretained residual refuses the plan. There is no raw recapture,
new actor inference, or automatic alternate group selection.

`round_ids` contains between 1 and 32 unique nonzero IDs. Its order is the review
order, and its length must equal `sidecar.budget.rounds`. There must be at least
one priority group per possible refinement, so a three-round schedule needs at
least two groups. The original owner rejects any round ID already used or leased
in the journal. Version 1 rejects both new fields and still requires one round.

For example, these are the sidecar and native-review sections for a **two-round
synthetic fixture**, with the rest of the recipe unchanged and `schema` set to
`"fa.learned-publication/2"`:

```json
{
  "sidecar": {
    "identity": {"object_id": 1001, "generation": 1, "transform_id": 7},
    "priority": [{"layer": 1, "side": "key", "position": 3, "head": 0}],
    "budget": {"rounds": 2, "residual_bytes": 65536, "committee_bytes": 1048576}
  },
  "native_review": {
    "round_ids": [101, 102],
    "polls": 4096,
    "probe_coordinates": 10000,
    "reconstruction_products": 1000000,
    "evaluations": 4,
    "scalar_products": 4398046511104,
    "sampling_entries": 16384
  }
}
```

The example reserves two original native evaluators per round, each with its
explicit policy allowance. Actual models require their own complete work and
input bounds. The loader sums every member's original native policy reservation
and multiplies by the full number of rounds using checked arithmetic. Insufficient
evaluations, scalar products or sampling entries refuse before durable startup.
An unstarted, failed or cancelled future round does not refund that reservation.

All windows freeze from one fresh start time. Round 1 uses the existing
`CONFIG.timing.commit_ms` and `reveal_ms` offsets; each following round uses the
preceding round's scheduled reveal deadline as its origin. The entire schedule
must fit strictly within both the recipe TTL and configured runtime before asset
admission, and within the remaining original action deadline after generation. Windows
never slide to compensate for slow work, partial disclosure or missing members.
Receipt-only recovery does not apply this prospective lifetime check.

Before the first native Begin, every future roster receives fresh original
evaluators, the complete registered probes, and fresh commitment salts. These
are built from the already admitted immutable model assets. Files are not reread
and evaluators are not replaced after any answer is known. The same helper input
profile remains independently provisioned for the actual coarse/refined inputs;
the command does not infer calibration or statistical independence.

The first round always starts with the original coarse sidecar. If complete
numerical probes remain uncertain, the original worker abstains without starting
its native model. An original completed review with explicit abstention may buy
the next declared residual. Missing workers are not such a request. The purchase
adds that exact retained block to the existing evidence, charges cumulative
residual and replicated committee bytes, and records a new input revision. The
old review cannot authorize the richer input. A fresh original round must run
all probes and, only after complete quiet, the full native-model judgment.

The final original congress still supplies only the automatic review result;
independent human approval and current-policy publication checks remain required.
A completed roster with no abstaining members is terminal; a Deny or Hold in
that result does not trigger another model attempt. Only explicit abstention
can request a richer input through the original planner. Unresolved uncertainty,
unavailable future windows and exhausted
disclosure or poll allowances remain restrictive. Original cancellation destroys
remaining numerical owners while retaining spent work, acknowledged refinement,
round leases and unresolved publication obligations.

**The poll allowance applies to the entire sequence.** Each real quantum still
records one policy observation for all current members together. The original
1024-event source-history ceiling is unchanged. The two-round test fixture has a
909-byte coarse input and at most 57 residual bytes plus 8 bytes of framing. Its
first uncertain round needs five polls and no native inference; the richer round
and complete publication are bounded by 1018 source observations without assuming
any BPE compression. This is a bound for that synthetic fixture, not capacity
evidence for arbitrary models or inputs. Operators must budget their complete
prompt, probes, protocol, generation and publication histories.

## Provisioning trained K/V probes

`KvDecoderCampaign::monitor_json` can now produce this command's existing
`fa.learned-kv-monitor/1` input directly from a complete accepted training,
calibration and held-out K/V campaign. Every model layer and both sides must
pass before export. The actual loader tests preserve exact coefficients and all
runtime budgets, then execute original sampled quiet and hold behavior. See
[trained K/V monitors](TRAINED_KV_MONITORS.md) for the public API and required
independent model/codec provisioning. The JSON does not authenticate training
labels, model/tokenizer identity or lineage, and import alone does not establish
held-out acceptance.

`KvDecoderCampaign::monitor_artifact` also exports `FAKVMON2`, which contains
that same JSON and complete original model, tokenizer and codec/policy comparison
bytes. Set `files.monitor` to this artifact to require exact matching of the
independently imported assets before generation or store creation. This detects
changed unused weights, tokenizer semantics, codec provenance and inconsistent
settings even under unchanged numeric identities. It does not authenticate the
artifact or its training labels. See [bound trained monitors](BOUND_TRAINED_KV_MONITORS.md)
for the complete binding and size contract.

For actual checkpoint files, the offline
[`train_learned_kv_probes_from_checkpoint` command](TRAIN_LEARNED_KV_PROBES_COMMAND.md)
performs complete fitting, calibration and held-out evaluation and saves the
accepted bound artifact. It retains failed campaign diagnostics and supplies no
publication approval or automatic policy activation.

## Independent fit binding: fa.learned-fit-binding/1

The fit archive can be produced directly from checkpoint files and original
token documents by the [codec training command](TRAIN_LEARNED_KV_CODEC_COMMAND.md),
using the original fitting checkpoint interface. Its separately saved binding
and archive are the exact files consumed here and by the K/V monitor trainer.
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
residual retention. A version 2 priority must name blocks retained by this policy;
retention alone never discloses them to helpers or changes version 1 behavior.
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

## Native model roster: fa.learned-native-roster/1 or /2

`members` is an object keyed by the exact helper names already registered in
`CONFIG.helpers`. Its keys must equal the complete committee roster.

| Group | Required fields |
| --- | --- |
| Root | `schema: "fa.learned-native-roster/1"` or `"fa.learned-native-roster/2"`, `members` |
| Each member | `model`, `files`, `text` |
| Member `model` | `identity`, `context`, `stream` |
| Member `model.identity` | `tenant`, `model`, `model_generation`, `tokenizer_generation`, `profile_generation` |
| Member `files` | `model_config`, `weights`, `tokenizer`, `monitor`, `sampling` |
| Member `text` | `max_new_tokens`, `max_output_bytes`, `stop_tokens`, `tokenization`, `scalar_products`, `sampling_entries` |
| Member `text.tokenization` | `input_bytes`, `pair_lookups`, `heap_pops` |

Each tokenizer is the same explicit format/path object described above.
In version 1, `files.weights` is a single absolute SafeTensors path. In version
2, it also accepts the explicit checkpoint-source object described above. Both
forms feed the same complete effective model into the original native evaluator.
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
original full-input profile. For progressive primary recipes, each future round
has fresh original evaluators from the same explicitly loaded roster. All rosters and whole-sequence
work reservations are installed before the first round; a failed member is not
replaced after its result is known.

## Original transitions and recovery

Creation first admits the external recipe and native inputs. The first original
store image includes the stream, exact learned generation recipe, mandatory
sidecar provenance, policy-only source requirement and original terminal recovery
reserve. The reserve precedes learned Enable in that single canonical image.
Guarded recovery pins the exact reserve before replay, cleanup or fencing.

Each generation step acquires current policy through the original source owner
and observes time before and after acquisition. After the post-read clock, it
checks the original source frontier and read-start lease; exact expiry refuses
before numerical work. The original write-ahead
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

For checkpoint sources, this comparison binds the complete effective primary
model after original merging, including its final profile and every parameter.
An equivalent physical repack from several shards to one file, or an equivalent
premerged checkpoint, can recover the same generation when all other bound
inputs remain equal and file admission still succeeds. A changed effective
parameter refuses recovery before journal mutation even when the numeric
identity, selected prompt logits and generated message remain unchanged.
Manifest filenames and shard packaging do not replace that comparison.

Recovery reconstructs original acknowledged operations and pauses generation.
Continuation acquires fresh policy and time, resumes the same numerical state,
and completes any retained pending intent through the original path. It does
not reset the sampler, replenish numerical work or revive old effect keys.
Native inputs and the reviewer profile are needed because this route can still
perform a new review and publication.

Version 5 additionally requires intact forecast coverage. Recovery of an
unanswered forecast retains its work but loses coverage, so
`--continue-generation` cannot restart that output attempt.

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
new generation or refinement. The outer recipe still has its closed required fields, including
the `native_roster` path, but that path is not opened.

The original retained action supplies the deadline for the exact source-only
ticket retry; changing `ttl_ms` cannot renew it. The stored stream must match and
the action must be a message release. Only the original receipt/reconciliation
path may resolve Dispatching or Unknown. There is no effect resend or fresh
admission on an absent key. Repeated recovery consumes the original finite
recovery capacity.

The generic raw `inspect`/resume commands are not substitutes for the explicit
learned recipe recovery path. A new message in an admitted version 4 stream uses
the separate predecessor-bound `--after-message` form, while explicit stream
finish has its own form and receipt lookup described above. Neither operation
is implied by loading a checkpoint source or replaying a recorded receipt.

## Verification status

The last executed learned-command selection predates recipe version 5. It was
independently reviewed and executed with qualified `nightly-2026-09-08`.
All forty-one learned-command example tests
compiled. Thirty-nine passed, including all four checkpoint-source tests and
all three finite-continuation tests alongside the existing direct, admission,
progressive review, trained-monitor, bound-artifact and explicit finish tests.
Two actual Unix-listener tests failed at listener creation with environment
EPERM: the complete peer publication test and authenticated stop during a
partially computed refined round. Neither test is skipped, and their transport
paths have not been replaced by a weaker fallback.

The progressive positive executes actual coarse numerical uncertainty, pays for
one retained exact K residual, invalidates the old input, runs fresh complete
probes and native model judgment, requires independent human approval, publishes
once, and reopens the original receipt without another review. Negative controls
cover one-byte-short residual allowance, complete native Deny leaving the third
roster unused, total poll exhaustion with retained successor work, changed policy,
malformed schedules and aggregate work, unretained groups, and insufficient
future windows. The configured-runtime/TTL admission boundary is checked before
model loading and again against the live remaining action deadline.

All four checkpoint-source tests pass on qualified `nightly-2026-09-08`. The
tests in
[`learned/tests/checkpoint_source.rs`](../crates/fa-reference/examples/supervise_publication/workflow/actor_service/learned/tests/checkpoint_source.rs)
cover independently provisioned nonzero adapted final-generation assets,
original sampled generation and codec fitting, actual native review, both human
decisions, numerical and terminal recovery, equal physical repacking, and a
changed unused effective parameter refused before journal mutation. Additional
controls in those tests cover old schema behavior, closed selection, exact and
insufficient byte/read allowances, pre-tensor identity/merge/index refusals and
missing later helper shards without fallback. Version 4 continuation has its
own verification record in
[the continuation contract](LEARNED_TEXT_STREAM_CONTINUATION.md).

The finish positive reopens a full one-message stream, preserves its complete
prefix and numerical state, runs another native review and requires independent
human approval. Refusals retain original capacity, work and deadlines; terminal
receipt recovery needs no live evidence, helper inputs or current-clock reads.

All complete-input, nonzero K/V, native-token, reservation, residual-spending,
original-outcome and source-history assertions remain. These synthetic fixtures
establish executable contract behavior; they do not establish pretrained probe
quality, independent model judgment, OS isolation or a matched-cost research
result. The command was unexecuted when first authored; this later execution
found and fixed real atomic-reserve and post-read-freshness omissions.

The fresh original complete project gate passes qualified compiler identity but
stops at the reviewed source snapshot: its 512-file discovery ceiling,
depth-eight limit, stale existing digests and unlisted existing/new files refuse this tree. Its records
and enforcement are unchanged; formatting, Clippy and complete workspace
qualification are not reached or claimed to pass.
