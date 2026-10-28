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
create-learned-generated CONFIG LEARNED_RECIPE --resume --reconcile-receipts [--finish-after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor NEW_OUTPUT [--retain-topology-checkpoint NEW_OUTPUT]] [--recover-receipt-checkpoint]
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
checkpoint no longer satisfies subsequent ordinary exact-cut topology recovery.
The explicit `--recover-receipt-checkpoint` option below can confirm only the
strictly checked receipt successor from the original matching pair. The separate
terminal recovery flag does not apply. Keep the original archives and every
successfully written successor.

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
old pair remains rejected by ordinary recovery once the journal advances;
the explicit repair option below accepts only its narrow receipt successor.
These are two independent exports, not an atomic multi-file transaction or
archive authentication.

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
previous source/history pair until its successor has been saved. These ordinary
exact-cut finalizers do not infer a safe suffix from a stale checkpoint, perform
a zero-event settlement retry, or authenticate operator-provided archives. The
separate restricted repair cursor below handles the specifically checked
receipt successor. Unknown dispatched effects
remain charged unless the original endpoint receipt resolves them; a Fence can
still cancel undispatched work under the existing native rules.

## Verification update, 2026-10-27

The complete unchanged `mediated_learned_receipts` integration target now runs
successfully: **9 passed, 0 failed**, including all four new native custody
functions. It uses actual native publication/reconciliation and all three
guarded learned modes. The integration harness was compiled with LLVM and
linked to the matching official nightly-2026-08-31 Cranelift library; test
threads had 32 MiB stacks. These synthetic scenarios establish the exercised
contracts, not forecast quality or deployment qualification.

The real native library, publication executable, complete publication test
target and mediated receipt target also pass direct metadata checking. Both
new forbidden owner-access snippets fail with E0599 and the permitted immutable
surface compiles. The new five one-shot and four resident-service regression
functions type-check in the complete operator target but remain unexecuted.
Full operator harness builds with LLVM, serialized LLVM and Cranelift stopped
at a 1.7 GiB resident-memory watchdog before a harness existed. The full library
unit metadata attempt was SIGKILLed before completion, so the new five-barrier
unit matrix has neither a completed full-target typecheck nor a runtime pass.
No source, test body, assertion, dependency gate or qualification requirement was
removed to obtain the successful scoped checks.

The runtime and metadata results above precede concurrent main commit
`0fb1c1b`, whose abandonment implementation is preserved during integration.
They do not claim verification of that new event or its regression cases.

## Recover source custody after interrupted receipt exports

An acknowledged receipt reconciliation can leave the journal three events ahead
of the independently retained source/history pair if either export fails.
Ordinary exact-cut recovery correctly rejects that older source checkpoint.
The explicit native repair entry point on `FileMediatedLearnedRecovery` and
`FilePredictiveMediatedLearnedRecovery` is:

```rust,ignore
let mut repair = recovery.require_receipt_checkpoint(
    &saved_source_checkpoint, request, finish_after,
)?;
// Advance this same cursor through its complete original bounded replay.
while repair.progress().status == FileLearnedRecoveryStatus::Replaying {
    let done = repair.progress().replayed_events;
    repair.advance(done, 1)?;
}
repair.check_request()?;
let receipt = repair.finish_reconciled_receipt_with_history(trusted_clock)?;
let successor = saved_source_checkpoint.after_reconciled_receipt(
    &profile, &store_identity, &receipt,
)?;
```

Admission must happen before any replay. It requires the original exact terminal
reserve and checks the selected checkpoint's profile, store identity, history
prefix, source work, graph and original availability. Saved authority/control/
revision minima strengthen the independently selected floors; they never replace
higher floors. The original policy, inventory, predictor and current credential
epoch requirements remain mandatory throughout full numerical replay.

The private `FileReceiptTopologyRecovery` has two accepted source boundaries:

| Events after the retained exact prefix | Finalization |
| --- | --- |
| None | Execute the original single Fence/Time/Sweep transaction, with one trusted clock call and the original ordinary-capacity requirements. |
| Exactly Fence, Time, Sweep | Recheck the complete replay and selected request, reread canonical bytes, synchronize storage and clean verified staging, then return the existing immutable acknowledged cut. |

The second branch adds no event, calls no clock and performs no journal
replacement or new sweep. It still performs full original replay, canonical
filesystem reads, file/directory synchronization and cleanup. Failed confirmation
or cleanup returns no candidate receipt or custody. This is not a zero-I/O
shortcut. It can confirm an already present triplet at full ordinary capacity;
an unchanged cut still cannot spend terminal-only capacity on settlement.

Every other suffix refuses, including a partial or reordered triplet, another
Fence or triplet, source/numerical work, credential maintenance, Stop and
AbandonUnknown. A changed profile, store path, prefix, source state, higher floor,
current credential epoch, request kind or finish predecessor also refuses.
Dropping an incomplete cursor does not clean staging or mutate the journal.
Existing exact-cut recovery APIs remain strict.

The result carries the original selected historical message or finish, exact
history and snapshot, and the saved source boundary in `reconciled_from()`.
The journal-wide Sweep does not encode the triggering request or command.
A literal triplet produced by three separate original native transactions is
also valid source succession; repair makes no claim of single-batch provenance
or of which request triggered it.

`finish_reconciled_receipt_port_with_history(clock)` on the same restricted
cursor returns the existing sealed receipt port plus this operator custody.
It freezes the entire original source-only envelope before confirmation or
mutation. Neither finalizer exposes a Store, live owner, source observer,
reviewer or an alternate recovery operation. The successor retains identical
source documents, work expenditure and last source operation; it does not read
next live files, recertify the graph or replenish a source budget. Unknown and
irrecoverably unknown effects remain charged and cannot be resent by repair.

