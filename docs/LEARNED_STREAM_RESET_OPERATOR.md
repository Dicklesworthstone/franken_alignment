# Capture, initiate and recover a learned-stream reset

The operator can capture an eligible original numerical stream checkpoint,
initiate an independently selected reset, and complete an already acknowledged
native `BeginStreamReset` after a crash. Completion implements the plain and
mediated guarded recovery paths described in
[PENDING_LEARNED_RESET_RECOVERY.md](PENDING_LEARNED_RESET_RECOVERY.md) and
[LEARNED_STREAM_CONTAINMENT.md](LEARNED_STREAM_CONTAINMENT.md). Capture and fresh
initiation also support the explicitly selected owned prompt-audit stream
recipe. Its [preparation, narrowing and predictive reset workflow](LEARNED_PREDICTIVE_STREAM_RESET.md)
reaches the complete unsampled prompt through the actual CLI, preserves the
pinned policy/predictor and uses the original fresh successor-cache audit before
sampling. Each operation retains the original confirmed audience and containment
rules. The [mediated predictive workflow](LEARNED_MEDIATED_PREDICTIVE_STREAM_RESET.md)
adds exact paired source/history custody through preparation, narrowing,
initiation and completion. Raw, legacy owned and post-sampling predictive resets
remain unavailable.

## Capture and independent initiation

### Capture during healthy generation

A single live Message command can select a checkpoint before its next sampled
token, after the original prompt is complete or immediately after an explicit
message continuation:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE \
  --checkpoint-before-generation CHECKPOINT_PROFILE \
  --retain-history-anchor NEW_OUTPUT_ANCHOR
```

The closed profile `fa.learned-generation-checkpoint/1` selects three canonical
unsigned decimal strings:

```json
{
  "schema": "fa.learned-generation-checkpoint/1",
  "checkpoint": "71",
  "expected_position": "2",
  "expected_sampled_draws": "0"
}
```

The selected original numerical recipe already binds the model and stream.
Checkpoint ID and position must be nonzero; the position and draw count must
match the actual quiet `Generating` boundary exactly. The example describes a
two-token prompt before its first sample. A continued message instead selects
the cumulative position and draw count after the previous confirmed message.
The command refuses a mismatched or already crossed boundary before drawing
output; it does not wait until a later position happens to match the document.

The actual original capture binds the current actor revision, authority epoch,
control sequence and complete confirmed audience. It is durably acknowledged
before another Stop/source/deadline check and before the next sampled-token
intent. Capture retains its existing synchronous full historical replay and
serialization costs; ordinary generation remains cooperatively stepped. This
option performs no reset, grants no publication key, and leaves all monitor,
native congress and independent human review requirements in force.

This live selection requires an explicit `functional_restart` configuration and
a plain nonpredictive stream or the explicit owned prompt-audit recipe. The
owned profile requires the complete original prompt and zero cumulative sampling.
It composes with fresh campaign provisioning, actor/precheck service and existing
anchor options; a resident sequence captures only its first selected Message.
It cannot be used for Finish or historical maintenance. Mediated capture requires
the explicit owned prompt-audit recipe and new paired history/topology outputs;
recovery additionally requires an exact v2 input pair. The topology creation
profile must therefore use `fa.mediated-create/2`, or the recovery profile
`fa.mediated-submit/2`, with an explicit `retain_checkpoint` destination distinct
from the history output.
Use a fresh checkpoint ID. A repeated ID refuses before generation; an already
acknowledged capture remains in history and ordinary continuation can omit the
new-capture flag. The usual anchor export is attempted at request completion.

A stream checkpoint remains eligible only within its captured confirmed audience
boundary. Successful publication changes that boundary. Existing failure paths
still apply their original Stop/drain behavior; this option does not implement
automatic incident reset or make a terminally stopped owner resettable.

For an intentional fresh pause before any prediction or draw, use
[`--prepare-stream-checkpoint`](LEARNED_PROMPT_PREPARATION.md). It retains the
captured anchor and exits with a maintenance result, while the live option above
continues into the original review/publication cycle.

### Capture an existing quiet prefix or initiate reset

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --capture-stream-checkpoint CHECKPOINT_REQUEST \
  --history-anchor INPUT_ANCHOR --retain-history-anchor NEW_OUTPUT_ANCHOR

supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --begin-stream-reset RESET_REQUEST \
  --history-anchor INPUT_ANCHOR --retain-history-anchor NEW_OUTPUT_ANCHOR
```

