# Recover an acknowledged learned-stream reset

This operator completes an already acknowledged native `BeginStreamReset` after
a crash. It implements the plain and nonpredictive mediated guarded recovery paths described in
[PENDING_LEARNED_RESET_RECOVERY.md](PENDING_LEARNED_RESET_RECOVERY.md) and
[LEARNED_STREAM_CONTAINMENT.md](LEARNED_STREAM_CONTAINMENT.md). It does not capture
a new numerical checkpoint or begin a fresh reset. The original intent, numerical recipe,
confirmed audience, and incident frontier must already exist.

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
supports plain guarded learned text streams and their nonpredictive mediated
histories. Predictive histories retain their native stream-reset refusal and are
never downgraded to a plain profile.

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
accepts new input. No actor gateway, reviewer endpoint, effect permit, or live
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
requirements. Ordinary creation continues to use the audit-only Config reader.
Changing a declaration, model generation or grade cannot relabel an existing
journal, and `bit_exact` remains unsupported.

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

A terminal Stop is a separate recovery boundary. If only Stop/Fence followed the
original input pair and a Stop export fails, use the existing terminal checkpoint
recovery path with new output destinations. The reset command does not reopen a
stopped suffix. There is one explicit remaining compound-loss limitation: if a
reset completion already outlived its exports, a later Stop is acknowledged, and
its successor exports also fail, the original pair now precedes both reset and
Stop records. The current generic terminal repair cannot accept that mixed suffix,
and this increment does not add a dedicated terminal-only repair for it. Preserve
the original paired archives, canonical stopped history, and any received Stop
receipt for a separately reviewed native recovery. Retrying `--complete-stream-reset`, supplying manual
source minima, or weakening expected guards cannot repair that custody.

## Source and verification

The operator, strict request decoder, and authored regressions are in
`examples/supervise_publication/workflow/actor_service/learned/stream_reset.rs`,
its `stream_reset/request.rs` and `recovery/stream_reset.rs` modules, and
`tests/stream_reset.rs` with `tests/stream_reset/topology.rs`. The native source
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
authored source regressions: runtime execution and type checking remain pending
under the required RCH runner.
This increment does not close FA-108 or establish a production restart grade.
