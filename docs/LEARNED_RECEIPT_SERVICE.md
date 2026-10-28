# Peer-checked learned receipt service

The Linux `supervise_publication` example can serve an original learned message
or finish receipt without retaining a writable supervisor:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume --serve-receipts ACTOR_PROFILE --history-anchor INPUT --receipts-only
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume --serve-receipts ACTOR_PROFILE --history-anchor INPUT --reconcile-receipts
```

For an original finish, place its exact predecessor before the final mode:

```text
supervise_publication create-learned-generated CONFIG FINISH_RECIPE --resume --serve-receipts ACTOR_PROFILE --history-anchor INPUT --finish-after-message PREVIOUS_REQUEST --receipts-only
```

The same finish selection is available with `--reconcile-receipts`. The argument
order is closed: one actor profile, one mandatory input anchor, at most one
explicit finish predecessor, and one selected mode as the final argument.
Reconciliation also accepts explicit retained outputs and the narrowly scoped
`--recover-receipt-checkpoint` option, described below.
Credibility activation, reviewer/live-generation flags, stop, audit and mixed
receipt modes are rejected; passive mode also rejects either retained-output
flag before configuration intake. Existing no-output learned receipt commands
and raw/stream receipt services retain their behavior.

## Provisioning and startup

`CONFIG` and `LEARNED_RECIPE` are the original independently selected supervisor
and learned numerical inputs. Plain learned, required raw-residual forecasts and
required owned-K/V forecasts use their corresponding original guarded cursors.
For a topology-mediated learned history, also supply `--topology-profile PATH`
with the existing `fa.mediated-submit/1` or `/2` historical contract. Retained
source/history output requires the exact version-2 checkpoint pair; version-1
manual minima cannot establish complete source custody. Both sealed mediated cursors retain complete guard,
predictor, original/current topology, availability, floor, anchor and exact
reserve checks before selecting a receipt. The input anchor is mandatory in
every topology recovery mode. Historical admission never opens the profile's
`next` live topology files or returns a live topology observer. See
[learned topology operation](LEARNED_OPERATOR_PUBLICATION.md#enforce-topology-during-learned-publication)
for checkpoint extent, spent work and withdrawn-availability requirements.

Primary model, tokenizer, fit/monitor and predictor assets still have to bind to
the exact retained history. Recovery is not inference-free: verification of
historical numerical operations and original fitting still performs work.

The loader does not open native helper assets or current policy evidence, claim
reviewer or stop sockets, or restore a live approval role. A skipped native-roster
path is still part of the original closed recipe schema. No missing file is
replaced with a different model or an ordinary helper process.

`ACTOR_PROFILE` uses the existing `fa.actor-service/1` schema. Its request must
match `LEARNED_RECIPE.request`, and its scope/clock and supervisor-owned socket
directory must match CONFIG. Kernel UID/GID and optional PID checks, socket path
and ownership rules, candidate/connection/exchange limits and channel framing
are the existing peer-service contracts. Model labels do not authenticate the
connecting principal.

The input anchor is mandatory and uses the original independently retained
`FileHistoryAnchor` archive. Existing admission requires its canonical path to
be outside the mutable authority store. This path check does not establish
independent ownership, authenticate the custodian or protect joint rollback.
Retain the archive and actual recovery floors independently. Passive service
does not advance the journal or export a successor. Explicit reconciliation can
retain the acknowledged pair when the output options below are selected.

One monotonic invocation budget begins before configuration ingestion and is
clamped to the smaller CONFIG/ACTOR_PROFILE runtime. Startup checks it after
intake, before opening the original cursor, at every original replay event and
at verified Ready. Complete guard, predictor, policy, external floor, exact
history-prefix and original terminal-reserve checks remain mandatory. The
request, release kind and accepted finish predecessor are selected on the same
verified cursor before socket binding or any recovery mutation. A wrong or
missing request cannot spend a fence or create an endpoint.

The new socket is bound and secured after that selection but before the consuming
finalizer. An existing path is not removed; collision drops the replay cursor
without cleaning staged files or changing canonical history. Deadline checks
surround finalization as well. Initial asset reads, one reducer call and native
persistence remain synchronous and can overrun a checkpoint; this is not a hard
latency guarantee or an independent watchdog.

## Passive versus explicitly reconciled observations

`--receipts-only` consumes `finish_receipt_port`. It verifies the original
canonical cut and returns a sealed immutable receipt without a clock, fence,
cleanup, sync or journal replacement. It needs no spare journal capacity.
Canonical and staged bytes remain unchanged. Pending and unknown outcomes remain
pending and unknown, including a publication not yet reconciled into its receipt.

`--reconcile-receipts` consumes `finish_reconciled_receipt_port`, or its
history/checkpoint-retaining counterpart when outputs are selected. The
original fixed `Fence -> Time -> Sweep` transaction must acknowledge before the
immutable port is issued. It observes its one trusted logical clock only after
replay/selection; queries original endpoint receipts without sending an effect;
and retains unresolved liabilities and charges. This requires the original
ordinary capacity for Time/Sweep. Terminal reserve is not additional ordinary
funding. A finalization error can leave an ambiguous visible replacement; it does
not trigger a fallback, new generation, second sweep or effect retry.

After successful reconciliation, a deadline or service error explicitly reports
that reconciliation was acknowledged. Failure to start or complete transport
cannot undo that cut or imply that no publication occurred. Observe it again
with passive receipt mode rather than inventing another effect request.

## Retain custody before serving reconciled receipts

For a topology-mediated learned history, select an existing exact v2
source/history pair and distinct new outputs:

```sh
supervise_publication create-learned-generated CONFIG.json LEARNED_RECIPE.json \
  --resume --serve-receipts ACTOR_PROFILE.json \
  --history-anchor /operator/custody/history-0.anchor \
  --topology-profile /operator/custody/receipts-0.json \
  --retain-history-anchor /operator/custody/history-1.anchor \
  --retain-topology-checkpoint /operator/custody/source-1.bin \
  --reconcile-receipts