These are bounded offline operations. They acquire the original exclusive
history lock and replay every numerical and guarded transition. They do not
return a live owner or reviewer, observe fresh source/time, perform inference,
or publish output. A selected active owner must first release its lock. Capture
requires an already acknowledged quiet, active numerical prefix with an
unchanged confirmed audience; the command does not prefill or resume generation
to manufacture one. A functional restart grade must have been selected at native
bootstrap. The single learned creation command now admits an explicit
`functional_restart` declaration, while `audit_only` remains insufficient for
paired numerical checkpoint capture.

The capture document has the closed schema `fa.learned-stream-checkpoint/1`.
All eight coordinates are independently supplied canonical decimal strings:

```json
{
  "schema": "fa.learned-stream-checkpoint/1",
  "checkpoint": "1",
  "expected_journal_revision": "12",
  "expected_actor_revision": "2",
  "expected_authority_epoch": "0",
  "expected_control_sequence": "0",
  "expected_stream": "11",
  "expected_position": "1",
  "expected_sampled_draws": "0"
}
```

The numbers illustrate the complete shape, not a current history. Checkpoint ID,
stream and position must be nonzero. The actual journal revision, actor revision,
authority epoch, control sequence, numerical stream, position and sampled draws
must match exactly before capture. The command never fills missing coordinates
from the mutable store. Unknown/duplicate fields, noncanonical numbers and
unsupported declarations refuse before mutation.

Initiation uses the complete existing `fa.learned-stream-reset-recovery/1`
document below, including the independently selected checkpoint, original
control request, restart budget and first incident sequence. The native law
requires capture before the declared incident frontier. A saved checkpoint does
not authorize a different audience, restore spent numerical allowances, reduce
incident history or refund effects.

Both finalizers recheck the complete independent guards, effective policy,
credential epoch and recovery floors before and after preparing their original
native event. Capture appends one `StreamCapture`; initiation appends one
`BeginStreamReset`. Neither appends a recovery Fence. In particular, initiation
must leave the exact pending intent uninterrupted for `--complete-stream-reset`.
Use the resulting anchor as that completion command's input. An ordinary reopen
between initiation and completion can fence the intent and make it ineligible.

Exact retries append zero events. Capture matches the original capture's event
index and all coordinates; initiation matches the entire original intent and
incident frontier. An already completed reset is reported as historical
completion without initiating or auditing it again. A changed or interrupted
intent is refused. Original ordinary journal capacity funds these events; the
terminal reserve is not available to them.

The result schema is `fa.learned-stream-control-result/1`, with outcome
`captured`, `already_captured`, `pending`, `already_pending` or
`already_completed`. It reports native revision, counters and independent anchor
retention with `authority: "none"`. After acknowledgment, archive export and
reporting are both attempted even if a deadline or output fails. Preserve the
same request and old anchor, then choose a new export path to recover a lost
acknowledgment without duplicating capture or initiation.

These modes reject raw or legacy predictive selections and all live-service,
credibility or other maintenance combinations. A topology-backed history
requires the explicit owned prompt-audit recipe and a `--topology-profile`
`fa.mediated-submit/2` selection with exact input custody and new paired outputs.
Its offline finalizer permits only an unchanged source/history cut or its own
single matching selected event after that cut; unrelated later work cannot be
covered by stale source accounting. Both archive attempts and the report retain
the native acknowledgment even if one output fails. The explicit owned
prompt-audit profile also admits `--narrow-stream-checkpoint` with an independent
exact request; its original `NarrowAuthority` consequence supplies a later
control frontier without changing policy or predictor generations. See the
[predictive lifecycle and request schema](LEARNED_PREDICTIVE_STREAM_RESET.md).
Reset **completion** supports both exact nonpredictive and owned prompt-audit
topology custody as described below.

## Complete an existing reset

This is the recovery form of the registered `reset` mutation in
`registry/vocabulary.json`: the recorded original intent carries the prior
authority, exact request matching supplies idempotency, and the original bounded
replay/audit supplies its work cost. The request document creates no new permit.

## Command and independent inputs

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --complete-stream-reset RESET_REQUEST \
  --history-anchor INPUT_ANCHOR \
  --retain-history-anchor NEW_OUTPUT_ANCHOR \
  [--topology-profile TOPOLOGY_PROFILE] \
  [--stop-profile PEER_PROFILE]
