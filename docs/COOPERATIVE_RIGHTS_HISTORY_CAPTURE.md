# Cooperative exact-history capture

## Contract and implementation status (2026-10-15)

The rights journal can now retain its original input while the existing replay
cursor verifies it. `Journal::begin_replay_with_history_anchor` explicitly selects
v1 capture; `begin_replay_with_history_anchor_and_recovery_reserve` selects v2
with independently supplied reserve and lifetime limits. Call `advance` with the
original read-call, byte and record-check budgets, then consume
`finish_with_history_anchor` to obtain the verified journal and exact archive.
An ordinary replay cannot upgrade to capture after observing its result.

This uses the original decoder, reducer, minimum-head coverage, EOF check and
failure/unwind latch. It neither serializes a reconstructed ledger nor reads the
source a second time. Cancellation discards the private reconstruction and
archive; it cannot refund unknown effects or issue partial evidence. Exact bytes
are evidence, not a signature, a freshness guarantee or execution authority.
Independently retain the archive before using it against rollback or forks.

Captured bytes add up to the admitted history length, with vector allocation
slack, to the existing frame and decoded ledger storage. Allocation can move
previously captured bytes; immutable archive conversion at completion can copy
the full prefix. Poll budgets bound logical reads/checks, not CPU instructions,
filesystem latency, allocation latency or elapsed time. Capture is opt-in; the
ordinary replay does not retain the additional archive.

The unchanged synchronous `HistoryAnchor::from_bytes` import is an independent
oracle for the new tests. Six regression functions cover v1/v2 parity, exact
bytes, unknown charges, reserve capacity, every truncated prefix before the
required minimum, corrupt tails, explicit selection, zero work, cancellation,
lifetime limits, wrong profiles, interrupted reads and unwinding.

This advances the recovery/evidence path of FA-014 and plan sections 8, 16 and 18;
it does not change their semantics or close that packet. The targeted remote-only
RCH test command was attempted and exited 127 before compilation because `rch`
is unavailable in the editing environment. Tests are authored, not executed;
Rust compilation, formatting, Clippy and complete qualification remain pending.

## Locked file integration (2026-10-15)

`FileJournal::begin_history_anchor` now consumes a healthy acknowledged writer
into the original `FileRecovery` cursor. It retains the same descriptor and
exclusive lock, seeks to the original header and checks with the original
identity, limits and reserve. Every `advance` shares the original replay budgets.
`finish_with_history_anchor` requires matching head, extent and complete rights
before returning the original owner and exact archive. No new synchronization is
needed for that already acknowledged owner. The synchronous borrowing
`history_anchor` API is unchanged.

For recovery from a path, select `with_history_capture` before any read/check on
the existing cursor. This also works with independently anchored v1/v2 recovery:
the old prefix is compared while the complete successor is captured in the same
pass. Both the original file and parent-directory sync still precede return, and
extent is checked again after synchronization. Ordinary `finish` shares those
same checks; asking for an archive without selecting capture fails before sync.

Cancellation/incomplete finish closes the lock and returns no writer or archive.
A failed commit's poisoned owner cannot be captured. Only strict original
recovery can acknowledge a surviving complete suffix; dispatch liabilities remain
charged. None of these operations is a durable Stop, a refund, a repair or an
external resend. As before, same-length hostile changes after a read are outside
the trusted-directory/advisory-lock contract; metadata checks do not authenticate
file contents. Store the returned archive independently before relying on it.

Seven public file regressions cover v1/v2 original-owner parity, continued writes,
exclusive locking, cancellation/recovery, a real equal-CRC fork with different
rights, extent changes, anchored successors, explicit selection, partial finish
and poisoned capture. Four private regressions exercise the original persistence
hooks: errors/unwinds before and after both recovery barriers, healthy-owner
no-extra-sync behavior, extent changes during sync, and an actual complete record
whose original commit failed before acknowledgment. Together with the replay
increment there are 17 authored regression functions. Required targeted and full
RCH launches remain blocked before compilation by the missing launcher; no
executed Rust result, packet closure or runtime qualification is claimed.
