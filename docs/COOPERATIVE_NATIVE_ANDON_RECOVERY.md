# Single-pass cooperative native Andon recovery

Source implementation, 2026-10-13. Continues the original
[guarded native recovery](GUARDED_NATIVE_ANDON_RECOVERY.md), plan sections 9.9,
16 and 18, FI-A08/FI-A13/FI-A18. Native assessment/accounting persistence remains
one canonical journal. No packet or execution qualification is closed.

## One original engine and one original fence

`FileOversight::begin_open_andon_guarded` selects an exact independently retained
history anchor, full Andon window inventory, evaluator protocol and original
recovery requirements. Its opaque `FileAndonRecovery` retains the original
`FileGuardedRecovery` and exclusive Store lock. It does not run a separate event
reducer or expose the inner cursor. Original framing, limits, anchor and decoder
configuration admission remain in force before any historical event executes.
The additional native protocol and window preflight also occurs before replay.

`advance(expected_events, max_events)` executes the original Machine under the
same bounded quantum, predecessor validation and failure/unwind latch as base
recovery. The complete native Andon, evaluator and original guard/floor checks
run before Ready. Progress distinguishes attempted calls from verified events;
a failed or interrupted event cannot become a successful replay prefix. Invalid
quanta and stale predecessors spend no further event work. Ready is not an
acknowledgment and supplies neither an owner nor evaluator/human role.

`finish` rechecks the complete independent requirements and uses the original
single-fence completion on the already verified machine. It does NOT replay the
history again. Original capacity/encoding preflight and exact retained-canonical
comparison precede staging cleanup. Only acknowledged original persistence can
release the new owner and separately held roles. Native labels, pending/censored
cases, receipts and incident order survive; old role brands, approvals and live
clock eligibility do not. The original synchronous `open_andon_guarded` now drives
this SAME cursor to completion instead of replaying again through `transact`.

`require_recovery_reserve` delegates to the original pre-replay reserve selector.
A mismatch, absence or late selection consumes/refuses the cursor without writes;
it never creates or replenishes capacity. Cancelling or dropping the cursor at
any boundary, including Ready, releases only private replay state and the lock.
Canonical bytes and unacknowledged staging remain unchanged. A consumed failed
finish returns no role bundle, including at each original Store barrier.

Historical `read_andon_guarded` remains non-mutating and shares the exact native
preflight/final checks. Base guarded recovery retains its original checks through
closed internal composition. No public callback can substitute an accepting
validator, and existing stopped/publication/reconciliation APIs are unchanged.

## Limits and verification

The quantum bounds original event calls, not wall time or interruptible I/O.
Initial file read/decoding, requirement checks and final encoding/synchronization
remain synchronous. One event can be expensive. This source does not wire a new
operator deadline loop or claim measured latency, cryptographic evaluator
identity, institutional independence, or production containment.

Eight new regression functions use the actual native broker, evaluator and Store.
They cover one-event/full-history parity and post-restart label resolution;
cancellation at every boundary with staging preservation; stale/invalid polls and
incomplete finish; invalid suffix/final-floor failure latching; caught original
application interruption; changed canonical bytes after Ready; exact zero/one
fence slots and all five Store barriers; and absent/wrong/late/matching reserve
selection. One compile-fail example rejects inner-cursor access. The fourteen
journal/bootstrap/recovery regression functions from the preceding increments
and their existing assertions remain unchanged.

All source preimages were compared by Git blob identity; independent patch
reconstruction, postimage comparisons and Git whitespace checks passed. Targeted
native-cursor tests, crate doctests and unchanged xtask were attempted through
`RCH_REQUIRE_REMOTE=1 rch exec`. The launcher is absent (exit 127 before Cargo).
Compilation, Rust test execution, rustfmt and Clippy remain unexecuted. Source
inspection and patch checks are not execution evidence.