```

The historical profile omits the live-source `retain_checkpoint` field.
Its nested source anchor must exactly equal the independent input anchor.
Non-topology reconciliation can select only `--retain-history-anchor` with its
mandatory input. Topology retention requires both outputs, an exact v2 profile
and the matching input anchor. A finish additionally selects
`--finish-after-message PREVIOUS_REQUEST` before the final mode.

Both outputs must be new absolute paths, distinct after parent normalization,
and outside the authority store. Neither may name the service socket, including
through a symlinked parent or ancestor alias. The socket and both output paths
are compared using canonical parents before recovery begins. Option and path
admission precede replay.
Original request selection, socket binding/security and the pre-finalization
deadline checks still precede the trusted clock and replacement. In particular,
an occupied socket leaves the journal and staged files unchanged.

The native finalizer validates the complete selected source cut before mutation
and captures the acknowledged canonical bytes under the original recovery lock.
After acknowledgment, the service attempts both archives independently before
checking the late deadline or constructing a peer session. An export failure
serves no receipt and reports the acknowledged revision; a successful partner
archive remains retained. The socket was already bound for precommit validation,
but no actor frames are processed before retention succeeds. A later deadline
or rejected peer cannot erase the retained pair or trigger another effect.

The successor retains original source documents, spent work and last source
operation, with the acknowledged floors and unavailable topology. No live source
is read or recertified. History and snapshot contents stay with the operator;
the actor still receives only the original redacted immutable observation.

Use the successor anchor and a new historical profile pointing to the successor
source checkpoint for subsequent passive service or explicit reconciliation.
See [retained receipt custody](LEARNED_RECEIPT_RECONCILIATION.md#retained-topology-receipt-custody)
for profile selection and the next passive command. No-output reconciliation
preserves the previous behavior and makes an older v2 checkpoint stale when the
journal advances. Interrupted receipt exports require the explicit restricted
repair mode below; preserve the original and every successfully written
successor archive.


## Repair interrupted exports before serving

Use the original independent matching v2 source/history pair, two fresh output
paths, and `--recover-receipt-checkpoint` before the final reconciliation mode:

```sh
supervise_publication create-learned-generated CONFIG.json LEARNED_RECIPE.json \
  --resume --serve-receipts ACTOR_PROFILE.json \
  --history-anchor /operator/custody/history-0.anchor \
  --topology-profile /operator/custody/receipts-0.json \
  --retain-history-anchor /operator/custody/history-repaired.anchor \
  --retain-topology-checkpoint /operator/custody/source-repaired.bin \
  --recover-receipt-checkpoint --reconcile-receipts
