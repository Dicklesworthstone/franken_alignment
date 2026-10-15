# Single-replay learned receipt reconciliation

This is scoped FA-014 implementation (plan sections 8.4 and 16.4), not packet
closure. The consumer is the learned publication operator's explicit receipt
settlement path. Ordinary recovery previously issued a live owner and then
replayed numerical history again for time and reconciliation transactions.

`finish_reconciled_receipt(request, finish_after, clock)` now consumes the
original complete learned recovery into one acknowledged `Fence -> Time -> Sweep`
replacement. `None` selects a message; `Some(previous)` selects a finish with its
exact confirmed predecessor. The base learned cursor and ordinary, raw-residual,
and owned-K/V guarded cursors expose this operation. Each guarded wrapper checks
its original independent configuration, policy, inventory and floors before
forwarding. Optional exact history anchors keep their existing prefix contract.

The native guarded reconciliation engine and learned finalizer share one private
transaction implementation. No caller-supplied candidate, event list or closure
is accepted through a public interface. The same original source-admission,
consistency preflight, reducer, prefix/byte checks, canonical rechecks, staging
cleanup and single replacement remain in effect. Existing native guarded and
Andon recovery retain their role-provisioning behavior after acknowledgment.

Learned receipt completion returns only `Knowledge<ActorOutcome>` through the
existing request-book projection. It issues no observer, reviewer, effect permit
or live owner. The sweep queries existing endpoint records; it cannot recreate a
sendable envelope or repeat publication. Published but unresolved messages may
become confirmed only from an original receipt. Missing receipts remain Unknown
and charged. Denied requests need no FrozenAction and remain denied. The same
fence pauses numerical generation and preserves its work, sampling history and
lifetime forecast evidence rather than replenishing them.

Selection precedes the clock and every mutation. Incomplete replay, a bad key,
a wrong release/predecessor, unmet guards/floors or a changed canonical image
returns no observation. A trusted clock runs once after complete replay and the
first canonical check. A second canonical check precedes cleanup/replacement.
An unwinding clock drops the private cursor without writing. A persistence error
may leave the complete replacement visible but returns no candidate outcome;
subsequent recovery queries receipts, never resends an effect.

This operation is explicitly mutating: it spends three original events and
requires ordinary capacity for Time/Sweep. It does not borrow the terminal-only
stop reserve or offer a zero-event retry. At ordinary exhaustion use stopped
recovery for settlement, or `finish_receipt` for non-mutating observation. The
existing read-only path is unchanged. It still returns Unknown for publication
whose receipt has not been reconciled. No source, native helper or new numerical
step is run during finalization, but verifying history still performs original
numerical replay. A synchronous event, clock or filesystem operation can exceed
a caller's time budget; no hard latency or adversarial-filesystem claim is made.

Four new regression functions exercise all three independently guarded learned
modes. The positive oracle uses original separate Fence, Time and Sweep calls on
a test journal, then compares the new transaction with the exact resulting
canonical bytes and original actor projection. Other cases cover denied requests,
unresolved charges, partial replay, wrong selection/floors, canonical changes
before and during the clock, clock unwind, full ordinary/terminal capacity and
successful subsequent read-only inspection. Native review/human approval in the
fixtures establishes paths, not statistical calibration. Existing shared-engine
storage-barrier regressions remain unchanged.

These tests are authored and source-reviewed but UNEXECUTED. The prescribed
remote-only RCH launcher is absent in this session, as are cargo, rustc and
rustfmt. Preimage checks and whitespace checks do not establish compilation,
runtime correctness, performance or production qualification. No bead or gate
is closed and no repository verification requirement is weakened.
