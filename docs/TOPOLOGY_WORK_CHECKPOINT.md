# Topology work checkpoints

`driver::topology::checkpoint::TopologyCheckpoint` pairs the original selected
source's actual work and last operation with the same healthy owner's exact
history anchor, control/authority floors and historical topology availability.
Its consumer is topology-supervised operator continuation: an operator should
not have to guess how many initial/final/failed checks the previous run spent.
The original source and recovery bridge still own all quotas and rebind rules.

Capture is `FileDriverTopology::checkpoint(host, topology_bytes, inventory_bytes)`.
The input bytes must be the retained original expectations, not rereads of files
that may have changed during publication. The original bounded parsers reconstruct
the COMPLETE loaded input, including unselected inventory metadata, and compare
it to the source. Source/owner identity and native graph binding must match.
A poisoned owner cannot capture a supposedly acknowledged checkpoint. An
acknowledged withdrawal can: it preserves spent work and unavailable coverage.

The checkpoint retains private journal and inventory content. Debug is redacted;
it is never an actor response. Capture has no file I/O, clock, source refresh,
journal transition, permit or callback. It synchronously parses, copies the two
bounded input documents and encodes the acknowledged original journal.

## Transport and recovery

`write_to` preflights the complete archive before writing. `read_trusted` checks
all lengths before allocation, uses the original input parsers and history-anchor
reader, requires exact nested extent and real outer EOF, and rejects malformed
flags/counter relations. Each input remains at most its original 1 MiB allowance;
the nested anchor keeps its original limit. The total ceiling is the sum of those
limits and the 73-byte checkpoint header. No journal wire format changes.

The archive is independently trusted custody, NOT authentication. Its counters
are captured rather than asserted by the live caller, but imported bytes are
only as trustworthy as their custodian. Joint rollback, counterfeit archives and
unrecorded work after the latest successful retention are not detected. A source
checkpoint is not current evidence or a cut certificate. Actual recovery must use
its history anchor and full independently selected mediated requirements, then
the original native fence, newer graph/inventory rebind and fresh source checks.
No remaining allowance is replenished by import, capture, refusal or recovery.

Export does not flush or synchronize a caller's writer. A complete private file
must be independently acknowledged by its custodian before relying on it. Failed
or partial output cannot undo publication or authorize a resend. Retain the old
archive until the successor is acknowledged. Operator integration must preserve
the SAME source through completion and export before releasing its custody.

## Driver capture and checkpoint-backed resumption

`FileSupervisedDriver::topology_checkpoint(revision, topology, inventory)` captures
its selected source and the same healthy owner at the explicit revision. This is
a data export, not another source read; missing live files do not invalidate the
already acknowledged capture, and a stale revision or foreign owner is refused.

After original guarded recovery, use `resume_topology_checkpoint` with separately
recovered observer custody, the retained checkpoint, an explicit fresh operation
and `TopologyCheckpointResume` naming independently provisioned next files and
complete next input. The consumer validates the checkpoint's exact history prefix
and control/authority floors before invoking original source resumption. Use the
checkpoint's anchor and floors in the recovery cursor as well: this consumer does
not retroactively validate a recovery fence already performed through another API.

An append-only history can be valid yet contain more source expenditure than an
older checkpoint records. The consumer therefore rejects ANY native mediation
event after the retained checkpoint. Ordinary recovery fences, time observations
and reconciliation are allowed; they do not replenish source allowance. This
refusal is deliberately conservative, including topology events that might not
have spent reads. It cannot reconstruct unrecorded work lost before acknowledgment.

The original driver resumption requires unavailable coverage, newer graph AND
inventory generations, unchanged source/checker limits, and room for both initial
and final checks. It installs source custody before reading actual next inputs.
A failed source read preserves native withdrawal and expenditure; an old cut or
effect key is never restored. Capture a successor only from the healthy original
owner. This adds library integration, not automatic CLI checkpoint export/import.

## Terminal checkpoint successors

`TopologyCheckpoint::after_admission_stop(profile, identity, history)` accepts a
sealed `FileAdmissionStoppedHistory`; `after_stopped` accepts a sealed
`FileStoppedHistory` from combined recovery, later drain or an exact retry.
They carry the independently retained source custody to the same acknowledged
terminal cut. The original bootstrap profile and canonical store identity must
match. The original decoder and exact prefix checker validate the captured
journal; only the matching Stop, Fence and StopProgress events may follow the
saved cut. Even an ordinary time observation or another actual source check
refuses an older source checkpoint.

Topology/inventory bytes, source expenditure, last operation and limits remain
unchanged. The exact acknowledged anchor and control/authority floors advance
with the terminal result. Historical availability follows the actual native
events: Fence withdraws it, while Stop alone does not fabricate withdrawal.
This data is neither a live source nor a fresh cut certificate. The methods
perform bounded parsing/copying, without file access, clocks, semantic replay,
journal mutation or new authority. Captured acknowledgement remains the selected
cut even if canonical or source files subsequently change or disappear.

Durable export remains separate from native acknowledgement. Trusted custody,
joint rollback and unretained work retain their existing limits. The consumer
must keep the prior archive until the new exact checkpoint is durably retained.

## Verification status

The original five authored native regression functions cover exact history/work
round trips, real mediated recovery and native retained-work resumption,
acknowledged source loss, foreign/poisoned capture, changed expected input,
bounded framing, every truncation, trailing data, fragmented/interrupted input
and failed output. Five additional recovery regressions cover two serialized
checkpoint-backed recoveries, revision/owner binding, stale versus current
checkpoints after actual source work, foreign histories and invalid profiles,
floors or budgets, and source loss after reinstatement with retained expenditure.
Existing codecs, reducers and source/driver checks remain unchanged.

The original results below are historical. Six public terminal regression
functions in `tests/topology_terminal_checkpoint.rs` cover exact native
admission/drain/combined cuts, missing live files, unchanged expenditure,
nonterminal successors, equal-counter forks, binding and truthful Stop-only
availability. They pass with the qualified nightly's official Cranelift backend.
Five related internal tests remain authored; building the complete library test
harness was killed by SIGKILL on the shared host. Current execution evidence and
qualification limits are recorded in IMPLEMENTATION_STATUS.md.

The original tests were authored, not executed results. Fresh targeted and full-gate remote-only
RCH commands stopped before compilation with exit 127 because the launcher is
absent. Rust compilation, tests, rustfmt, Clippy and qualification remain pending.
The editing checkout is partial. No Beads, roadmap or production gate is closed.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  action::consequence::delivery::persistent::observed::driver::topology::checkpoint
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