```

Keep the original selected request and, for a finish, its exact
`--finish-after-message PREVIOUS_REQUEST` before the final mode. Missing or
mismatched custody, version-1 manual minima, incomplete outputs, duplicate/mixed
flags and passive repair refuse before recovery. Original immediate option
values are validated before stripping any flag. The original fixed actor
profile/input prefix and final-mode position remain required.

The service uses the same restricted native repair cursor as the one-shot
operator, with full original plain/raw/owned numerical replay, guards, current
credential epoch, source history, floors and reserve. At the unchanged cut it
performs the original settlement. With exactly one complete Fence/Time/Sweep
after the saved prefix, it confirms that existing cut with canonical reread,
synchronization and cleanup, without another clock, event, replacement or sweep.
Every partial, extra or unrelated suffix refuses. See
[the native repair contract](LEARNED_RECEIPT_RECONCILIATION.md#recover-source-custody-after-interrupted-receipt-exports).

Archive/socket alias checks run before opening recovery. After complete replay,
the exact historical request and finish predecessor are checked before the
socket is bound and secured. Occupied or insecure sockets therefore refuse
before native confirmation, cleanup or mutation. The original invocation
deadline still covers every replay event, Ready and finalization.

Both independent archives are attempted before late-deadline handling or
construction of a peer session. Any requested retention failure serves no
receipt, preserves a successful partner archive and reports the confirmed
revision. A later deadline or rejected kernel peer cannot erase successful
retention. No actor frames can initiate repair, reconciliation or source work.

The original sealed port carries only its exact historical envelope and
redacted outcome. Unknown remains Unknown, Cancel remains refused, and reconnect
does not refresh the observation. Choose new output paths for another explicit
repair invocation while the permitted successor remains current. Successful
repair supports later passive service from the newly retained matching pair.

## What the actor can access

The returned [sealed receipt port](SEALED_LEARNED_RECEIPT_PORT.md) contains only
one exact source-only Submit envelope and its original redacted
`Knowledge<ActorOutcome>`. The recovered Store lock, live generation state and
approval roles are released before the first peer exchange. The command may
retain its immutable loaded assets and configuration, but those inputs and its
policy-file reader are not handed to the receipt port. The actor uses the existing
client:

```text
supervise_publication actor-submit ACTOR_PROFILE ORIGINAL_SUBMIT_JSON
```

`ORIGINAL_SUBMIT_JSON` is the original source-only envelope encoded by
`FileLearnedTextStreamActorPort::encode_release`, not caller-supplied output text
or a newly derived cumulative stream frame. Every original field and intent byte,
including the old deadline, must match. Changed envelopes conflict; other keys
remain Withheld. An exact Submit issues only an instance-bound receipt ticket.
Cancel is always refused, even for an unsent request: the immutable observation
cannot release a real journal reservation. Poll and reconnect cannot admit work
or initiate another reconciliation.

The SAME existing bounded `serve_session` peer loop services the port. It retains
one service window, candidate quota and original peer session across reconnects.
There is no second wire parser, outcome reducer, policy read, helper, human
channel, authority or effect executor in this path. Transport counts printed to
stderr do not certify effect success, receipt currency or resolved liabilities.

**The receipt is historical at the recovered cut.** Because the Store lock has
been released, another authorized writer may advance the original journal while
this service runs. That cannot silently update the saved observation. Known
terminal values retain their original request-local basis; Pending/Unknown never
become current observations merely by polling. A new observation requires a new
independently admitted invocation. Operators must bound service lifetime and
protect the selected profiles and socket namespace.

## Verification record: 2026-10-16

Eight new example regression functions use actual imported generation, original
native-model review, separate human approval, canonical publication and the real
Unix peer client/loop. Coverage includes all three learned modes, pending and
cancelled requests, published-but-unknown versus explicitly reconciled outcomes,
finish selection, reconnect and Cancel refusal, missing live assets, original
guard/floor and socket-collision refusals, all startup event/Ready deadlines,
post-acknowledgment expiry, rejected credentials, and the actual command's file
and closed-option admission. They do not bypass unavailable sockets or substitute
canned native votes. The lower-level port tests additionally cover full journals,
canonical changes and per-byte envelope/ticket binding. Borrowed-host test calls
capture revisions before mutable method calls; no admission assertion changes.

These tests are authored and source-reviewed, not executed. The example selection,
the `learned_receipt_actor_port` integration target and the unchanged full
`cargo run --locked -p xtask -- check` were each attempted through
`RCH_REQUIRE_REMOTE=1 rch exec -- ...`; all three exited 127 before compilation
because `rch` is absent. Cargo, rustc and rustfmt are unavailable in this editing
runtime. No type check, syntax/formatting, Clippy, runtime, latency or release
qualification is claimed. Exact edited baseline blobs and uploaded overlays were
compared by Git identity; that comparison is not test execution.

This composes the existing FA-014 recovery and FA-107 one-directional actor
contracts. It adds no production dependency, journal tag, wire command or
alternative numerical/authority engine. Full packet acceptance remains open.
The required `br` tool is unavailable; existing Beads records and qualification
gates were not rewritten to claim completion.

## Retained-custody verification, 2026-10-27

Four new service regression functions exercise actual topology-backed learned
publication across plain/raw/owned modes, paired retention before peer exchange,
reconnect and Cancel refusal, independent archive races, late deadline and
kernel-peer rejection, malformed selections and occupied-socket refusal.
The complete original operator test target type-checks, including these bodies.
Their runtime execution remains open: full harness generation with both LLVM
and the matching official Cranelift backend reached the bounded memory watchdog.
The native integration target passes all nine functions, including its four new
custody controls. See the
[combined verification record](LEARNED_RECEIPT_RECONCILIATION.md#verification-update-2026-10-27)
for scope and the separate interrupted full library-unit check.

## Receipt repair verification, 2026-10-28

Six new resident regression functions are authored against the real original
service fixtures. They cover all three learned modes, loss of either/both
archives, repeated recovery and exact sealed-port reconnect, independent output
races, retention before late expiry or rejected kernel credentials, malformed
options/custody, higher floors, occupied sockets, changed canonical bytes,
partial/unrelated suffixes and real confirmation-cleanup failure. The archive/
socket collision function covers 36 mode/selection/output/alias combinations,
then includes a valid real peer exchange control. Existing fixture bodies and
assertions are retained; only narrow visibility is shared with the new tests.

The final production executable passes direct metadata checking, and all
18 native receipt integration tests pass. Changed Rust sources pass syntax
parsing and independent review. The complete operator test metadata/build
attempts were interrupted by shared host OOM pressure before a runnable harness
or final full-target typecheck existed. These six new resident functions,
together with the seven new one-shot functions, remain unexecuted. No tests,
assertions or libtest registrations were removed. See the
[operator verification record](LEARNED_RECEIPT_RECONCILIATION.md#operator-repair-verification-2026-10-28)
for the actual attempts; canonical workspace/RCH gates and Beads remain open.
