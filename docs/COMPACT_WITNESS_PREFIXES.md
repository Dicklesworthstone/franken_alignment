# Compact source prefixes in publication witnesses

This extends the bounded frontier and final-publication contracts in plan 7.7–7.8
without introducing another gate, driver, authority ledger or dependency. Long
closed source streams no longer require replaying their sequence numbers one by
one or rejecting an otherwise bounded witness solely after position 4,096.

## Native contiguous observation batches

`ProductFrontiers::accept_contiguous(key, stage, first, last)` records EVERY
position in the inclusive interval under the existing caller-supplied observation
contract. It is not a claim that only its endpoints were observed. The interval
must touch the known contiguous prefix, or be wholly inside it. Missing
predecessors refuse without mutation. The operation never creates a closing
marker, advances another stage/projection, or extends a terminally closed stream.

For admitted intervals its result equals ascending individual calls to the
original accept operation, including already observed pending successors. Work
is bounded by existing pending entries, not the numerical interval length. The
ordinary out-of-order max-gap limit and stream-capacity limit are unchanged.
This does not authenticate batch contents or certify real-world observation.

Publication witness replay consumes this primitive directly. Its reconstruction
starts with no pending entries and records just one closed authenticated
projection, so prefix reconstruction is constant work independent of its terminal
number, including u64::MAX. Other stages and projections are NOT reconstructed as
complete. The separately admitted closing marker remains essential: a Closed
assertion in a snapshot with no admitted marker still cannot prove absence.

## Wire compatibility and existing consumers

The existing FileWitnessInput constructor now accepts independently admitted
closures through the full u64 sequence domain. All entry/value/request and packet
byte limits remain unchanged. Existing materialization, exact judgment capture,
source-bound packets, durable FileOversight replay and final publication consume
the same representation. No new caller-supplied validation-success bit is accepted.

Canonical encoders retain byte-identical FAPWIN01/FAPWEV01 for previously admitted
inputs. A closing prefix above the legacy MAX_REPLAY_PREFIX (4,096) selects
FAPWIN02/FAPWEV02. Payload field encodings and all opaque input metadata are
unchanged; the packet does not grow with the numerical prefix. The old limit now
applies specifically to version-one wire admission, not native reconstruction.
This supersedes the earlier general 4,096 replay-work limitation described in
PUBLICATION_VALIDATION.md; its other snapshot/value and authority limits remain.

Version-one decoding still rejects an admitted prefix above 4,096. Version-two
headers on small inputs are noncanonical and refuse, rather than creating
multiple encodings for the same observation. Older readers refuse the new header;
they never truncate its prefix or silently weaken closure. Unknown headers,
truncation and trailing bytes remain errors. Exact action, feed-cut, source,
semantic, policy, human, deadline and receipt requirements are unchanged.

Large sequence numbers are not unlimited snapshots: native entry/value bounds,
fixed packet ceilings and enclosing journal event/byte/recovery reserves still
apply. Prefix reconstruction is distinct from exact witness comparison; the
latter retains its own work budget and still refuses on exhaustion. No latency,
throughput, RSS or full-journal constant-time claim is made.

## Verification status

Six native regression tests compare all 128 small pending patterns and their
interval neighbors against original single-position ingestion, and cover huge
prefixes, u64::MAX, holes, closure, capacity, independent stages and gap windows.
Seven integration tests in tests/compact_witness_prefixes.rs cover legacy/compact
header boundaries, bounded round trips, malformed inputs, missing admitted
closure, actual two-key publication, late value/absence/range/opaque invalidation,
receipt-only refunds, exact comparison exhaustion, and canonical restart/replay.
Existing packet and source tests are preserved without assertion changes.

An independent Python differential model executed 4,608 interval cases (1,793
admitted, 2,815 missing-predecessor refusals) and matched individual ingestion.
This is not execution of the Rust implementation. Authored Rust passed lexical
and delimiter/whitespace/declaration screens; uploaded source hashes match local
files. The required RCH xtask attempt exited 127 because rch is unavailable, and
cargo/rustc are absent. Compilation, formatting, Clippy and all 13 new Rust tests
remain UNEXECUTED. Exact-revision remote verification remains required; no Bead
closure, production activation or authenticated-source qualification is claimed.
