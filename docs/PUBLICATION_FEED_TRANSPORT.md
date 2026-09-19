# Concrete publication change-feed transport (FA-061 / FA-062)

`FileOversight::refresh_publication_feed` replaces manual delivery of individual
notices with a bounded concrete file acquisition. It uses the original reverse
index, change sequence, producer heartbeat, lease, journal and publication gate.
It creates no authority, event tag, background task or dependency.

## Producer and host contract

The types live in `observed::publication::capture::heartbeat::feed`.
`PublicationFeedBatch` contains an original `PublicationHeartbeat`, an exclusive
`after` position and EVERY record in `(after, heartbeat.through]`, in order.
At most 256 records and 32 KiB are accepted. Empty windows are valid only when
`after == through`. Source substitution, duplicate or gapped sequences, wrong
versions, truncation and trailing bytes refuse. The versioned file is `FAPFEED1`.
The original notice codec preserves all key, range, domain and All variants;
a syntactically decoded malformed range retains its existing conservative
all-observation withdrawal semantics, not a selective invalidation assertion.

Each sequence identifies immutable notification contents. A newer heartbeat may
extend the stream, and a retained window may roll or widen without changing its
records. Heartbeat generations/times obey the original monotonicity rules;
rereading never renews the producer-anchored expiry. A producer must retain enough
records for the consumer's declared bootstrap/acknowledged position. Records
before the independently pinned bootstrap have no implied retained history.

`PublicationFeedFile::new(path, source)` selects a concrete operator-owned file,
not a caller-overridable reader or positive cache. Producers write an immutable
replacement then rename it atomically. Acquisition checks regular-file type,
path/handle metadata and bounded size before/after reading. Paths are bounded to
4,096 bytes and the read allocates at most 32 KiB plus one sentinel byte. These
checks are not an adversarial-filesystem sandbox or a real-time I/O deadline.

Create the original validation/change/freshness profile, then call
`refresh_publication_feed(journal_revision, &reader, clock)` before the original
effect boundaries. The method durably withdraws feed eligibility BEFORE I/O.
After decoding it samples the trusted elapsed clock. It checks every overlapping
record against original journal contents, including earlier out-of-order notices.
An identical covered sequence is skipped; a changed one quarantines the live owner.

The unseen contiguous suffix and its heartbeat are applied on a private replay
of the original machine and become acknowledged through ONE canonical replacement.
The encoder checks every history prefix against event/byte and recovery-reserve
limits before candidate execution. No partially installed suffix is returned.
The prior withdrawal remains a separate acknowledged non-permitting transition.
A normal call adds `new_notices + 2` journal events; rereads add no notice duplicates.

## Incomplete coverage and failure

When `after` lies beyond the acknowledged complete prefix, no later record is
misrepresented as catch-up. The original heartbeat records its observed head and
retains missing-tail refusal. Supply a window covering the real missing prefix;
original repair semantics withdraw all observations through the final repair.
A newly permitting heartbeat does not re-create withdrawn witness inputs.

An inner `FileCaptureError` means acquisition failed after durable withdrawal.
An outer `JournalError` after a successful read means installation was not
acknowledged; clock unwind, conflicting history, allocation, capacity, replay
and storage failures quarantine the owner. A post-rename error can expose the
whole new cut, never a claimed partial success; use original fenced recovery.
The returned report distinguishes applied notices, complete/observed frontiers
and current heartbeat eligibility. It is not accepted as a publication permit.

All original exact structured/opaque, source, human, credential, deadline and
receipt checks remain. Ingestion itself cannot execute, approve, refund, clear
an independent policy-source interruption or restore old keys. Receipt-only
reconciliation remains independent of transport availability. Reopen replays
the canonical prefix without reading this file and invalidates the old heartbeat
acquisition via the existing dispatcher epoch.

The packet unifies records and heartbeat in one file image; it does NOT make
witness/policy reads atomic with that image, authenticate the producer, verify
real-world completeness, or implement a remote network feed. Manual trusted host
APIs remain; this is an additional concrete transport, not OS enforcement of the
host's integration choices. Overlap verification scans bounded retained history;
replay/encoding/allocation are extra work, not the reverse-index comparison budget.

## Verification

`tests/file_publication_feed.rs` adds ten regressions for canonical packets,
size/sequence neighbors, selective successful publication, post-dispatch changes,
rolling overlap/equivocation, unavailable windows and repair, stalled leases,
failed final writes, caught clock unwind and full-batch capacity.
These Rust tests are UNEXECUTED. The required RCH xtask check was attempted and
exited 127 (`rch: command not found`); cargo/rustc are also unavailable here.
Compilation, formatting, Clippy and exact-revision execution remain outstanding.
FA-061/062 remain open; no production activation or authentication claim.
