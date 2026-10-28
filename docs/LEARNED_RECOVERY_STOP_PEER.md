# Authenticated Stop during learned recovery

## Native contract · 2026-10-28

A fully replayed learned history can now become a sealed `FileRecoveryStop`
target for the existing authenticated Stop protocol. The target retains the
original exclusive Store lock, canonical bytes, native reducer, independently
selected profile and fixed Stop request. It exposes only the immutable request
and reviewer expectation; no live owner, reviewer role, source writer, weaker
recovery cursor or generic mutation callback escapes.

`into_recovery_stop(operation)` is available on the original learned cursor and
all five guarded wrappers: plain, required raw-residual, required owned-K/V,
mediated plain, and mediated predictive (raw or owned). Every wrapper rechecks
its complete original recipe, graph, effective policy, credentials, predictor
requirements and independent recovery floors before transferring custody. A
retained recovery reserve is mandatory; this path does not install or refill it.
Partial replay or a failed requirement cannot produce a transport target.

`VerifiedReviewerSocket::into_recovery_stop_connection` transfers the same
kernel-authenticated Linux socket into `StopRecoveryConnection`. The generic
stream constructor retains the existing embedding-supervisor obligation to
supply authenticated bounded/nonblocking I/O. The original `StopClient` and
fixed-size nonce, audience, scope, clock-domain and operation binding are
unchanged. Merely admitting a peer, sending a challenge or receiving a partial
request does not authorize any write. Only the complete exact bound request
consumes the target through the original `finish_admission_stop` operation.

## Durable result and failure semantics

A new admission Stop appends the original Stop/Fence pair in one native
replacement. It does not perform ordinary live recovery, issue a live role,
acquire or journal time, settle an effect, sweep a liability, rerun numerical
work or refund source work. Canonical custody is rechecked by the original
terminal consumer before staged cleanup or replacement. Unknown outcomes stay
Unknown, their charges remain, and pending numerical state remains the original
state. The existing progress reducer determines whether the result is already
drained or still pending; admission acknowledgment alone does not imply drain.

The native result is retained in `StopRecoveryApplication` before receipt
encoding or delivery. A lost reply cannot erase an acknowledged history anchor,
snapshot or progress. Storage failure produces the existing Unconfirmed wire
status and preserves the native storage error, including possible replacement
visibility. The connection applies at most once and latches failures; it cannot
retry a mutation after partial I/O or a caught unwind. Dropping an unapplied
connection releases its original cursor without cleanup or a write.

An explicit exact-request reopening can confirm the original Stop without a
new event, clock sample, replacement or duplicate control action. A different
operation cannot take over that historical request. The existing terminal
checkpoint recovery remains the way to repair independently retained custody;
the transport does not infer missing receipts or unknown endpoint execution.

No journal event, wire tag, agent verb, dependency, runtime or founding-plan
semantics changes. This is reference implementation work under FA-014 and
FA-120, plan sections 8.5, 8.11, 16.4 and 16.9, serving FI-A02 and the existing
FA-INV-005/006 recovery obligations. Full packet prerequisites and qualification
remain open.

## Source and verification

The transport is implemented in
`crates/fa-reference/src/action/consequence/delivery/persistent/observed/reviewer/peer/control/recovery.rs`.
The original learned terminal handoff and five guarded wrappers provide the
sealed factory; the existing Stop wire projects the native admission result.
Nine authored regression functions under `control/recovery/tests/` cover:

- Exact native Stop/Fence bytes with charged Unknown outcomes, pending numerics,
  staging retention before the request, and the original exclusive lock.
- Every partial replay and independent guard/floor/policy/credential refusal,
  required reserve, and all six plain/predictive/mediated configurations.
- Silent, malformed, wrong-binding and truncated requests, fragmented I/O,
  lost reply, exact historical retry and different-operation refusal.
- All five original storage barriers and canonical substitution after target
  selection, with Unconfirmed results and preserved conservative liabilities.

Three compile-fail examples express the forbidden owner/role/cursor escapes.
These are authored tests, not execution evidence. The 13 changed native Rust
files pass tree-sitter Rust syntax parsing and independent source review.
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib`
stops before compilation because `rch` is absent (exit 127). There is no local
compiler fallback. Type checking, test execution, qualified formatting, Clippy
and the full workspace gate remain pending; this code does not close the scoped
bead or any production acceptance gate.
