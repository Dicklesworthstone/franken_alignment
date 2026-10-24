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

## Explicit operator settlement

The original learned publication command now accepts:

```text
create-learned-generated CONFIG LEARNED_RECIPE --resume --reconcile-receipts [--history-anchor INPUT]
create-learned-generated CONFIG LEARNED_RECIPE --resume --reconcile-receipts --finish-after-message PREVIOUS_REQUEST [--history-anchor INPUT]
```

This opt-in path consumes the original ordinary/raw/owned guarded cursor through
`finish_reconciled_receipt`; it does not construct a driver and then replay the
same numerical history for new Time and Sweep transactions. A single monotonic
budget covers command intake, original asset loading, pre-open, every replay
event and Ready. The trusted logical clock is called only by the final native
transaction, once. No current policy evidence, native helper, reviewer/actor/stop
socket, new sampled token or refreshed qualification enters settlement.

Both explicit receipt modes use the same closed argument/asset intake. They
reject mixed or duplicate modes, live continuation, next-message, stop, review
audit, credibility activation and successor-anchor output before reading input.
Path-valued anchor parsing precedes boolean flag removal, so a missing anchor
value cannot be repaired by stripping the reconciliation flag. Required model,
tokenizer, codec, predictor, full guards, floors and optional old anchor retain
their exact existing meaning. Missing or mismatched assets have no fallback.

`--receipts-only` remains non-mutating and uses its unchanged historical outcome
finalizer and elapsed-check sequence. Ordinary `--resume` also retains its
original owner/reconciliation workflow. The new explicit settlement spends three
original events even for an already terminal selected record; it is not a
capacity-free retry. Original sweep outcomes for other keys are not represented
by the selected actor response. Inspect them independently as needed.

After acknowledgment, a second elapsed check can withhold the response but now
explicitly reports that reconciliation was acknowledged. It cannot claim that
nothing changed, roll back a confirmed image or retry an effect. A partial stdout
failure retains the original response encoder's no-retry behavior. Subsequent
`--receipts-only` inspection recovers the historical outcome without another
fence. Only confirmed execution returns the original successful-publication exit
status; a denied, pending or unresolved result is not relabeled success.

The outer actor-service dispatcher now routes BOTH receipt flags. This also
repairs the existing baseline stream `--reconcile-receipts` path, whose inner
implementation was previously unreachable from that dispatcher. Exact learned
verbs still delegate to numerical parsing instead of a weaker stream profile;
baseline stream recovery retains its existing complete grammar, required request,
independent anchor, reserved capacity and peer-bound non-admitting service.

Seven additional regression functions cover actual loaded message and finish
commands across all three modes, wrong finish/predecessor selection, unchanged
Unknown liabilities, every precommit timer cut, acknowledged late expiry, partial
stdout failure followed by read-only retrieval, conflicting modes/path options,
and both real outer dispatch paths. The baseline test uses an original retained
policy refusal and the existing stream service; missing-key retry leaves staging
and canonical bytes unchanged. Tests use synthetic model data as controls, not
calibration evidence. Existing read-only tests and assertions remain unchanged.

These seven tests and the four library-integration tests remain UNEXECUTED.
Targeted and full remote-only RCH launches stop before compilation because the
launcher is absent; no Rust toolchain is available locally. Verified preimages,
Git blob checks and selected-source whitespace checks do not qualify compilation,
behavior, formatting, Clippy, performance or production deployment. No gate,
assertion, dependency requirement or original packet acceptance was weakened.

## Refusal-state clarification (2026-10-24)

The original receipt projection distinguishes native nonadmission with no
retained action from an admitted terminal policy denial. Reconciliation must
preserve both, spending the same original three-event transaction without new
generation, dispatch or permission. The corrective fixtures exercise both
classes in the original learned and baseline command routes, beside unchanged
confirmed-publication and unresolved-charge controls. The original selection,
canonical-byte, staging, timer, floor and acknowledgment checks remain in place.
See [`LEARNED_RECEIPT_SELECTION.md`](LEARNED_RECEIPT_SELECTION.md#retained-nonadmission-and-policy-denial-2026-10-24)
for the source-only preflight boundary. This is an existing-contract clarification;
the implementation status retains the actual execution and qualification limits.
