# Single-replay learned receipt reconciliation

This is scoped FA-014 implementation (plan sections 8.4 and 16.4), not packet
closure. The consumer is the learned publication operator's explicit receipt
settlement path. Ordinary recovery previously issued a live owner and then
replayed numerical history again for time and reconciliation transactions.

`finish_reconciled_receipt(request, finish_after, clock)` now consumes the
original complete learned recovery into one acknowledged `Fence -> Time -> Sweep`
replacement. `None` selects a message; `Some(previous)` selects a finish with its
exact confirmed predecessor. The base learned cursor, ordinary/raw-residual/owned-K/V guarded cursors and
both topology-mediated learned cursors expose this operation. Each guarded wrapper checks
its original independent configuration, policy, inventory and floors before
forwarding. Optional exact history anchors keep their existing prefix contract.

The native guarded reconciliation engine and learned finalizer share one private
transaction implementation. No caller-supplied candidate, event list or closure
is accepted through a public interface. The same original source-admission,
consistency preflight, reducer, prefix/byte checks, canonical rechecks, staging
cleanup and single replacement remain in effect. Existing native guarded and
Andon recovery retain their role-provisioning behavior after acknowledgment.

The original `finish_reconciled_receipt` returns only `Knowledge<ActorOutcome>`
through the existing request-book projection. It issues no observer, reviewer, effect permit
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
create-learned-generated CONFIG LEARNED_RECIPE --resume --reconcile-receipts [--finish-after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor NEW_OUTPUT [--retain-topology-checkpoint NEW_OUTPUT]]
```

This opt-in path consumes the original ordinary/raw/owned guarded cursor through
`finish_reconciled_receipt`; it does not construct a driver and then replay the
same numerical history for new Time and Sweep transactions. A single monotonic
budget covers command intake, original asset loading, pre-open, every replay
event and Ready. The trusted logical clock is called only by the final native
transaction, once. No current policy evidence, native helper, reviewer/actor/stop
socket, new sampled token or refreshed qualification enters settlement.

Both explicit receipt modes retain their closed argument and asset intake.
They reject mixed or duplicate modes, live continuation, next-message, stop,
review audit and credibility activation. `--receipts-only` also rejects either
retained-output flag before configuration intake. Reconciliation admits its
explicit outputs before replay: paths must be new, absolute, outside the
authority store and distinct after parent-path normalization. Duplicate,
unpaired, occupied, unsafe or mixed-cut selections refuse before the trusted
settlement clock. Path-valued parsing precedes boolean flag removal. Required
model, tokenizer, codec, predictor, full guards, floors and input anchor retain
their existing meaning; missing or mismatched assets have no fallback.

`--receipts-only` remains non-mutating and uses its unchanged historical outcome
finalizer and elapsed-check sequence. Ordinary `--resume` also retains its
original owner/reconciliation workflow. The new explicit settlement spends three
original events even for an already terminal selected record; it is not a
capacity-free retry. Original sweep outcomes for other keys are not represented
by the selected actor response. Inspect them independently as needed.

When retention is selected, both exports are attempted independently before
the post-acknowledgment elapsed check or stdout write. A late deadline withholds
the response but still attempts retention. An export failure does not suppress
the other export or the original response; combined failures are preserved.
Every such error identifies the acknowledged revision and cannot undo the
transaction or retry an effect. Only confirmed execution returns the original
successful-publication exit status; denied, pending or unresolved results are
not relabeled success.

Commands without retained outputs preserve their previous behavior. Successful
reconciliation advances the journal by three events, so an older v2 source
checkpoint no longer satisfies subsequent exact-cut topology recovery. There is
no receipt counterpart of `--recover-terminal-checkpoint`: an earlier pair is
not promoted across an unretained suffix after interrupted exports. Keep the
original archives and every successfully written successor.

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

## Retained topology receipt custody

Selecting `--retain-history-anchor` requires an independent `--history-anchor`.
Without topology, this retains the complete acknowledged history. With
`--topology-profile`, both retained-output flags are required, and the selected
version-2 source checkpoint must contain exactly the independent input anchor.
Version-1 manual source minima cannot produce a retained source checkpoint.
The original complete cursor checks the selected source cut before its clock.

Use the existing historical
[version-2 profile](TOPOLOGY_CHECKPOINT_OPERATOR.md#subsequent-publication),
omitting its live-source `retain_checkpoint` field. Its `source_checkpoint`
must select the same cut as `history-0.anchor` below. Initial historical
documents remain required; receipt recovery does not read the `next` live files.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- \
  create-learned-generated CONFIG.json LEARNED_RECIPE.json \
  --resume --reconcile-receipts \
  --history-anchor /operator/custody/history-0.anchor \
  --topology-profile /operator/custody/receipts-0.json \
  --retain-history-anchor /operator/custody/history-1.anchor \
  --retain-topology-checkpoint /operator/custody/source-1.bin
```

