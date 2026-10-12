# Offline stop and drain for learned histories

The learned publication operator can recover an existing original history
directly into its native stopped state, without live helpers, reviewer custody,
current policy evidence or another publication attempt:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --recover-stop OPERATION --history-anchor INPUT
```

This is the existing Linux reference example and its original Stop operation.
`OPERATION` is a nonzero original stop identity. A matching retained stop reuses
its original request; a different operation cannot replace it. The independently
selected configuration, primary numerical assets and raw or owned predictor
assets must still match the history. Native reviewer models, reviewer/stop
sockets and the policy evidence file are not opened. An optional
`--credibility-activation` capsule is ignored without being read.

The command accepts an independently retained history anchor. It rejects
`--retain-history-anchor` before configuration or recipe I/O: this terminal path
returns no owner from which to export a successor. Keep the selected old archive.
It cannot be combined with generation continuation, next-message or finish
options, and it never creates a fresh authority store.

## The original sealed completion

`FileLearnedRecovery`, `FileGuardedLearnedRecovery`,
`FilePredictiveLearnedRecovery` and `FileOwnedPredictiveRecovery` expose:

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
anchor refusal, canonical conflicts and unavailable/stale clocks. One internal
test exercises all five original Store failure barriers for both sent and
already executed histories, preserving a sampled step and pending numerical
intent. Five operator tests cover all three loaded modes, every replay checkpoint,
missing live assets, the actual CLI, original guard admission and failed output.

These fourteen tests are authored and independently source-reviewed. The
required remote-only RCH execution gate remains pending; source parsing does
not establish type correctness, runtime behavior or release qualification.
The scoped FA-014 bead and full packet remain open. The change implements the
existing recovery and liability contracts in plan sections 16.4/16.9 and FI-A02;
it adds no agent verb, dependency, alternative journal or effect executor.
