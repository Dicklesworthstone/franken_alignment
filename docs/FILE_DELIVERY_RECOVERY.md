# Whole-process recovery of one local publication domain

## Connected consumer and scope

`delivery::persistent::FileDelivery` is a Unix, synchronous, operator-owned file
publisher. It connects the ORIGINAL `DeliveryBroker`, exact `PolicyAuthority`,
congress/session reducer, action/rights ledger, and `PublicationEndpoint` to a
persistent canonical history. It serves the FA-006/FA-012/FA-014 recovery and
one-effect work in plan 8.3, 8.4 and 16.9. This is a bounded reference profile,
not admission of a new production storage foundation or a replacement for the
required Asupersync/FrankenSQLite integration.

The caller provisions an explicit `FileDeliveryProfile`, then calls `create` on
a NEW protected directory. The profile includes the original scope/rights budget,
exact policy, congress roster and thresholds, initial actor data, target, retention,
clock-domain identity and storage limits. This is trusted bootstrap data, never
configuration accepted from an actor proposal. The profile remains immutable.

The only external effect is the payload returned by `read_publication` from the
canonical `delivery.bin` history. The raw journal also contains private snapshots,
ballot salts and original requests; it MUST NOT be served directly as the payload.
No public API exposes the underlying broker, endpoint, original permit, sendable
envelope or mutable authority. The existing standalone brokers and filesystem
endpoints retain their behavior and are not automatically migrated into this host.

This profile supports exact-policy proposals, complete reference congress reduction,
reservation, dispatch, publication, receipt reconciliation, sealing, cancellation
and fencing. `ReferenceReview` contains explicitly supplied trusted observations;
it reconstructs the original reference commitments/reveals, not authenticated
network helper execution. Missing members and abstentions still prevent Continue.
It is NOT the complete OversightBroker profile: full actual-helper-input capture,
human two-key roles, monitored numerical hosting, streaming, dynamic policy changes,
fleet membership and actor resets are not imported or silently downgraded.

## One canonical publication and its accounting history

Every operation first admits bounded input bytes and computes its transition using
the original reducers in a PRIVATE RAM-only projection. No replay opens a separate
filesystem endpoint or sends a network request. The locked owner writes the full
candidate history to a new `delivery.pending`, synchronizes it, renames it over
`delivery.bin`, and synchronizes the directory before acknowledging the operation.
The new directory and owner lock are also synchronized during creation.

The history records inputs to the original methods, NOT caller-supplied balances,
permission flags or terminal outcome enums. Reopening has to reproduce legal
transitions. Scope, exact action equality, witnesses, current policy epoch, deadline,
congress decisions, one-use permits, version conflicts and receipt-only refunds are
still checked by their original implementations. There is no second rights reducer.

Dispatch is committed separately from publication. After dispatch, the original
units are charged but the payload has not changed. Publication records the endpoint's
payload/version and terminal outcome in the same canonical replacement. Receipt
acceptance is a later transaction. Consequently, a published effect can coexist
with an unresolved charge when its acknowledgment is lost. An identical repeated
publication uses the original endpoint's retained receipt and cannot execute twice.

A write/sync/rename failure returns no new permit, endpoint outcome or refund and
makes the owner unavailable. `storage_failure` reports the actual I/O stage/kind;
rename attempts are conservatively marked as possibly visible. `inspect` is the
last acknowledged in-memory cut and can lag the canonical file after failed I/O.
It is not a post-failure permission source. A read-only consumer may already observe
the new canonical payload before the failed owner reports anything to its caller.
No error deletes that history or treats staging bytes as published results.

## Recovery after all process-local owners disappear

`open` requires the INDEPENDENTLY supplied complete bootstrap profile. Its canonical
encoding must equal the journal's binding, including original total rights and
policies. The canonical directory identity is also bound, so copying the journal to
a differently named directory cannot accidentally duplicate its recovery domain.
The owner acquires the existing kernel-managed `File::try_lock` lock; another
cooperating owner cannot open concurrently. The lock is released by descriptor
closure, including process exit, not by deleting a stale Boolean lockfile.

Replaying the complete canonical history reconstructs both original reducers with
matching new process-local brands, but exposes none of their reconstructed permits.
Before returning a writable owner, recovery PERSISTS an original-authority epoch
revocation, cancels only undispatched attempts, advances and installs the original
dispatcher fence, and forgets all sendable old envelopes. Original dispatched
attempts become Unknown; confirmed and confirmed-nonexecuted outcomes remain intact.
Suspension and narrowed authority are not reset by reconstructing the process.

