# Topology-supervised subsequent publication

## Operator entry point

The existing guarded submission command now has explicit topology selection:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- submit CONFIG.json SUBMIT.json \
  --guarded /operator/custody/recovery.json \
  --topology-profile /operator/custody/next-topology.json
```

`recovery.json` remains the original `fa.guarded-submit/1` profile: independently
retained journal/control/authority floors, an exact external history anchor, and
optional create-new successor archive output. No default floor is introduced.
The new selector is separate from creation, stop, receipt and investigation profiles:

```json
{
  "schema_version": "fa.mediated-submit/1",
  "initial": {
    "topology_path": "/operator/custody/initial-topology.json",
    "inventory_path": "/operator/custody/initial-inventory.json"
  },
  "current": {
    "topology_path": "/operator/custody/current-topology.json",
    "inventory_path": "/operator/custody/current-inventory.json"
  },
  "available": true,
  "next": {
    "topology_path": "/operator/live/next-topology.json",
    "inventory_path": "/operator/live/next-inventory.json"
  },
  "operation": 100,
  "source": {
    "checks": 2,
    "reserved_read_bytes": 4194308,
    "last_operation": 2
  }
}
```

Paths, counters and operation above illustrate the shape, not deployment defaults.
`initial` and `current` name the complete independently retained original/current
contracts BEFORE recovery, including withdrawn current graphs. `available` must
match that selected history. Neither graph nor availability is inferred from the
journal. All pairs use the original closed topology/inventory parsers and bounded
regular-file intake; their resolved paths must be distinct and outside the store.
All loaded graphs must bind the original resource and scope. Content versions are
still checked independently against each actor's frozen target.

For a NEW request, `next` supplies explicitly newer graph AND inventory generations.
Its source limits and checker budget must match the independently supplied current
contract. `source` carries independently retained spent work and the last source
operation; the exact reservation/count relation is checked, never guessed from
journal records or reset to zero. Remaining allowance must cover BOTH initial and
final checks. `operation` must exceed the retained source floor and every native
update ID, with room for both subsequent checks. The `next` paths are the fixed
source paths from that independently retained context, now containing the explicitly
provisioned newer inputs. The original `recovery_requirements` API captures that
context; neither the journal anchor nor the existing creation CLI exports these
source counters automatically. No existing store is created or replaced on a missing-path error. Unknown/duplicate options refuse;
there is no topology-to-legacy or checked-witness fallback.

## One original owner through recovery and publication

The command retains the original Store lock during anchored mediated replay.
Initial/current topology, availability, original source/publication guards,
effective policy, independent floors, absent optional guards and the installed
terminal reserve are checked through `FileMediatedRecovery`. Replay yields after
each original event, with a distinct Ready deadline checkpoint. The native
Fence/Time/Sweep replacement then reconciles prior deliveries. Both its individual
outcomes AND retained unknown/dispatching/irrecoverable stages must be settled
before another request can enter the actor gateway.

The fresh observer reconnects through the existing
[source-continuity API](TOPOLOGY_SOURCE_CONTINUITY.md), which composes
[withdrawn source custody](TOPOLOGY_SOURCE_RECOVERY.md) with independently retained
work and the maximum native/source operation floor. The same custodian applies
the original revoking rebind and enters the shared creation/continuation workflow. Original fresh policy capture and native initial topology certification precede admission. Time is observed again after
capture. Helpers, independent human approval, two-key dispatch, final post-policy
topology refresh, original publication/reconciliation and child cleanup are shared
with [initial topology creation](TOPOLOGY_SUPERVISED_CREATION.md), not reimplemented.

The actor must supply its exact intended post-recovery/rebind policy epoch and
current endpoint version. This command does not rewrite those values, the payload,
request ID or deadline, and the legacy proposal helper is not made topology-aware
by this change. Additional optional credibility activation keeps its original
explicit input, ordering and epoch rules; this profile does not admit unpinned
predictive, evaluated, learned, stream or checked-witness deployments.

## Retries, loss and retention

For exact paired custody, select the existing `fa.mediated-submit/2` profile
instead of the manual v1 counters:

```json
{
  "schema_version": "fa.mediated-submit/2",
  "initial": {
    "topology_path": "/operator/custody/initial-topology.json",
    "inventory_path": "/operator/custody/initial-inventory.json"
  },
  "source_checkpoint": "/operator/custody/source-before.checkpoint",
  "next": {
    "topology_path": "/operator/live/next-topology.json",
    "inventory_path": "/operator/live/next-inventory.json"
  },
  "operation": 100,
  "retain_checkpoint": "/operator/custody/source-after.checkpoint"
}
```

The checkpoint's complete history cut must match the independently selected
history anchor in `fa.guarded-submit/1`. Select a separate new
`retain_history_anchor` destination in that recovery profile. Both outputs are
admitted before recovery acknowledges its Fence/Time/Sweep transaction. Their
normalized destinations must differ, remain outside the authority store, and
not alias the selected reviewer or Stop socket. Existing files are never
overwritten. The native create-new writers check again when exporting, so a
later competing file remains its creator's property.

An existing request is query-only even when its original deadline has elapsed.
Its independently retained historical topology is still verified, but `next`
files, live policy evidence, credibility files, helper programs and human sockets
are not opened. The original gateway enforces byte-identical retry equality;
a changed request is an idempotency conflict, never a second publication. Current
requirements must describe the newly selected history on each later invocation;
recovery does not silently change an old profile's current graph or availability.

An existing settled request can also retain its matched successor checkpoint
without reading the next topology or inventory. The original native recovery
finalizer supplies the checkpoint for the exact reconciled cut. The command
does not create fresh source evidence to service that historical request.

New-input failures after recovery may leave acknowledged recovery work. Before
source binding, the command attempts both selected exports from the original
native reconciled cut. A missing next file, bad next selection, expired deadline
or returned preparation failure therefore need not strand the history beyond
its retained source accounting. Export requires the same healthy owner's
complete history anchor still to equal that saved cut. A later native event or
poisoned owner refuses the old source checkpoint.

After source binding succeeds, retention instead captures the actual original
source, its newer graph, cumulative checks, reserved bytes and operation floor.
It never falls back to the earlier recovery checkpoint after acknowledged source
work. Both exports are attempted independently, and export errors accompany the
original failure. A partial export is not a successful pair. No retention path
reopens recovery, refreshes a source, refunds work or retries publication.

For a corrected invocation, explicitly select the retained successor pair.
Failure before binding leaves the selected newer graph eligible; failure after
binding requires the next graph and inventory generations to advance again.
The original finite source allowance and all fresh publication checks remain.
This covers returned errors, not process termination before export or recovery
of an unacknowledged replacement. Manual v1 selection retains its original
independent-minimum contract and cannot mint an exact source checkpoint.

Live rebind failure retains native poisoning/visibility rules. Source loss
at final publication reaches the original withdrawal/sealing path; uncertainty
never creates a refund or a resend. An output or archive failure cannot undo an
acknowledged effect. The old archive is never overwritten. Stop profiles keep
[their separate mediated terminal path](MEDIATED_STOP_OPERATOR.md).

## Costs, bounds and verification status

Historical admission loads two bounded input pairs. New work loads a third pair,
then the original custodian reserves its initial and final refresh allowances.
Existing-request retries do neither new-source step. The original event, byte,
policy-source, helper and terminal reserve bounds remain unchanged. A single
monotonic elapsed budget covers history intake/replay/next intake and continues
into the live workflow; synchronous reads, one original event and persistence
barriers are not hard interruptible-time guarantees.

The resumed source preserves the independently supplied work; it cannot discover
an omitted newer counter snapshot or protect joint rollback of journal and custody.
It is not a claim of authenticated global observation quotas. This is not a continuous
watcher, persisted mandatory-source flag, OS perimeter attestation, remote-effect
adapter or deployment qualification. Omission is not automatically repaired: callers
must explicitly select this mode for later topology-supervised live publication.

Ten authored operator regressions (plus a synthetic subprocess entrypoint) cover
second publication and cumulative accounting, absent-live-assets receipt retry and
changed-document conflict, source loss after dispatch, every pre-fence elapsed
checkpoint, mismatched history, missing/old/widened next input, stale actor epochs,
human rejection, archive/output loss, retained-work exhaustion versus exact remaining
capacity, the actual top-level command and closed mode selection. These are enforcement fixtures, not helper-calibration evidence.
Targeted and full-gate RCH attempts stop before compilation because `rch` is absent.
Rust compilation, tests, rustfmt, Clippy and complete qualification remain unexecuted.
No Beads or roadmap packet is closed by these source changes.

The paired-retention increment adds source regressions for receipt-only export
with absent live inputs, corrected subsequent publication after failed startup,
pre-Fence output refusal, competing archive paths, deadlines and post-bind source
accounting. Independent source review and scoped formatting are distinct from
execution. The required RCH runner is unavailable in this session; compilation,
runtime tests, Clippy and complete qualification remain pending.
