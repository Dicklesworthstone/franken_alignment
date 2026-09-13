# Durable actor checkpoints and containment reset

Consumer: the trusted host restoring the original file-backed oversight actor
while retaining its full-input congress, mandatory human key and effect ledger.
This implements plan 11.2/11.10 and FA-108 at the existing durable boundary, using
the original ContainmentAuthority rather than importing saved balances or permits.

record_actor_state stores an explicit bounded ActorState observation through the
original state-replacement operation. State/schema qualification remains a trusted
host responsibility. The immutable bootstrap profile cannot be changed through a
state update. Epoch and actor-revision predecessors prevent delayed pre-recovery
writes, even though reopening does not itself change actor revision. Exact update
retries return their historical acknowledgment without another actor mutation.

capture_actor_checkpoint snapshots the CURRENT native actor and retains the native
checkpoint handle privately. The journal records the capture operation, not an
independently supplied checkpoint image. Reopening rebuilds that native handle by
replaying capture. Public handles contain only immutable data identity and a live
owner brand; reacquiring one after recovery cannot restore permissions. A foreign
or pre-recovery handle cannot be presented to another owner, even with matching IDs.

reset_actor requires that handle plus an explicit supervisor operation and exact
actor/control/authority predecessors. It calls the original reset: restore token,
cache and sampler state; intersect the target ceiling; advance the revocation floor
and incident counter; cancel undispatched work; refund only its reservations.
At the original incident threshold, the native controller suspends without restoring
actor memory. Neither state updates nor checkpoint replay erase that threshold.
Current policy, original resource totals and dispatched/unknown/executed effects
never enter a checkpoint and never rewind. Human-key withdrawal and retirement of
old helper sessions are committed together with reset in one canonical replacement.

An exact reset retry is acknowledgment recovery, not another incident. Its receipt
remains historical after later work, policy changes, suspension or reopening.
Changed operation fields conflict. Original request identities and cancelled
reservations cannot become fresh work. New work requires a new request, current
evidence, the advanced epoch, original congress and independently issued human key.

Reset is not an endpoint fence. Issued envelopes retain their original ordering;
the optional first-publication guard can independently seal now-invalid authority.
Historical execution wins, and sealing requires separate reconciliation before a
charged effect can be refunded. Stop/drain stays available without actor state,
helper inputs or another human approval. A terminal stop cannot be reset away.

State reads and exact retries refuse on a faulted owner, because disk may contain
an unacknowledged replacement. The existing store remains exclusively owned and
replay remains memory-only. Recovery requires the original bootstrap and path;
no clock, authorization, human key or sendable envelope is imported from disk.
The new outer journal tags 16/17/18 carry only state-update/capture/reset inputs;
all previous event and bootstrap bytes remain intact. The simpler FileDelivery
profile does not silently gain these events.

Each original state constructor limit applies. The file profile additionally keeps
at most 128 update identities and 8 MiB of retained logical update payload; exact
retry cannot evict or replenish this allowance. Native checkpoint count/byte limits,
incident and round limits, and the original journal caps apply independently.
These are logical retention bounds, not measured peak-memory or latency claims.

Six integration functions and two codec tests cover all-owner reconstruction, state/capture/reset
idempotence, late prior-epoch observations, old-key refusal, scoped fresh publication,
unknown/executed obligations, repeated incident escalation, monotone narrowing,
original qualification, terminal stopping and lifetime limits. A compile-fail
example keeps checkpoint data distinct from effect permits. Native reducers and
older tests are unchanged apart from a crate-private read-only actor-state accessor.

Rust compilation, tests, formatting and Clippy have NOT run: the required RCH
command could not start because rch is unavailable (127). This is not production
or crash qualification. No dependencies, Beads state or historical evidence changed.
The original cache/sampler bytes are host-supplied observations; this does not yet
instantiate or restore a numerical decoder inside the durable owner. Authentication,
anti-rollback storage, hostile-process containment and full hosted-decoder migration
remain separate requirements. Restoring actor memory does not undo external effects.
