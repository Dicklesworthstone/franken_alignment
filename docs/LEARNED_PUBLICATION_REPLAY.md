# Reuse acknowledged learned history during publication checks

Publication evidence must be read again at its original boundaries. Replaying
the entire numerical history for every evidence acquisition is separate work:
an already acknowledged, process-local predecessor can supply its verified
prefix while every subsequent journal event is still replayed normally.

## Consuming replay custody

`FileLearnedReplayContinuation::retain_for_control` moves an existing opaque
continuation into the same live owner's existing replay-cache slot:

```rust,ignore
let revision = host.revision();
carry.retain_for_control(&mut host, revision)?;
```

The continuation can only originate from an acknowledged original transition.
Handback checks its issuer, the current journal revision, storage availability
and prefix bounds. It also validates any already retained continuation and keeps
that value if its verified prefix is newer or equal. The transfer consumes its
input even on refusal. It neither reconstructs a runtime from saved bytes nor
exports another live owner, writer, reviewer, forecast or effect key.

Handback performs no journal write, inference, source read or clock observation.
It does not resume a paused generation or change a work allowance. The original
control operation still has to replay the entire suffix after the retained
prefix. A failed suffix replay is not retried cold inside the same operation.
Recovery creates a new live owner and still requires independent recipe binding
and complete historical replay; a pre-recovery continuation cannot be installed
into that new owner.

The cooperative forecast stepper returns its final acknowledged continuation
when numerical generation becomes inactive. Its original forecast, numerical
result and interruption latch remain authoritative. An acknowledged numerical
failure remains a failure, including when its historical predecessor is retained
for subsequent control work.

The runnable learned operator returns its generic generation carry when the
completed prompt hands sampling to the original forecast stepper and when a
successful output reaches its required stop token. An interrupted session,
unfinished preparation, pending intent or changed numerical predecessor refuses
that transfer. Existing Stop, deadline and fresh-source checkpoints remain
between the same numerical quanta.

## Publication acquisition and acknowledgment

Publication input withdrawal, producer freshness persistence and plain capture
installation use the existing control-candidate replay and persistence helpers.
Coupled producer ingestion uses the same candidate rule. Its cache receives the
actual retired live machine only after successful storage replacement and records
the **pre-batch** event count. Every notice, heartbeat and capture in that batch
therefore remains in the suffix that the next operation must replay.

Original preflights, withdrawal before external I/O, canonical encoding, source
admission, conflicting-overlap checks, history completeness, freshness, storage
faults and acknowledgment boundaries remain in force. No current evidence is
cached by this optimization. Owners without an existing continuation retain the
original cold replay behavior until acknowledged generation supplies one.

Feed overlap checks also recognize a bootstrap policy inside the independently
bound learned Enable configuration. This reads the exact supplied runtime recipe;
it does not infer a policy or runtime authority from journal bytes.

This is a reference execution improvement, not a performance SLO or complete
release qualification. Current execution results and limitations are recorded in
[implementation status](../IMPLEMENTATION_STATUS.md).