Old facade permits cannot bind the new owner, even in the same process. Cancelled
IDs cannot be reused. No prior approved request is resent or given a new permit.
Already-published values remain visible, and the original endpoint's retained
receipts resolve them without reexecution. A missing status stays charged; explicit
`seal_unexecuted` obtains the original atomic nonexecution receipt before the
original ledger refunds it. Sealing a key that actually executed returns its
execution outcome instead. Cancellation is not a post-dispatch refund path.

A saved elapsed tick is historical. Recovery sets `clock_ready` false and requires
an explicit fresh observation in the profile's SAME process-independent clock
domain before permission or endpoint-status operations. A supplied tick below the retained
durable clock floor refuses. The clock's correctness, continuity across host restart,
and truthful current observations remain operator assumptions; this library does
not sample a host clock, persist a failed unsynchronized observation, establish a
tick duration or bound filesystem latency. Cancellation/fencing remains independent.

Verified exclusive recovery can discard an inert regular staging file, after the
canonical file is validated and synchronized. Missing, truncated, malformed or
semantically illegal canonical history refuses. No fallback to a staging file,
empty ledger, fresh total budget or old decision is provided.

## Representation, bounded work and remaining trust

Version one starts with `FADLGR` plus zero and version byte 1. Integers and lengths
are big-endian. It binds canonical directory bytes, the complete supplied bootstrap,
an event count and length-delimited typed events. The bootstrap is compared with
independent operator configuration, not decoded into newly trusted authority.
Snapshot and ballot maps have strict sorted keys; unknown tags, duplicate keys,
invalid counts, suffix bytes and truncated frames refuse. Re-encoding additionally
checks canonical framing. All events must then execute through the original reducers.

Limits are at most 4,096 events and 16 MiB of canonical bytes. Each snapshot has
at most 256 entries/256 KiB of values; each review has at most the original 256
members and 256 KiB of aggregate member/salt bytes. Underlying original action,
policy, congress, actor-state, delivery and rights bounds remain in force. Limits
are bound to the store and cannot be enlarged by reopening. Even idempotent calls
and recovery fences consume history slots in this first profile. Exhaustion may
make writable recovery unavailable; read-only inspection preserves the actual
liabilities rather than manufacturing a new allowance.

This baseline replays and rewrites the bounded history for EVERY mutation. Its
cumulative work/write amplification is quadratic in operation count. Snapshot,
policy, receipt and speculative projections coexist in memory. Encoded bytes are
not peak RSS, allocation traffic, actual syscall cost or latency. It is not an
optimized transactional database or cancellation-correct asynchronous adapter.

New directories request 0700 and files 0600. Direct root symlinks/nonprivate roots
and nonregular selected files refuse. The root, all ancestors and filesystem must
remain operator-controlled; path metadata checks do not defeat hostile rename,
mount races or a writer that ignores the cooperative lock. Storage is plaintext.
The format has no cryptographic authentication or independent anti-rollback anchor.
A malicious same-path snapshot rollback, coordinated valid edit, cross-host cloned
namespace or false bootstrap authority is OUTSIDE this crash-recovery profile.
Filesystem/device durability after successful synchronization is an environmental
assumption, not a universal hardware/power-loss theorem.

The original `FileEndpointRecovery` continues to require its surviving controller.
This new coupled host instead recovers its own narrow domain from the full history
and independent profile. It does not recover arbitrary existing OversightBroker
state, serving credentials, model memory or an external-world effect.

## Source tests and verification

The first increment adds twelve public scenario tests and two compile-fail examples.
They exercise original-policy/congress publication, exact denied and missing-helper
controls, mutated input/action refusal, one-use handles, owner loss with published
or unsent dispatch, sealed refunds, old-permit invalidation, suspension, deadlines,
kernel-lock contention, bootstrap/location substitution, real create-new failure,
partial/staging history, limits and private creation modes. Existing assertions and
reducers are unchanged. Reference snapshots and ballots are test inputs, not real
provider authentication or independent helper inference.

The required RCH test command is:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_delivery
```

Rust compilation/tests, rustfmt, Clippy and the full revision-bound gate remain
pending. The editing environment has no rch or Rust toolchain; there is no local
compiler fallback. No Beads task or production gate is closed. Successful source
construction is not execution evidence or a complete G1/production qualification.
