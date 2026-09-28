# Durable original learned checkpoints and containment

The existing `FileOversight` journal now records captures of the broker's
original paired learned KV/sampler and authority checkpoints. This serves the
rewind-as-containment work in plan section 11.10 and FA-108. It adds no numerical
engine, authority ledger, model importer or effect-approval bypass.

## Original capture, exact replay, branded handles

`capture_learned_checkpoint` requires the live original quiet, nonempty, active
generator, its expected actor revision and authority epoch, a fresh clock, and
no unresolved token intent. The broker retains its original restart-grade,
stream-owner, state-equality and storage limits. Capture is persisted through
the original candidate/journal replacement boundary before returning a handle.

The record binds exact original tokens, KV words, logits, samples, RNG encoding,
actor copy, work and audit state. The additional checkpoint witness binds all
remaining continuation ceilings, cumulative abandoned-work counters, original
recovery costs, incident count and authority sequence/epoch. Saved values are
comparison material only. Replay executes the original generation and capture;
it never loads archive tensors or a claimed quiet result as live state.

`learned_checkpoint` reacquires historical identity from an acknowledged owner.
`check_learned_checkpoint` checks the owner brand and original capture, not effect
eligibility. Exact capture retries return the old cut without a new write.
Foreign and pre-recovery handles do not belong to the recovered owner, even
when their numeric IDs and metadata match. Inventory and logical state bounds
remain enforced. A checkpoint is never a permit.

The original configuration binding, recovery fence, explicit numerical resume,
sidecar requirement and both effect keys are unchanged. The original token
witness layout is retained; checkpoint events have a distinct new tag. Reopening
continues to require the independently supplied exact `FileLearnedConfig`.

## Verification boundary

Seven authored regression functions cover original capture/reconstruction,
historical retries, foreign/recovered brands, pending-token and pause barriers,
corrupt comparison bytes in otherwise valid journal framing, and the exact
checkpoint-count boundary, original two-key publication, and failures at all
five canonical storage barriers. The existing numerical tests and fixtures are reused
without changing their assertions. These synthetic controls do not qualify a
trained detector or registered restart grade.

Compilation, Rust tests, rustfmt and Clippy are UNEXECUTED: the required remote
RCH launcher is unavailable in this preparation environment. Source comparisons
are not a substitute for a successful remote build. The capture increment is
followed by the write-ahead reset path below; neither has runtime qualification.

## Write-ahead original reset

`begin_learned_reset` records a fixed checkpoint identity, original
`FileResetRequest` and `KvRestartBudget` before any new audit or restoration.
It checks the actual control sequence, actor revision, authority epoch, existing
checkpoint and lack of a pending token/reset. Two journal event slots must fit.
Audit requests do not widen the original policy: every dimension is intersected
with the original learned policy by the original restarter.

`complete_learned_reset` executes `reset_hosted_learned` on the original replayed
broker, then persists its result through the same canonical replacement as token
completion. `reset_learned_checkpoint` combines these two operations. A new reset
advances the journal twice. Outer `Err` returns no acknowledged candidate result;
inner `Err` is an acknowledged original refusal or failure. A pure original
preflight refusal may consume journal records but does not pretend to have run
an audit. An admitted original failure retains its actual reported recovery
costs and failed numerical owner.

The completion witness compares the exact original instruction, restored state,
remaining numerical/telemetry ceilings, cumulative abandoned work, recovery
reservations and completed work, original control receipt and fresh full-prefix
restart audit. The original source descriptor remains attached to that audit;
it is not relabeled as a new-stream observation. The resumed source starts Empty
and cannot admit a proposal until a new original token passes its monitor.
Original checkpoint-count and state limits remain; completed operation records
are additionally bounded by `MAX_CHECKPOINTS` and aggregate logical witness bytes
by `MAX_WITNESS_BYTES`. These are not peak-memory or allocator limits. Exhausting
completion capacity after computation leaves the intent barrier rather than a
usable older state.

`learned_reset_result` retrieves an acknowledged historical result. An exact
operation retry does not write, re-audit, add an incident or issue another key.
Changed arguments for that operation refuse. After recovery, checkpoint handles
must be reacquired; the old handle cannot transfer its owner brand. A completed
reset is rebuilt by executing its original capture/audit/restore/control events,
not by deserializing its cached result as authority. Normal whole-history replay
still physically recomputes historical work; the retained counters describe
logical operations, not total CPU work across replays.

## Interrupted intent versus a completed reset

`pending_learned_reset` exposes the immutable instruction and whether a stop or
recovery fence interrupted it. A pending reset blocks new generation, captures,
reviews, human approval, dispatch and final positive publication in both live
admission and semantic replay. It does not refund existing dispatched work.
Original cancellation, stop, time observation, sealing and reconciliation remain
available. Source-bound learned workers also check the pending-reset barrier at
their existing current-input gate before their next numerical or I/O quantum;
previous disclosure and work are not recalled or refunded. Additional evidence-management operations do not clear the barrier.

An intent without an acknowledged completion **cannot resume after recovery**.
Recovery advances the authority fence and marks that original intent interrupted.
Neither explicit generation resume, an old completion call, a replacement reset,
nor rewritten expected epochs can abandon it. Its fixed allowance and independently
bound original policy remain the bound on possibly executed partial work; no
measured completion or incident is invented. Settlement and terminal containment
are available, but continuation is deliberately unavailable for this ambiguous
case. This increment does not implement an intent-abort/refund or a restart of
interrupted numerical reset work.

If canonical replacement reached rename but its directory-sync acknowledgment
was lost, recovery may instead find the completed reset. Its original incident,
state, costs and result are then rebuilt, followed by the ordinary recovery
fence and numerical pause. No additional reset incident is generated. A NEW
supervisor instruction may also reset a recovered held run that has no pending
reset; it names the fresh control predecessor and still leaves generation paused
until explicit resume. Installed automatic stops and incident-threshold suspension
remain terminal. Cumulative audience-stream owners remain excluded.

## Effects and regression sources

Only the original reset authority decides cancellation, narrowing and refunds.
Successful or admitted failed reset withdraws stale human keys and active reviews
without fabricating endpoint outcomes. Unknown and executed effects remain
charged. Original receipt/seal reconciliation works after recovery without a
live helper input or an old human key. No old approval becomes authority for the
resumed source, and a fresh quiet token still needs congress and both configured
publication keys.

Fourteen additional regression functions cover exact stochastic continuation and
reconstruction, operation retry and foreign handles, post-reset two-key
publication, old-key invalidation, executed/unknown liabilities, repeated
sampling and telemetry exhaustion across reopen, incident escalation,
interrupted-intent blocking with continued settlement, all five intent and
completion storage faults, original failed audits, recovered holds versus fixed automatic
stops, capacity/stale preflight, and corrupt witnesses/changed intent/missing
write-ahead records. A real-socket control also checks that an already-started
source-bound learned helper refuses before first disclosure when reset is pending.
The original seven capture test bodies remain unchanged.
All twenty-one functions are authored source tests, not an executed test count.
RCH compilation, tests, rustfmt and Clippy remain UNEXECUTED in this environment.

This extends the in-process and durable reference composition, not empirical
restart-grade attestation, incident-frontier authentication, cross-machine
numerical fidelity, journal authenticity, trained-detector quality or production
release qualification. The existing operator/local storage trust boundaries
and synchronous whole-history replay costs remain unchanged.