```

This is a Linux operator mode. All paths are absolute. The complete argument
list belongs to this mode before shared options are removed. Generation,
continuation, credibility activation, and other maintenance flags are not accepted.
Topology selection uses the independently retained source checkpoint described below.

`CONFIG` supplies the original bootstrap profile. This recovery mode can read an
explicit `actor.grade: "functional_restart"` declaration; it never upgrades an
`audit_only` declaration or accepts a `bit_exact` claim. The ordinary Config
reader keeps its original audit-only contract. The declaration is an independent
expected native profile, not an attestation about an external model or process.
The actual numerical/text recipe and full guarded history still have to match.

`LEARNED_RECIPE` supplies the independently retained original numerical and
text-stream contracts. If the history contains an acknowledged policy
replacement, select the full expected effective policy through the independent
[recovery policy envelope](LEARNED_RECOVERY_POLICY.md), while retaining the
original bootstrap Config. Recovery does not read current source/helper evidence,
launch inference workers, or obtain a fresh publication approval. This increment
supports plain guarded learned text streams, their nonpredictive mediated
histories, and the explicitly selected plain or mediated owned prompt-audit
stream profile.
Every other predictive history retains its native refusal and cannot downgrade
to a plain profile.

`RESET_REQUEST` and `INPUT_ANCHOR` are separately retained files outside the
mutable authority store. The anchor must be an exact original acknowledged
history prefix. The request must reproduce the independently retained original
intent and incident frontier. The operator never fills missing values from the
current journal.

`NEW_OUTPUT_ANCHOR` must be a new file outside the authority store. Its parent
must already exist. Admission rejects an existing destination; the actual
acknowledged export repeats create-new admission, writes a private file, and
synchronizes the file and parent directory. The input archive is not overwritten.

## Topology source custody

A mediated history additionally requires `--topology-profile TOPOLOGY_PROFILE`.
Use the existing closed `fa.mediated-submit/2` profile with `source_checkpoint`
and `retain_checkpoint`. The source checkpoint must contain the same exact
history anchor as `INPUT_ANCHOR`, including the original `BeginStreamReset`
intent and incident frontier. Manual v1 source minima and a checkpoint from
before the intent cannot establish this custody.

`retain_checkpoint` must name a new file outside the authority store, distinct
from `NEW_OUTPUT_ANCHOR`. Both destinations are admitted before replay. The
profile's independently selected `initial` documents still pin the original
graph. The `next` paths remain required by the existing profile grammar but are
not opened, bound, or certified by reset recovery. Current live topology,
inventory, source evidence, and helper files are not needed for this operation.

The sealed `FileStreamResetTopologyRecovery` retains the original guarded replay
and exclusive lock. Its source-cut admission allows only the selected original
`StreamReset` completion and recovery `Fence` records after the retained prefix.
A changed command, frontier, topology update, source check, time observation,
receipt reconciliation, or unrelated numerical work refuses before cleanup or
mutation. Every original guard, effective-policy requirement, current credential
epoch, and independent recovery floor still applies.

After acknowledgment, `TopologyCheckpoint::after_learned_stream_reset` pairs the
new native history with the original source documents, spent work, and last
source operation. A recovery Fence withdraws topology availability. No source
allowance is refilled, and the new checkpoint does not certify a current cut.
Subsequent live publication still requires the existing newer-source/rebind and
native source-check path.

## Request format

The closed schema is `fa.learned-stream-reset-recovery/1`. Every integer is an
unsigned canonical decimal **string**, including counts and versions. Leading
zeros, negative or fractional values, unknown fields, duplicate fields, omitted
fields, and counters outside their native range are rejected.

The example below illustrates the complete shape. Every value must be replaced
by its exact independently retained counterpart from the original intent.

```json
{
  "schema": "fa.learned-stream-reset-recovery/1",
  "checkpoint": "1",
  "first_incident_sequence": "2",
  "control": {
    "operation": "900",
    "expected_control_sequence": "2",
    "expected_actor_revision": "1",
    "expected_authority_epoch": "0",
    "binding": {
      "round": "900",
      "reducer_generation": "1",
      "evidence_root_hex": "0808080808080808080808080808080808080808080808080808080808080808"
    },
    "retained_targets": [
      {
        "adapter": "1",
        "object": "1",
        "contract_version": "1",
        "expected_version": "1",
        "generation": "1"
      }
    ]
  },
  "restart_budget": {
    "cache_values": "512",
    "audit": {
      "preparation": {
        "compression": {
          "source_values": "1000",
          "encoded_bytes": "65536",
          "work_units": "1000000"
        },
        "source_check": {
          "source_values": "1000",
          "encoded_bytes": "65536",
          "reconstruction_products": "1000000"
        }
      },
      "monitoring": {
        "encoded_bytes": "65536",
        "probe_coordinates": "10000",
        "reconstruction_products": "1000000",
        "materialized_values": "256",
        "refinements": "64"
      }
    }
  }
}
```

The request is bounded to 256 KiB, depth 8, 16,384 JSON items, and 64-byte
strings. The target list admits up to the native `MAX_ATTEMPTS` limit (1,024).
The incident frontier is nonzero and must match the recorded
`BeginStreamReset`. Native admission checks the full control request, budget,
checkpoint, predecessor, and original stream audience.

## Ordering and Stop

One original guarded cursor verifies the entire canonical history under its
exclusive recovery lock. Replay advances one event at a time and checks the
original elapsed deadline before opening, during replay, and at Ready.
A mismatch, expired deadline, or failed replay returns before staging cleanup,
reset completion, or Fence.

When `--stop-profile` is selected, the existing peer-authenticated Stop transport
is serviced at those same checkpoints. A pending checked peer takes the original
terminal recovery-Stop path at Ready **before** reset completion or live role
issuance. An acknowledged Stop attempts both its resulting history anchor and,
when selected, the operation-bound topology checkpoint, preserving their original
source work. This also covers a Stop winning during an exact retry after an
earlier reset completion outlived its export. A silent or failed admitted session
does not fall through into reset recovery.

With no pending Stop, the cursor invokes the native
`finish_pending_stream_reset` finalizer before ordinary recovery can fence the
intent. The native finalizer performs the original audit and containment
transition and publishes completion plus Fence in one canonical replacement.
It does not execute an external effect, refund spent publication rights, move
the revocation floor backward, or reset incident accounting.

The resulting owner remains internal to this command, paused and without fresh
time. A restored numerical source is Empty until the original fresh-input path
accepts new input. The explicit owned profile privately retains the separately
audited successor cache only for its next original predictor acquisition. No
actor gateway, reviewer endpoint, effect permit, or live
resume is issued by this operation. Filesystem and output calls are synchronous
cooperative boundaries; the elapsed budget is not a hard preemption guarantee.

## Continue a recovered functional history

A restored owner remains paused until a separate live operation supplies fresh
source and time and obtains the original review authority. The existing learned
continuation commands accept the same explicitly declared `functional_restart`
bootstrap profile as an independent recovery expectation. For example, a fresh
request can use the newly retained history anchor:

```text
supervise_publication create-learned-generated CONFIG NEXT_REQUEST_RECIPE \\
  REVIEWER_PROFILE --continue-generation \\
  --history-anchor RESET_OUTPUT_ANCHOR \\
  --retain-history-anchor NEW_PUBLICATION_ANCHOR
