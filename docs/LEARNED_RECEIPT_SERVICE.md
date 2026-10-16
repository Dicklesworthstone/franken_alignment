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
order is closed: one selected mode, one actor profile, one mandatory input
anchor, and at most one explicit finish predecessor. Credibility activation,
reviewer/live-generation flags, stop, audit, successor-anchor output and mixed
receipt modes are rejected; they cannot silently select a less guarded route.
Existing single-shot learned receipt commands and raw/stream receipt services
retain their behavior.

## Provisioning and startup

`CONFIG` and `LEARNED_RECIPE` are the original independently selected supervisor
and learned numerical inputs. Plain learned, required raw-residual forecasts and
required owned-K/V forecasts use their corresponding original guarded cursors.
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
Retain the archive and actual recovery floors independently. The service does
not write or advance a successor anchor.

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

`--reconcile-receipts` instead consumes `finish_reconciled_receipt_port`. The
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
