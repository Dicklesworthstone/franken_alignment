# Terminal recovery of a mediated source-publication domain

## Implemented boundary and status

The existing operator entry point accepts an explicit `fa.guarded-stop/2`
recovery profile:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- recover-stop CONFIG.json 900 \
  --guarded STOP_PROFILE.json
```

This is terminal recovery of an existing domain, not new publication, a weaker
recovery fallback, or a new command. It uses the original `FileMediatedRecovery`
and [native terminal completion](MEDIATED_STOP_RECOVERY.md). The same source
policy, effective policy, full-input committee and human-review bootstrap,
independent history anchor, floors and terminal reserve remain mandatory.
The source file and helper/reviewer processes need not be available.

This source is **unqualified**. The eight new operator regressions have not
executed. The editing environment has no RCH launcher or Rust toolchain; no
compilation, formatting, Clippy, runtime, deployment or performance pass is
claimed. FA-014 and the mediation work remain open. Existing tests and native
encoders, reducers, canonical replacement and result rendering are unchanged.

## Independent recovery profile

```json
{
  "schema_version": "fa.guarded-stop/2",
  "minimum": {
    "journal_revision": 17,
    "control_sequence": 5,
    "authority_epoch": 1
  },
  "history_anchor": "/operator/custody/before.anchor",
  "retain_history_anchor": "/operator/custody/after.anchor",
  "topology": {
    "initial_topology": "/operator/custody/initial-topology.json",
    "initial_inventory": "/operator/custody/initial-inventory.json",
    "current_topology": "/operator/custody/current-topology.json",
    "current_inventory": "/operator/custody/current-inventory.json",
    "available": true
  }
}
```

The numbers and paths above are examples, not values to infer from the journal
being recovered. Provision the exact independently retained values for the
selected domain. The initial pair describes the original enabled graph; the
current pair describes its last registered graph, even after coverage was
withdrawn. Each pair uses the unchanged `fa.authority-topology/1` and perimeter
inventory decoders. Original and current files may be identical when the graph
never changed. Graphs and availability are compared by the original mediated
checker; parsing never installs or certifies a graph.

All input paths must be absolute, resolve outside the authority-store directory,
and pass the existing bounded regular-file reader. Inputs missing, malformed,
symlinked at their final component or located within that directory refuse
before recovery writes or the terminal clock. Trusted parent directories are
still required; path checks are not protection against hostile filesystem races
or an independently provisioned but dishonest custodian.

`available` describes the required canonical image **before this recovery's stop**.
A completed stop withdraws coverage. Retrying that acknowledged stopped image
therefore requires `available: false`, not an automatic substitution of the
previous live requirement. A matching drained retry uses the same native
no-clock, no-new-event confirmation, including at full journal capacity.
It still validates the independent requirements and history.

The optional output path is create-new only. Keep the previous archive until
new custody is acknowledged. Canonical stop acknowledgment precedes output
retention and reporting. If output is lost or a competing writer claims the
archive path, the stop remains committed and the old archive is not replaced;
use the exact same stop operation and a new output path to recover its result.

## Compatibility and control behavior

Version 1 remains the original nonmediated source-publication profile and
rejects a mediated journal. Version 2 requires its topology object and rejects a
nonmediated journal; no stored graph, availability, floor or optional gate is
accepted by default. `resume`, `submit` and read-only proposal investigation do
not accept the new terminal-only profile. Prediction, evaluator, numerical,
identity, stream, campaign and credential configurations are not silently
admitted: this command version requires their absence through the original
complete guard inventory. More elaborate deployments retain their explicit
library recovery entry points.

The command retains one elapsed budget from recovery startup through anchor and
topology intake, each original replay event and the Ready boundary. Expiry at
any checkpoint drops the private reconstruction without stop/fence, staging
cleanup, source acquisition, role issuance or journal replacement. Reads,
parsing, one original event and final storage barriers are synchronous; these
checkpoints do not bound kernel latency or interrupt an in-flight syscall.
Configuration/profile parsing performed by the outer CLI precedes this budget.

After full verification the command selects the exact native stop request and
consumes the cursor into **Stop -> Fence -> StopProgress**. It does not first
spend an ordinary recovery fence, read fresh evidence, launch helpers or issue
effect keys. Original endpoint observations alone settle pending effects;
unknown or expired receipts retain their charges. Returned data contains only
the original stop report and exact acknowledged archive, not a live owner.

## Cost and authored regression coverage

The profile keeps its original 16 KiB/depth/item limits. Topology intake performs
four separately bounded reads (two topology documents and two inventories),
plus the original reader's EOF checks, and uses the existing per-file ceilings.
Only one pair's input bytes is decoded at a time; both resulting graph
requirements remain retained alongside the original replay state. Identical
paths are not assumed to be cached. This is an explicit bounded intake cost,
not an execution-time or measured memory guarantee.

`examples/supervise_publication/recover_stop/guarded/mediated_tests.rs` exercises
the actual `recover-stop` command and its original renderer, separately
registered initial/current graphs, exact archive output and no-clock retry;
mode/schema separation and legacy success; native guard/floor/source mismatch;
missing/malformed/symlinked/in-store inputs; every elapsed checkpoint; exhausted
ordinary space with the original terminal reserve; and lost output plus racing
archive creation with corrected retention. Its fixtures use actual canonical
files and the original topology loader/checker. They are not OS-containment,
helper-accuracy or durability qualification evidence.
