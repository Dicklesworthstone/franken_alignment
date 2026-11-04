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

## Operator recovery integration · 2026-10-28

Live learned continuation, next-message and Finish startup now retain an admitted
Stop peer while the same original guarded cursor advances one event at a time.
The final Ready checkpoint selects either the original live finalizer or the
sealed terminal target. A valid complete Stop request reaches the original
admission consumer before any live owner or observer is issued. The plain,
required raw-residual and required owned-K/V paths work with or without mediation.
No-peer recovery still executes its original single recovery Fence.

Use the existing command from the independently authorized process:

```text
supervise_publication stop-peer REVIEWER_JSON REQUEST_ID
```

The original `stop-fleet` client can use the same endpoint and wire. Select the
original operation of the running invocation; a resident learned sequence keeps
its first recipe's operation. No new Stop command, profile schema or protocol
is required. The client may wait for complete guarded replay before receiving
its challenge. It receives the native acknowledgment and pending/drained state,
not proof that replay was preempted or an unknown remote effect was undone.

The original workflow logical and elapsed limits remain in force. The peer
session's finite elapsed budget begins when kernel admission succeeds and is
not restarted at Ready. The same candidate quota survives every checkpoint.
Malformed, truncated, silent or expired exchanges terminate startup before live
custody; they cannot fall back into ordinary recovery. Admission errors,
including exhausted candidate quota, are latched so reusing the same control
object cannot bypass the refusal after the rejected queue empties.

After native application, the immutable result is cached before any fallible
post-acknowledgment budget callback or receipt write. Only the existing bounded
cleanup allowance may drain the historical reply. A late deadline or broken
socket preserves the native result. The startup function deliberately returns
an explicit terminal message beginning `native admission Stop acknowledged at
revision ...` rather than fabricating an actor result or yielding live custody.
A native storage refusal instead says that completion is unconfirmed.

### Independently retained custody

An existing `--retain-history-anchor NEW_OUTPUT` selection retains the exact
acknowledged terminal history. For mediated recovery, a version-two
`fa.mediated-submit/2` profile may select `retain_checkpoint` beside its original
`source_checkpoint`; the original matching history/source pair must already
have passed guarded admission. The destination accessor reads only that
selection. It never calls the live `next()` source loader.

History and topology outputs are attempted independently after acknowledgment,
even if the deadline elapsed or the network reply was lost. Source custody uses
the original `after_admission_stop` method, preserving exact source expenditure,
operation floors and history association. An output collision preserves the
successful partner archive and reports every failure with the native revision.
Keep prior custody and select fresh paths for the existing terminal-checkpoint
recovery command when repair is necessary; do not repeat an effect to recreate
an archive. Unconfirmed application does not export a fictitious terminal cut.

This path adds no helper launch, next-source read, reviewer work, actor request,
numerical continuation or effect-clock observation. Existing CLI recipe/model
admission and credential preparation still precede Stop socket creation. Initial
file reads, a single numerical replay event and final persistence remain
synchronous: this is cooperative stopping at event boundaries, not an OS-call
or numerical-kernel latency guarantee.

### Operator regressions and limits

Ten new regression functions in `tests/recovery_control/durable.rs` and its
`refusals.rs` child use real Unix sockets, the original StopClient, actual learned
histories and the original offline admission consumer as the byte/state oracle.
They exercise all six configurations and early/middle/Ready arrivals; published
but unreconciled Unknown effects; no-peer and rejected-identity controls; silent,
malformed and truncated frames; original quota/session exhaustion; source,
floor, anchor and canonical failures; lost replies, late deadlines and both
independent output races. A positive control calls the actual continuation
`run()` path with an already admitted recipe and missing live helper/source
inputs. Four pre-existing recovery-control regression functions remain, with
the two connection-only expectations updated to require an explicit request.

The nine changed operator Rust files pass syntax parsing and independent source
review. Together with the native commit this adds 19 regression functions and
three forbidden-owner API examples. The required example test dispatch and full
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` both stop
before compilation because `rch` is absent (exit 127). None of these new runtime
tests or compile-fail examples has executed. Type checking, qualified formatting,
Clippy and full qualification remain open, and bead
`franken_alignment-fa-014-full-hc0s.6` remains in progress.