```

The next request must retain the original numerical generation and stream
contracts. Its request identity and review rounds are fresh; changing them cannot
refill lifetime numerical work, restart allowances or spent publication rights.
Actual publication still requires the native automatic review and authenticated
human approval. A mediated continuation additionally supplies the newly retained
source checkpoint and follows the existing newer-source/rebind and certification
path.

The same scoped declaration is accepted by guarded receipt recovery and service,
resident continuation, diagnosis, policy replay, credential maintenance,
abandonment and terminal Stop. Every path retains its original independent
requirements. Single and resident learned creation use their scoped explicit
functional-restart reader; unrelated creation keeps its audit-only reader.
Changing a declaration, model generation or grade cannot relabel an existing
journal, and `bit_exact` remains unsupported.

Historical review audit and situation observation also accept the explicit
functional profile when the original learned recipe is selected. They retain
the original immutable replay and report no current effect authority. Fleet
observation uses the same scoped decoding and existing input allowance; a wrong
profile still produces an Unknown member under the existing fleet contract.
Terminal shutdown fleets accept functional members only when their independent
learned recipe and history anchor are declared. Native admission Stop and passive
reobservation retain their existing full replay, accounting and custody rules.

## Outcomes and retained custody

After native acknowledgment, the command attempts history-anchor retention and
any selected topology-checkpoint retention independently, then emits one `fa.learned-stream-reset-result/1` JSON object. Acknowledgment is
separate from the original reset result:

| `outcome` | Meaning | Exit result |
| --- | --- | --- |
| `restored` | The original reset reports numerical restoration. | Success only if retention, reporting and the original deadline also succeed. |
| `contained` | The original reset acknowledges containment without restoration. | Nonzero. |
| `refused` | Native completion retained an original reset refusal. | Nonzero. |
| `unavailable` | Native acknowledgment occurred but its result could not be inspected. | Nonzero. |

The report includes the operation, checkpoint, incident frontier, revision,
control sequence, authority epoch, numerical position and sampling counters,
paused state, incident count, charged/reserved rights, original refund result,
effect execution count, restart work, and anchor-retention status.
`topology_checkpoint_retained` is `true` or `false` for a selected topology
checkpoint and `null` for a plain history. Counters are decimal strings. `authority: "none"` is explicit: this report is historical
evidence.

A late deadline, retention failure, or failed/partial stdout after acknowledgment
does not undo the reset and never triggers an automatic retry. The command
returns a nonzero diagnostic identifying acknowledgment and the failed boundary.
Retention is still attempted after acknowledgment even when the deadline has
elapsed. A final deadline check after output prevents a slow successful flush
from extending the original operation budget.

An exact retry of an already completed intent adds only the ordinary recovery
Fence. It does not repeat the reset audit, count another incident, or refill
restart work. A mediated exact retry also preserves the same source-work
counters even if an earlier output could not be retained. Preserve the same
expectation and independently retained input anchor/source-checkpoint pair,
inspect the original history, and select new output paths for any deliberate
retry. One successful export never substitutes for the missing half of its pair.
A changed expectation or newer pending operation refuses.
An intent already interrupted by Fence or Stop stays interrupted.

## Recover paired archives after a recorded Stop

If the original reset completed but its exports were lost, and an admission Stop
was subsequently acknowledged but its exports were also lost, the old source and
history pair precedes both operations. Recover that exact terminal cut with:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE \
  --recover-stopped-stream-reset RESET_REQUEST STOP_OPERATION \
  --history-anchor ORIGINAL_INPUT_ANCHOR \
  --retain-history-anchor NEW_STOPPED_ANCHOR \
  --topology-profile TOPOLOGY_PROFILE
```

