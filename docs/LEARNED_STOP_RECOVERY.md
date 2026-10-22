# Offline stop and drain for learned histories

The learned publication operator can recover an existing original history
directly into its native stopped state, without live helpers, reviewer custody,
current policy evidence or another publication attempt:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION --history-anchor INPUT
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION --history-anchor INPUT --retain-history-anchor OUTPUT
```

This is the existing Linux reference example and its original Stop operation.
`OPERATION` is a nonzero original stop identity. A matching retained stop reuses
its original request; a different operation cannot replace it. The independently
selected configuration, primary numerical assets and raw or owned predictor
assets must still match the history. Native reviewer models, reviewer/stop
sockets and the policy evidence file are not opened. An optional
`--credibility-activation` capsule is ignored without being read.

The command accepts an independently retained history anchor and can retain its
acknowledged stopped successor using `--retain-history-anchor OUTPUT`. INPUT is
optional; omitting it does not establish external history continuity. OUTPUT must
be a new private file outside the mutable authority directory. The same original
path admission and bounded anchor decoder apply. Keep the old archive until the
new export succeeds. Stop recovery cannot be combined with generation
continuation, next-message or finish options and never creates a fresh authority
store.

## Separate sealed library phases

The original learned cursor and all five guarded variants expose a clock-free
admission stop and a fallible later drain:

```rust,ignore
let request = recovery.stop_request(operation)?;
let admitted = recovery.finish_admission_stop(request)?;
// On a separately opened and completely verified stopped history:
let request = stopped_recovery.stop_request(operation)?;
let drained = stopped_recovery.finish_stopped_drain(request, fallible_clock)?;
```

Admission appends only the original Stop/Fence pair and returns
`FileAdmissionStoppedHistory`, including its exact acknowledged anchor.
A matching retained stop appends zero events, even when unresolved liabilities
remain. It does not consult logical time or attempt settlement. Later drain
requires the same acknowledged stop and appends only its original StopProgress
event when needed. A completed drain retry uses no clock or additional event.
Both phases preserve executed and unresolved charges without returning a live
owner, predictor, reviewer or topology role.

Each guarded consumer calls the original complete readiness check before
delegating to the same verified machine and exclusive canonical store.
Independent policy, floor, predictor, topology, availability, reserve and anchor
requirements still apply. `try_finish_stopped_with_anchor` additionally exposes
fallible-clock combined completion on every cursor, preserving the original
atomic transaction and returning no successful receipt or anchor on refusal.
Existing infallible combined methods retain their signatures and behavior.
Command-line phase selection is a separate integration step.

Four public regression functions exercise real reserved, dispatched, executed
and expired effects through the learned, guarded, raw-predictive and
owned-predictive cursors. Controls cover exact anchors and retries, unavailable
clocks without an implicit admission fallback, canonical replacement during
clock acquisition, and failed independent floors on every new consumer.
Their execution qualification is recorded in IMPLEMENTATION_STATUS.md.

## The original sealed completion

`FileLearnedRecovery`, `FileGuardedLearnedRecovery`,
`FilePredictiveLearnedRecovery`, `FileOwnedPredictiveRecovery`,
`FileMediatedLearnedRecovery` and `FilePredictiveMediatedLearnedRecovery` expose:

```rust,ignore
let request = recovery.stop_request(operation)?;
let stopped = recovery.finish_stopped(request, trusted_clock)?;
```

Both methods require complete original replay. Guarded variants additionally
check the same independently required guard inventory, effective policy,
credential epochs, external floors and exact predictor ownership. Anchored
variants keep the selected prefix on the same exclusive canonical cut. The
operator also requires the exact original `RecoveryReserve::terminal()` before
the first replay event. No requirement is inferred from a weaker opener or
changed after replay begins.

The advancing terminal transaction appends exactly the original
`Stop -> Fence -> StopProgress` sequence in one canonical replacement. It uses
the fully verified machine, without another numerical replay or an ordinary
recovery fence first. Every encoded prefix still passes the original event and
byte limits. The three-event terminal tail therefore remains usable when
ordinary journal capacity is exhausted; recovery never enlarges that reserve.

The clock is called once only when progress is required. Exact request validation
and canonical comparison precede the callback; canonical comparison occurs
again after native terminal preflight and before staging cleanup/replacement.
Stale intent, a changed canonical image or invalid time returns no stopped
receipt and does not clean staging. Storage failure returns no candidate receipt,
although the complete replacement may already be visible after an ambiguous
final synchronization. Retrying verifies the actual canonical history again.

If the same stop is already drained, completion confirms the existing canonical
image and returns `FileStoppedRecovery::AlreadyDrained`. It needs no clock call,
new event, fence or replacement, including when the journal is full. Confirmed
canonical durability may remove an abandoned staging file; staged bytes are
never promoted into a new outcome.

## Capture the acknowledged stopped history

All six cursor types additionally expose `finish_stopped_with_anchor`, returning
`FileStoppedHistory` from `observed::shutdown::recovery`:

```rust,ignore
let request = recovery.stop_request(operation)?;
let history = recovery.finish_stopped_with_anchor(request, trusted_clock)?;
let (stopped, anchor) = history.into_parts();
```

The stop observation and private `FileHistoryAnchor` describe the same exact
acknowledged revision. Advancing completion moves the bytes of the successful
original replacement into the anchor; already-drained completion moves the
identical verified canonical image after confirming durability. Capture adds no
journal event, clock call, numerical replay, re-encoding or storage reopen.
It retains the already bounded canonical byte buffer until the anchor is dropped.
The ordinary `finish_stopped` API keeps its original return type and semantics.

Complete independent guard, predictor, floor, reserve and input-anchor checks
remain mandatory. A failed terminal write returns neither a stopped observation
nor a candidate anchor, even when a replacement might have become visible.
The returned history contains no live owner, observer, reviewer or authority.
Pending and irrecoverable effects remain charged and can still appear in the
anchor; possessing an anchor is not evidence that the domain has drained.

The library result is not an export acknowledgment. The custodian must write
and synchronize a separate private archive, preserving its old archive until
that succeeds. Export failure does not undo the stop or justify resending an
effect. A later canonical successor does not change the captured revision.
Independent custody and protection against rolling back both journal and
archive remain operator assumptions.

## Retain the terminal anchor from the operator

When OUTPUT is selected, all three original operator modes consume
`finish_stopped_with_anchor` through the same one-event replay driver, preserving
every admission and Ready checkpoint. The command exports that captured anchor
through the existing bounded private-file writer. It does not obtain a live
owner, reopen the authority store, capture a newer revision or perform another
numerical replay. The file is created exclusively with mode 0600 on Unix, then
both the file and its parent directory are synchronized before export succeeds.
A path appearing after admission is never overwritten.

Export precedes report rendering. On export failure the command still attempts
the native stop report and returns an error that explicitly preserves the
acknowledged stop and says retention failed. A simultaneous output failure or
pending drain is retained in the same error rather than hiding either outcome.
The native stop report does not by itself acknowledge archive retention; check
the command result. No path, anchor bytes or private evidence is added to stdout.

After successful retention, INPUT may be the new stopped anchor. A drained retry
with a fresh OUTPUT writes the identical anchor without any terminal clock,
extra journal event or replacement. After failed retention, keep INPUT and select
a different new OUTPUT; an existing or partial output is never overwritten or
silently accepted as acknowledged custody. A pending drain still requires its
original time/capacity and may advance the next retained revision. Export remains
separate from native persistence: failure cannot undo a stop or refund a charge.

## Results and liabilities

Stdout uses the existing native stop JSON report. `stopped_drained` means the
original stop and drain are confirmed. `stopped_pending` reports the unresolved
and irrecoverable operation IDs and returns a command error. Original executed
effects and expired unknown outcomes remain charged. Only original receipts can
release liabilities; stopping never resends an effect or converts uncertainty
to a successful receipt. A repeated pending drain still needs fresh time and
finite terminal capacity.

If report output fails after the stop is acknowledged, the error says so. Retry
the same operation to report the verified original stopped cut. Output failure
does not initiate another effect or invalidate an acknowledged stop.

The operator services its elapsed replay budget before opening, before every
original event and once more at Ready. Cancellation drops the original cursor
without fencing, cleaning staging, obtaining terminal time or returning a live
role. Initial framing, an individual reducer call and final storage work remain
synchronous; the budget is cooperative, not a hard cancellation-latency bound.

This handles one original local reference sink. It does not stop an OS process,
prove remote-provider settlement, authenticate independent anchor custody or
protect a jointly rolled-back journal and anchor.

## Verification status

Eight public integration tests cover all four cursor types, full terminal
capacity, reserved/sent/executed/expired effects, exact stop retries, guard and
anchor refusal, canonical conflicts and unavailable/stale clocks. The original
internal storage test exercises all five Store failure barriers for both sent
and executed histories, preserving sampled work and pending numerical intent.
An additional regression runs the same barriers through anchored completion,
compares the returned anchor against actual canonical bytes, and requires an
identical anchor on a drained retry with replacement deliberately unavailable.
Five operator tests cover loaded modes, replay checkpoints, missing live assets,
the actual CLI, original guard admission and failed output.

Seven additional operator regression functions cover exact private export and
reuse in all three modes; actual CLI export without live assets; late output
collisions and corrected retries; unsafe custody refusal; simultaneous export
and report failure; all original replay deadline cuts; and invalid stop or
independent guard refusal. Existing no-export and lost-report tests remain,
with the former unsupported-output assertions replaced by positive terminal
retention and configuration-admission controls.

The new anchor implementation, operator integration and regressions have not
been compiled or executed in this session. The required remote-only RCH execution gate remains pending;
source inspection does not establish type correctness, runtime behavior or
release qualification. The scoped FA-014 bead and full packet remain open.
This implements plan sections 16.4/16.5/16.9/25.4 and FI-A02 without adding an
agent verb, dependency, alternative journal or effect executor.
