# Recover an acknowledged learned-stream reset

This operator completes an already acknowledged native `BeginStreamReset` after
a crash. It implements the plain guarded recovery path described in
[PENDING_LEARNED_RESET_RECOVERY.md](PENDING_LEARNED_RESET_RECOVERY.md) and
[LEARNED_STREAM_CONTAINMENT.md](LEARNED_STREAM_CONTAINMENT.md). It does not capture
a checkpoint or begin a fresh reset. The original intent, numerical recipe,
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
  [--stop-profile PEER_PROFILE]
```

This is a Linux operator mode. All paths are absolute. The complete argument
list belongs to this mode before shared options are removed. Generation,
continuation, credibility activation, topology, and other maintenance flags are
not accepted.

`CONFIG` supplies the original bootstrap profile. This recovery mode can read an
explicit `actor.grade: "functional_restart"` declaration; it never upgrades an
`audit_only` declaration or accepts a `bit_exact` claim. The ordinary Config
reader keeps its original audit-only contract. The declaration is an independent
expected native profile, not an attestation about an external model or process.
The actual numerical/text recipe and full guarded history still have to match.

`LEARNED_RECIPE` supplies the independently retained original numerical and
text-stream contracts. Recovery does not read current source/helper evidence,
launch inference workers, or obtain a fresh publication approval. This increment
supports plain guarded learned text streams. Predictive and mediated/topology
histories are rejected, without downgrading them to a plain profile.

`RESET_REQUEST` and `INPUT_ANCHOR` are separately retained files outside the
mutable authority store. The anchor must be an exact original acknowledged
history prefix. The request must reproduce the independently retained original
intent and incident frontier. The operator never fills missing values from the
current journal.

`NEW_OUTPUT_ANCHOR` must be a new file outside the authority store. Its parent
must already exist. Admission rejects an existing destination; the actual
acknowledged export repeats create-new admission, writes a private file, and
synchronizes the file and parent directory. The input archive is not overwritten.

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
issuance. An acknowledged Stop retains its resulting history anchor when an
output destination was selected. A silent or failed admitted session does not
fall through into reset recovery.

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

## Outcomes and retained custody

After native acknowledgment, the command attempts anchor retention and then
emits one `fa.learned-stream-reset-result/1` JSON object. Acknowledgment is
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
effect execution count, restart work, and anchor-retention status. Counters are
decimal strings. `authority: "none"` is explicit: this report is historical
evidence.

A late deadline, retention failure, or failed/partial stdout after acknowledgment
does not undo the reset and never triggers an automatic retry. The command
returns a nonzero diagnostic identifying acknowledgment and the failed boundary.
Retention is still attempted after acknowledgment even when the deadline has
elapsed. A final deadline check after output prevents a slow successful flush
from extending the original operation budget.

An exact retry of an already completed intent adds only the ordinary recovery
Fence. It does not repeat the reset audit, count another incident, or refill
restart work. Preserve the same expectation and independently retained input
anchor, inspect the original history, and select a new output path for any
deliberate retry. A changed expectation or newer pending operation refuses.
An intent already interrupted by Fence or Stop stays interrupted.

## Source and verification

The operator, strict request decoder, and authored regressions are in
`examples/supervise_publication/workflow/actor_service/learned/stream_reset.rs`,
its `stream_reset/request.rs` module, and `tests/stream_reset.rs`. Recovery
delegates to the original selected cursor and native stream-reset finalizer;
there is no replacement reset algorithm.

The tests use real numerical checkpoints, native review and reset operations,
canonical journals, independent anchors, and command-file parsing. Runtime
execution and type checking remain pending under the required RCH runner.
This increment does not close FA-108 or establish a production restart grade.