The old matching pair can be used repeatedly while this exact triplet remains
current. Keep it and every successful successor export until both new archives
are retained. These files remain independent operator custody, not an atomic
multi-file transaction or protection against jointly rolling back the archives
and journal.

## Repair verification, 2026-10-28

The complete unchanged `mediated_learned_receipts` target passes **18/18**
in 59.96 seconds, including all nine new repair functions and all nine existing
receipt/custody functions. The new cases exercise plain, raw and owned modes,
lost exports and repeated repair, a literal separately written triplet, exact
finish selection, ordinary exhaustion, independent credential generation and
higher floors, unsafe suffixes including AbandonUnknown, changed canonical bytes,
and a real staged-cleanup failure followed by successful confirmation.

The first run passed 16/18. The two failures were corrected in fixture setup:
predictive finish 72 now obtains its own original forecast after confirmed
message 71, and the reordered-suffix case first proves that Sweep without Time
refuses before constructing the valid wrong-order control. Native production
contracts and original assertions were not weakened.

The real library and integration target pass metadata checking. The full
library links using the matching official nightly-2026-08-31 Cranelift backend;
the unchanged LLVM integration harness links against it with 32 MiB test-thread
stacks. Both new forbidden wrapper-owner snippets fail with the expected E0599.
The exact production-library source manifest is
`aeff8b3660d0a0097345c7944f46b625bab4808b40bdefa18732a0499e3d68bf`.
These checks include the integrated concurrent main source through `fcc8fe766`,
but do not constitute execution of all abandonment regressions. Canonical
workspace/RCH qualification and Beads remain open.

## Explicit operator repair after interrupted exports

If reconciliation completed but either retained output was lost or failed, use
the original independently retained matching pair with
`--recover-receipt-checkpoint` and two fresh output paths:

```sh
supervise_publication create-learned-generated CONFIG.json LEARNED_RECIPE.json \
  --resume --reconcile-receipts --recover-receipt-checkpoint \
  --history-anchor /operator/custody/history-0.anchor \
  --topology-profile /operator/custody/receipts-0.json \
  --retain-history-anchor /operator/custody/history-repaired.anchor \
  --retain-topology-checkpoint /operator/custody/source-repaired.bin
```

Keep the original selected recipe/request. A finish also requires
`--finish-after-message PREVIOUS_REQUEST` for its exact confirmed predecessor.
The topology profile must use the original v2 `source_checkpoint` and omit its
live-source `retain_checkpoint` field. Its nested history must exactly equal the
independent input anchor. Both outputs are mandatory, new, distinct, absolute
and outside the authority store. Version-1 manual minima, missing/mixed input
custody, passive mode and incomplete output selections refuse. Original immediate
path and finish values are validated before any flags are removed, so stripping
one option cannot repair a neighboring malformed argument.

The command opens the original plain/raw/owned mediated recovery with all
independent guards, then consumes it into the sealed native repair cursor.
The ordinary selected-cursor route keeps its exact source-extent checks. One
monotonic budget and one-event cooperative replay cover intake, pre-open, every
event and Ready. No live driver, source observer or numerical continuation is
created.

At the unchanged original cut, this explicit reconciliation still performs its
one original three-event settlement and clock call. If exactly one complete
Fence/Time/Sweep already follows the saved prefix, the command confirms that
existing cut and exports matching custody without another clock, event or
replacement. A partial, additional or unrelated suffix refuses; repair cannot
reconstruct arbitrary missing custody. The full native confirmation and
selection rules above apply, including current credential epoch, higher floors
and real filesystem synchronization.

Both archives are attempted before late deadline handling or stdout. A failed
archive does not suppress its partner or the original redacted response, and
combined reporting/retention failures retain the confirmed revision. Late
expiry withholds the response but still attempts both archives. Keep any
successful output and choose new paths on the next repair attempt. Repeating
repair from the original pair is valid only while the exact permitted successor
remains current. Unknown outcomes retain their original charge and exit law;
repair is never an effect retry.

After both exports succeed, point a new historical v2 profile at
`source-repaired.bin` and use the matching `history-repaired.anchor` for passive
receipt inspection or the next independently authorized operation. Merely
reissuing ordinary reconciliation from that successor still spends another
transaction; use `--receipts-only` for observation.

## Operator repair verification, 2026-10-28

Seven new one-shot regression functions cover actual retained publication in
plain/raw/owned modes, loss of either/both exports and repeated repair, unchanged
cut settlement and successor selection, closed grammar and original immediate
option values, mixed/manual/unsafe custody and higher floors, real partial/extra
native suffixes, deadline/stdout/output races, and an actual finish with its exact
confirmed predecessor.

The final production publication executable passes direct metadata checking in
4.01 seconds; the complete native receipt target passes 18/18 as recorded above.
Changed Rust files pass syntax parsing and independent source review. Full
operator-test metadata first found a real missing explicit module path in the
concurrently added guarded-abandonment tests; the path is corrected without
changing those test bodies or assertions.

Full operator-test verification remains incomplete. A subsequent metadata check
was stopped after 55.27 seconds when the shared cgroup OOM count increased; the
unmodified full Cranelift harness build was SIGKILLed after 33.31 seconds
(934,376 KiB observed peak) with another shared OOM event. A final lightweight
metadata attempt was interrupted after 14.35 seconds by a further shared OOM
event. No operator harness was produced, so the seven new one-shot functions
have no runtime pass or completed final full-target metadata result. No source,
test body, libtest registration, assertion or gate was removed to fit the host.
Canonical workspace/RCH qualification remains open.