The closed command requires the same independently retained reset request,
original numerical recipe, and exact v2 source/history pair, together with the
nonzero canonical decimal operation of the already recorded Stop. The topology
profile must select a new source-checkpoint output, distinct from the new history
output. It also supports an admission Stop that won while reset completion was
still pending. That reset remains pending and interrupted.

The sealed native `FileStoppedStreamResetRecovery` first proves that the matching
Stop already exists in the locked canonical history. The original complete
guarded replay then verifies the exact profile, reset intent, incident frontier,
policy, source inventory, credential epoch and independent floors. Only the
selected reset completion, admission Stop and recovery Fence suffix is accepted.
A missing or changed Stop, changed reset expectation, unrelated time/source work
or later settlement `StopProgress` refuses this repair mode.

Confirmation uses the original exact Stop retry. It appends zero reset, Stop,
Fence, clock or source events and never returns a live owner or reset finalizer.
The original canonical-byte recheck and durable confirmation still apply;
successful confirmation may clean unrelated staging under that native contract.
Dropping an incomplete or Ready cursor confirms nothing and does not clean it.

Both acknowledged archive exports are attempted before reporting, even if one
fails or the original deadline has elapsed. The command emits the existing
`fa.admission-stop/1` report with `appended_events: 0`, the original Stop receipt,
and its remaining liabilities. It does not attempt settlement. Failed output or
retention returns a nonzero diagnostic without repeating either operation. Keep
the original paired archives and choose new output paths for a deliberate retry.
A newly retained exact terminal pair also supports a zero-event repeat.

The generic terminal checkpoint recovery path remains available for its original
Stop/Fence-only successors. This dedicated mode supplies the extra independently
selected reset identity needed to recover a compound reset/Stop suffix.

## Source and verification

The operator, strict request decoder, and authored regressions are in
`examples/supervise_publication/workflow/actor_service/learned/stream_reset.rs`,
its `stream_reset/request.rs` and `recovery/stream_reset.rs` modules, and
`tests/stream_reset.rs` with `tests/stream_reset/topology.rs` and its
`native_terminal.rs` and `terminal.rs` children. The native source
successor and sealed mediated cursor are in the corresponding
`observed/driver/topology/checkpoint/stream_reset.rs` and
`observed/guarded/mediated/stream_reset.rs` modules. Recovery delegates to the
original numerical stream-reset finalizer; there is no replacement reset algorithm.

The tests use real numerical checkpoints, native review and reset operations,
canonical journals, independent anchors/source checkpoints, and command-file
parsing. Topology cases cover absent live files, lost source export and exact
retry without additional audit/incident/source work, acknowledged audit refusal,
changed intent/frontier/budget, an incomplete retained prefix, unrelated successor
work, paired-output admission, deadline cancellation through Ready, and an
authenticated Stop arriving at Ready both before numerical completion and after
an earlier completion whose exports were lost. These are
authored source regressions. The terminal repair cases additionally cover
missing or mismatched Stop refusal, original canonical-byte confirmation,
incomplete and dropped replay, independent floors, zero-event retries from old
and new custody pairs, output races, deadlines, and the actual executable grammar.
Runtime execution and type checking remain pending
under the required RCH runner.
This increment does not close FA-108 or establish a production restart grade.