For an original finish request, also supply
`--finish-after-message PREVIOUS_REQUEST` with its exact confirmed predecessor.

After both exports succeed, save a new historical profile as
`/operator/custody/receipts-1.json`, changing `source_checkpoint` to
`/operator/custody/source-1.bin`. Source work and the last operation have not
advanced, so the existing valid operation selector remains valid. Inspect the
same request without another fence:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p fa-reference \
  --example supervise_publication -- \
  create-learned-generated CONFIG.json LEARNED_RECIPE.json \
  --resume --receipts-only \
  --history-anchor /operator/custody/history-1.anchor \
  --topology-profile /operator/custody/receipts-1.json
```

Retain the finish predecessor option when inspecting a finish. A further
explicit reconciliation can select this successor pair and two new output paths;
it still spends three events and preserves unresolved dispatched charges. The
old pair remains rejected once the journal advances. These are two independent
exports, not an atomic multi-file transaction or archive authentication.

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

## Acknowledged receipt history and source custody

The original learned recovery now offers
`finish_reconciled_receipt_with_history(request, finish_after, clock)`
on the base, guarded, raw-predictive, owned-predictive, mediated and predictive
mediated cursors. It returns `FileReconciledLearnedReceipt`: the selected
redacted outcome, acknowledged snapshot, exact history anchor and revision before
this one reconciliation. Its fields are private. It contains no Store lock,
owner, observer, reviewer or publication permission.

The existing fixed Fence/Time/Sweep engine and all selection, replay, capacity,
clock and canonical-byte checks remain authoritative. After its single
replacement acknowledges, the engine moves the already encoded replacement
bytes into the private history-anchor constructor. The history handoff adds no
numerical replay or fallible post-commit encoding. Failed or ambiguous persistence
returns no candidate receipt or custody, even if the complete replacement became
visible. An error cannot establish that no transaction occurred.

For an independently selected topology checkpoint, use
`finish_reconciled_receipt_with_checkpoint(request, finish_after, saved, clock)`.
It checks that checkpoint's complete source work, graph, availability, exact
history extent and prefix, canonical profile/path, and authority floors before
the trusted clock or mutation. Each guarded wrapper also retains its independent
current credential epoch, policy, predictor and higher floors. A stale source
checkpoint cannot be used as a substitute for the exact pre-settlement cut.

`saved.after_reconciled_receipt(profile, identity, &receipt)` derives the
matching source successor from the immutable result. It accepts only that same
complete starting cut followed by exactly one Fence/Time/Sweep. It preserves the
original topology/inventory bytes, spent checks/read allowance and last source
operation, while retaining the acknowledged floors and unavailable topology.
No current source files are opened, no graph is recertified and no source budget
is replenished. Prefix validation decodes and checks the retained original
history; it does not execute numerical history again.

The sealed actor-port counterparts return the original immutable receipt port
together with the same operator custody. They encode the original source-only
message or finish envelope before mutation. Sensitive history and snapshot data
remain separate from the redacted actor response. The port still cannot cancel,
admit a new request, refresh an outcome or obtain a live owner.

All export operations remain independent of native acknowledgment. Retain the
previous source/history pair until its successor has been saved. These APIs do
not infer a safe suffix from a stale checkpoint, perform a zero-event settlement
retry, or authenticate operator-provided archives. Unknown dispatched effects
remain charged unless the original endpoint receipt resolves them; a Fence can
still cancel undispatched work under the existing native rules.
