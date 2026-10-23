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
optional for histories without topology; topology recovery requires INPUT.
Omitting it for a non-topology history does not establish external history
continuity. OUTPUT must
be a new private file outside the mutable authority directory. The same original
path admission and bounded anchor decoder apply. Keep the old archive until the
new export succeeds. Stop recovery cannot be combined with generation
continuation, next-message or finish options and never creates a fresh authority
store.

## Separate operator phases

Use admission-only stop when new work must be fenced without obtaining a
settlement timestamp, then explicitly drain the same stopped operation later:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --stop-admission OPERATION --history-anchor INPUT --retain-history-anchor STOPPED_ANCHOR
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --drain-stopped OPERATION --history-anchor STOPPED_ANCHOR --retain-history-anchor DRAINED_ANCHOR
```

Both commands accept the same input/output anchor options and historical
`--topology-profile PATH` as combined `--recover-stop`. They support ordinary,
raw-predictive and owned-predictive histories, including topology-mediated
histories. Each command independently verifies the exact recipe, guards, floors,
reserve and retained history before consuming the original sealed recovery.
Neither command loads live reviewer models, reads current policy/topology
evidence, opens peer sockets or returns live authority.

The two-command example above applies directly to histories without topology.
For topology histories, each command also needs `--topology-profile PATH` and
an independently retained `--history-anchor`. A `fa.mediated-submit/2` source
checkpoint binds the exact selected anchor and complete journal extent. Use the
terminal-only `--retain-topology-checkpoint NEW_OUTPUT` option together with
`--retain-history-anchor NEW_OUTPUT` to retain the acknowledged successor pair:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --stop-admission OPERATION --topology-profile ACTIVE_PROFILE --history-anchor ACTIVE_ANCHOR --retain-history-anchor STOPPED_ANCHOR --retain-topology-checkpoint STOPPED_SOURCE
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --drain-stopped OPERATION --topology-profile STOPPED_PROFILE --history-anchor STOPPED_ANCHOR --retain-history-anchor DRAINED_ANCHOR --retain-topology-checkpoint DRAINED_SOURCE
```

`STOPPED_PROFILE` is an independently selected version-2 profile whose
`source_checkpoint` names `STOPPED_SOURCE`; preserve the original `initial`
documents and select an `operation` above the saved last source operation.
Its `next` paths are not opened by terminal recovery. For a later retry, select
the drained pair in the same way. Do not include the live-only profile field
`retain_checkpoint`: the explicit terminal flag owns this output.

The native successor preserves the complete original topology/inventory bytes,
source work, last operation and limits. It takes its exact anchor and floors
from the sealed native acknowledgement, without reopening the journal or
creating a source, observer, reviewer or owner. Only the original matching Stop,
Fence and StopProgress events may follow the saved source cut. A recovery Fence
withdraws historical topology availability; an exact retry of a live Stop that
did not fence topology preserves its actual historical availability. Neither
state is current certification or permission to admit work.

Both outputs must be distinct new private files outside the authority store;
aliases through resolved parent directories are rejected before replay. Both
exports and the original report are attempted independently. A partial export
does not undo the acknowledged stop, settle an unknown charge or authorize a
resend. Keep the previous custody until both outputs succeed.

Terminal mutation and the two exports are not atomic. If source-checkpoint
retention fails after the journal advances, the earlier version-2 checkpoint
is stale for ordinary exact-cut selection. Preserve the previous paired
archives and every successful output. The explicit terminal-checkpoint recovery
mode below can carry that original pair through a verified terminal-only
successor; changing only output paths does not activate it. No profile is
automatically downgraded. A separately selected historical version-1 profile
remains supported with its original requirements, but manual work minima cannot
produce an exact source checkpoint.

`--stop-admission` appends only the original Stop/Fence transaction. Its
`fa.admission-stop/1` report distinguishes `stop_acknowledged: true` from
`drain_attempted: false`; acknowledged admission stop is successful even when
liabilities remain. It never consults the settlement clock. `--drain-stopped`
requires a previously acknowledged stop with the same operation identity and
uses the existing stopped/drained report. An active history or different
operation is refused before clock acquisition. Missing or late time leaves the
earlier stop intact.

The existing `--recover-stop` remains one atomic combined transaction. Failure
does not silently perform admission-only stop. Phase selectors are mutually
exclusive and cannot accompany live creation, continuation, receipt, or Finish
options. The original monotonic runtime budget begins at command entry and
covers input admission, complete replay and terminal clock acquisition; it is
checked both before and after the fallible clock. A completed retry still needs
no clock and adds no event. Synchronous filesystem work is not preemptible.

## Recover after interrupted checkpoint export

If the native stop acknowledged but one or both exports failed, retain the
original version-2 profile and its matching original history archive. Retry
with the same stop operation, new output paths and the explicit
`--recover-terminal-checkpoint` option:

```text
supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --stop-admission OPERATION --topology-profile ORIGINAL_PROFILE --history-anchor ORIGINAL_ANCHOR --recover-terminal-checkpoint --retain-history-anchor REPAIRED_ANCHOR --retain-topology-checkpoint REPAIRED_SOURCE
```

