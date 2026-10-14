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

## Cost and trust

Anchor import is synchronous bounded replay, separate from recovery poll budgets.
The immutable prefix occupies its encoded length in memory; clones share storage.
Recovery additionally retains its ordinary bounded frame and reconstructed rights.
Lifetime limits must match even for v1, whose header does not encode them.
These are logical work bounds, not elapsed-time or interruptible-I/O guarantees.

The anchor must be obtained and retained independently. Valid structure does not
prove who supplied it, its freshness, or acknowledgment by a previous owner.
Joint rollback of the journal and anchor is not detected. These APIs do not
provide signatures, hostile-directory protection, remote-receipt authentication,
or exactly-once execution. Unknown/dispatched liabilities remain charged.

## Verification status

`crates/fa-journal/tests/history_anchors.rs` supplies positive successor/parity
cases and causal negatives for a real CRC collision, rollback, truncation,
invalid suffixes, foreign context, limits, explicit reserve selection, zero and
interrupted budgets, and cancellation. These are authored regressions, not
executed evidence: RCH and the Rust toolchain are unavailable in the editing
environment. Full FA-014 acceptance and runtime qualification remain open.
