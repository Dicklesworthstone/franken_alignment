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
