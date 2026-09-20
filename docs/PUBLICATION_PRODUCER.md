# Coupled witness-image and change-feed production

`publication::witnesses::producer::PublicationProducerImage` builds the input
snapshot, producer generation, included change cut, retained notifications and
producer heartbeat together. It serves the original FA-061/062 consumers and
plan sections 7.6–7.8. It does not authenticate external data or grant authority.
Its fixed effect scope prevents binding the image to an action in another
tenant/run/branch context.

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
numerical source sequence. It contains no permit or judgment-success bit; source input bytes may be sensitive.

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

## Locked persistence and existing consumers

`FilePublicationProducer::create` initializes one private producer directory using
the ORIGINAL Store lock, staged replacement and file/directory barriers. `publish`
takes the exact heartbeat-generation predecessor, actual next input image and
producer tick; it derives the suffix and cut and replaces the whole bundle ONCE.
It cannot acknowledge a new snapshot with an old feed file or vice versa. Before
replacement it compares the actual canonical bytes to the retained predecessor,
so an observed out-of-band edit is not overwritten or called an idempotent retry.

An exact retry after success/reopen verifies that canonical image and returns
AlreadyCurrent without changing any generation, notification or expiry. A failed
read, replacement or caught storage unwind quarantines the writer. Recovery pins
the whole expected profile and an independently supplied minimum generation,
validates the canonical bundle BEFORE cleanup, then discards only staged leftovers.
It never promotes an unacknowledged pending image. A failed producer write does
not itself revoke a consumer lease; consumer acquisition, expiry and invalidation
checks remain independently required. The same directory cannot
supply trustworthy independent rollback protection for itself.

`witness_reader(attempt, action)` and `feed_reader()` return the EXISTING concrete
PublicationInputFile and PublicationFeedFile types. Their explicit from_producer
constructors pin the complete producer profile and choose bundle decoding; raw
legacy constructors remain strict and do not silently reinterpret another format.
Every acquisition reopens the canonical file with the original regular-file,
identity/metadata and overflow-sentinel checks. Both projections decode the whole
bounded bundle before returning the original FAPCAP02/feed types. The feed read
therefore has the bundle byte ceiling, not the standalone 32-KiB file ceiling;
the extracted notification window retains its original 256-record/32-KiB bounds.

Configure the original validation/change/freshness gates, read the initial bundle,
and bind its original capture through bind_publication_file_source. Pass the two
readers to the existing refresh, supervised-step or completion entry points. No
new driver, journal event, endpoint or authority API is introduced. Later publish
calls automatically generate the notices consumed by the original invalidation
index; final exact checks still distinguish unrelated changes from stale review.

The two consumer reads are sequential. A producer replacement between them can
make the later snapshot ahead of the consumer's feed prefix; the EXISTING input
cut checks refuse that mismatch rather than assuming a shared observation time.
This is atomic producer storage, not an atomic transaction over consumer reads,
other policy files, or remote effects. Bundle file access must be limited to
principals authorized for the FULL input; it is not a lower-privilege feed-only
endpoint. File locks/metadata checks are not an adversarial-filesystem sandbox.

Eight file/native-gate regressions and one real helper-socket completion test pair
successful publication with derived negative changes, stalled heartbeat refusal,
inter-read replacement, profile/floor pinning, exact retries, storage failure,
out-of-band edits and strict reader bounds. One unit regression injects all five
original storage barriers and checks old-or-complete recovery plus exact retry.
Together with the ten image tests, 20 Rust tests and one compile-fail boundary are
authored. None has executed here. The required RCH xtask command again exits 127
because rch is absent; cargo/rustc are absent too. Source/hash/lexical screens are
not compiler, Clippy, formatting, runtime or hardware power-loss qualification.
FA-061/062 remain open; no production activation or source-authentication claim.
