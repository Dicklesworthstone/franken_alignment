# Exact history anchors for the rights journal

## Implemented source boundary

`fa_journal::replay::HistoryAnchor` imports a complete independently retained
canonical v1 or explicitly selected v2 rights-journal prefix. Import uses the
original strict decoder and rights reducer. The caller supplies identity, limits
and, for v2, the original recovery reserve. No profile is inferred from disk.
The anchor exposes immutable evidence bytes and a checked head, never rights.

`Journal::begin_replay_anchored` and
`Journal::begin_replay_anchored_with_recovery_reserve` use the original cooperative
replay cursor. Every byte covered by the anchor must match at the same offset.
The comparison runs in the existing budgeted read path, before those bytes can
be decoded or applied. A mismatch permanently fails the cursor; returned bytes
still count as work. Exact copies and valid successors undergo all original
framing, checksum, identity, accounting, reserve, clean-EOF and minimum checks.
Only completion exports accounting. Recovery never dispatches an external effect.

The ordinary `Head` API and both wire versions remain unchanged. A deliberately
constructed pair of valid histories with identical CRC-32 and sequence illustrates
why an exact prefix is stronger evidence than the legacy accidental-corruption
check, without relabeling CRC-32 as authentication.

## Locked-file recovery and capture

On Unix, `FileJournal::begin_open_anchored` and
`FileJournal::begin_open_anchored_with_recovery_reserve` select exact evidence
before opening the path. Wrong identity, limits or reserve refuse before file
access. Both use the original `FileRecovery`, bounded reader and single locked
descriptor, not a preflight reader followed by a second unlocked reopen. The lock
is retained while reading, after Ready, and through the original final extent
check and file/directory synchronization. Only successful `finish` returns a
writer. Cancellation or any failure returns no partial accounting and never
repairs, truncates, creates or replaces the journal. Synchronous convenience
methods are `open_anchored` and `open_anchored_with_recovery_reserve`.

A healthy durable owner can call `FileJournal::history_anchor` to capture its
acknowledged history on that same locked descriptor. Capture reads exactly the
acknowledged extent, checks the extent before and after reading, and invokes the
original strict decoder and reducer from the original initial configuration.
The result must reproduce the owner's head, encoded length AND complete rights,
not merely its CRC. Capture restores the descriptor to the end before success.
It neither writes journal bytes nor releases or clones the lock. Subsequent
commits continue on the same owner with its unchanged accounting and capacity.

A poisoned owner cannot capture an unacknowledged tail. Once capture starts I/O,
an I/O error, inconsistent content or unwind leaves the owner poisoned and
exports no anchor. Allocation refusal before I/O leaves the owner unchanged.
Drop a poisoned owner and recover against previously retained independent
evidence: any complete valid successor is replayed, not ignored or redispatched.

Capture only returns in-memory evidence. The caller must durably retain the
bytes and independently pin identity, limits and reserve before using them for
a later recovery. Reimport with `HistoryAnchor::from_bytes` for v1 or the explicit
`from_bytes_with_recovery_reserve` constructor for v2. Importing an archive read
from the journal being recovered is not independent rollback protection.

## Cost and trust

Anchor import and file capture are synchronous bounded replay, separate from
recovery poll budgets. Capture temporarily holds the full prefix and another
reconstructed ledger; conversion to shared storage may temporarily copy bytes.
The retained immutable prefix occupies its encoded length; clones share storage.
Recovery additionally retains its ordinary bounded frame and reconstructed rights
and, for files, the original 8 KiB read-ahead buffer. Lifetime limits must match
even for v1, whose header does not encode them. These are logical work bounds,
not elapsed-time or interruptible-I/O guarantees.

The anchor must be obtained and retained independently. Valid structure does not
prove who supplied it, its freshness, or acknowledgment by a previous owner.
Joint rollback of the journal and anchor is not detected. These APIs do not
provide signatures, hostile-directory protection, remote-receipt authentication,
or exactly-once execution. Capture retains the original trusted-directory,
advisory-lock and filesystem assumptions; it is not proof against a hostile
writer changing bytes behind the lock. Unknown/dispatched liabilities remain
charged. Existing head-only callers are unchanged; exact protection requires
selecting the anchored recovery APIs.

## Verification status

`crates/fa-journal/tests/history_anchors.rs` supplies positive successor/parity
cases and causal negatives for a real CRC collision, rollback, truncation,
invalid suffixes, foreign context, limits, explicit reserve selection, zero and
interrupted budgets, and cancellation.

`crates/fa-journal/tests/file_history_anchors.rs` adds nine regressions for
capture/reimport and continued writes, valid successors, lock retention through
Ready and finish, cancellation, same-CRC divergent accounting, poisoned capture,
unacknowledged suffixes, pre-open context refusal, invalid files, changed extent
after Ready, and explicit reserve capacity with unknown liabilities still charged.

These are authored regressions, not executed evidence: RCH and the Rust toolchain
are unavailable in the editing environment. Run the targeted journal tests and
the repository's required remote qualification before claiming acceptance:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Full FA-014 acceptance and runtime qualification remain open. No roadmap or Beads
completion status is changed by these source additions.
