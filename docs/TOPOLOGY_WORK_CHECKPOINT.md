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
work after the latest successful retention are not detected. A source checkpoint
is not current evidence or a cut certificate. Actual recovery must use its
history anchor and full independently selected mediated requirements, then the
original native fence, newer graph/inventory rebind and fresh source checks.
No remaining allowance is replenished by import, capture, refusal or recovery.

Export does not flush or synchronize a caller's writer. A complete private file
must be independently acknowledged by its custodian before relying on it. Failed
or partial output cannot undo publication or authorize a resend. Retain the old
archive until the successor is acknowledged. Operator integration must preserve
the SAME source through completion and export before releasing its custody.

## Verification status

Five authored native regression functions cover exact history/work round trips,
real mediated recovery and native retained-work resumption, acknowledged source
loss, foreign/poisoned capture, changed expected input, bounded framing, every
truncation, trailing data, fragmented/interrupted input and failed output. Existing
codecs, reducers and source/driver checks are unchanged. These are not executed
results: the remote-only RCH launcher is absent in this editing environment.
Rust compilation, tests, rustfmt, Clippy and full qualification remain pending.
The editing checkout is partial. No Beads, roadmap or production gate is closed.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference \
  action::consequence::delivery::persistent::observed::driver::topology::checkpoint
```