The option also works with `--drain-stopped` and `--recover-stop` in ordinary,
raw-predictive and owned-predictive modes. It accepts the original exact cut for
an initial attempt as well as its permitted terminal successors. A repaired
pair can then be used by an ordinary exact-cut terminal invocation.

The recovery cursor checks the independently retained prefix on the same locked
journal used for complete learned replay. The older source checkpoint and
standalone anchor must still describe exactly the same cut. The original graph,
historical availability, recipe, predictor, policy, guard inventory and terminal
reserve remain required, and every independent floor is preserved or
strengthened. A later cut must already contain the same stop operation; only
its Stop, Fence and StopProgress events may follow the retained prefix. Any
source check, time observation, publication event, equal-counter fork or
different stop refuses without mutation or staging cleanup.

The retained documents, source expenditure and last source operation are never
reconstructed from guesses. Only availability explained by the verified native
terminal suffix can change: Fence withdraws it and Stop alone preserves it.
The restricted cursor can acknowledge admission, drain or combined stop; it
cannot return a live owner, observer, reviewer or receipt-reconciliation path.
The profile's next inputs and live helper/evidence files remain unopened.

A matching admission retry appends no events or clock call. A later drain uses
the original settlement rules, and a completed retry needs neither a clock nor
another event. The selected output pair comes from the sealed native result,
preserving source work and exact native floors even if the mutable journal
changes after acknowledgement. Both exports and the report retain their
independent failure behavior; this option does not make them atomic.

The flag is accepted only by these three terminal modes and requires version-2
source custody. Ordinary live and receipt recovery retain their strict exact-cut
rules. Keep the original pair until replacement custody succeeds, and select
new output files after a collision or partial write. Mixing an older source
checkpoint with a newly exported anchor still refuses. Trusted custody and
protection against rolling back both the journal and its retained records remain
operator assumptions.

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
The explicit operator phases above use these same consumers; there is no
ordinary recovery fallback or second numerical replay.

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

When OUTPUT is selected, ordinary, raw-predictive and owned-predictive modes,
with or without topology, consume the selected native phased or fallible
combined finalizer through the same one-event replay driver, preserving every
admission and Ready checkpoint. The command exports that captured anchor
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
extra journal event or replacement. Without strict version-2 topology custody,
failed retention can be retried with the original INPUT and a different new
OUTPUT. With version-2 topology, the source and anchor must still form a valid
exact pair; follow the partial-export requirements above. An existing or partial
output is never overwritten or silently accepted as acknowledged custody.
A pending drain still requires its original time/capacity and may advance the
next retained revision. Export remains separate from native persistence:
failure cannot undo a stop or refund a charge.

## Results and liabilities

Drain and combined completion use the existing native stop JSON report.
Admission-only completion uses the separate acknowledgment report described
above. `stopped_drained` means the
original stop and drain are confirmed. `stopped_pending` reports the unresolved
and irrecoverable operation IDs and returns a command error. Original executed
effects and expired unknown outcomes remain charged. Only original receipts can
release liabilities; stopping never resends an effect or converts uncertainty
to a successful receipt. A repeated pending drain still needs fresh time and
finite terminal capacity.

If report output fails after the stop is acknowledged, the error says so. Retry
the same operation using valid recovery inputs, including the newly retained
topology/history pair when applicable, to report the verified original stopped
cut. Output failure does not initiate another effect or invalidate an
acknowledged stop.

The operator services its original elapsed budget before opening, before every
original event, once more at Ready, and around terminal clock acquisition.
Cancellation drops the original cursor
without fencing, cleaning staging, obtaining terminal time or returning a live
role. Initial framing, an individual reducer call and final storage work remain
synchronous; the budget is cooperative, not a hard cancellation-latency bound.

This handles one original local reference sink. It does not stop an OS process,
prove remote-provider settlement, authenticate independent anchor custody or
protect a jointly rolled-back journal and anchor.

## Verification status

Four terminal-checkpoint operator regressions in
`learned/tests/topology/terminal/retained.rs` pass using the qualified nightly's
official Cranelift backend and a scoped harness preserving the original test
bodies, assertions, fixtures and production logic. They exercise all three
predictor modes, real sent liabilities, paired admission/drain/retry custody,
unsafe-output refusals, independent export races and report failure. The
complete unchanged operator test source passes the standard compiler check.
Full-harness code generation hit the host memory limit; broader qualification
and the existing dependency-admission gate remain open as recorded in
`IMPLEMENTATION_STATUS.md`.

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

The original anchor implementation and its initial regression wave were
source-reviewed without compilation. Current phase integration and topology
regression execution are recorded in the latest `IMPLEMENTATION_STATUS.md`
entry; those results do not retrospectively qualify earlier unexecuted source
waves. The complete remote RCH/workspace gate and full FA-014 packet remain
open. Source inspection alone is not type checking or runtime qualification.
This implements plan sections 16.4/16.5/16.9/25.4 and FI-A02 without adding an
agent verb, dependency, alternative journal or effect executor.
