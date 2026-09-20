# Coupled witness-image and change-feed production

`publication::witnesses::producer::PublicationProducerImage` builds the input
snapshot, producer generation, included change cut, retained notifications and
producer heartbeat together. It serves the original FA-061/062 consumers and
plan sections 7.6–7.8. It does not authenticate external data or grant authority. Its fixed effect scope
prevents binding the same image to an action in another tenant/run/branch context.

A successor takes the exact heartbeat-generation predecessor, a complete bounded
`FilePublicationInputs` value (whose lanes may explicitly be unavailable), and a
producer observation tick. Callers do not supply notifications or a new included
cut. The producer derives them by comparing old and new input images.

For the same structured domain and interpretation, insertion, deletion, value
changes and version changes emit sorted exact-key notifications. Changes to
semantic/domain/closing metadata, disappearance or appearance of a lane, and any
whole opaque-input change conservatively emit All. Revision/control-cut movement
alone does not invalidate exact witnesses. More than 256 changed keys becomes one
Domain notice, never a truncated list. The existing index still treats every
notice as relevant to opaque judgments; final exact validation remains mandatory.

The retained window contains at most 256 consecutive notices. Eviction advances
its explicit `after` position; it does not assert that an older consumer caught
up. Existing ingestion detects a missing prefix and refuses until real coverage
is supplied. Missing lanes retain structured revision/control/semantic floors.
Counters use checked arithmetic, and failed successors leave the old image intact.

Heartbeat generation advances on an explicit producer observation. The snapshot
producer generation advances only when the input image changes; rereads change
neither. `capture(attempt, action)` generates the original FAPCAP02 representation
with its actual included cut, and `batch()` exposes the original feed format.
One canonical FAPPRD01 bundle stores both. All prior packet/entry/value limits
remain; the bundle is bounded by 3 MiB + 32 KiB + 256 bytes, independent of the
numerical source sequence. It contains no permit, judgment-success bit or secret.

The producer guarantees complete differences between the input images handed to
it, not observation of every real-world event between those images. Unobserved
transient changes, producer honesty, filesystem isolation and shared elapsed time
remain external assumptions. Historical bundle decoding is not an independent
proof that the producer derived earlier notices correctly. Production activation
still requires the repository's exact-revision verifier and source qualification.

Ten new tests cover exhaustive small insert/delete pairs, value/version changes,
u64 endpoints, metadata/lane/opaque changes, bounded coarse notifications, separate
heartbeat and input generations, rolling overlap, monotone floors and canonical
packets. They are authored but UNEXECUTED: RCH/Rust/Cargo are absent in this editing
environment. No Bead closure or performance/production claim is made.
