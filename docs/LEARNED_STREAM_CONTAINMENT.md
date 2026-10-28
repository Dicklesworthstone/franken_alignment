# Learned stream checkpoints within a confirmed publication boundary

## Capability and scope

Plain cumulative learned text streams can capture a quiet active numerical
prefix and restore it within the same exact receipt-confirmed audience boundary.
This connects the original learned KV/sampler checkpoint to the original
containment controller and durable oversight journal. It serves plan section
11.10, FA-108, and founding synthesis FS-02 (FI-A09, FI-A16 and FI-I03).

For example, after message A is confirmed, the original finite generation may
start message B and capture a checkpoint after accepting a token of B. A reset
within that same message can restore the original numerical prefix while A,
its target version, publication receipt and spent rights remain unchanged. The
original text output offset is retained, so the next message is B rather than a
second copy of A. New accepted-token evidence and ordinary approvals are still
required before B can be published.

This path accepts no installed action-consistency predictor, including optional
predictors. Predictor rearming requires a separate contract. It also does not
supply an operator command, authenticate the supervisor's incident declaration,
erase knowledge already exposed to an audience, attest a restart grade, or
establish the H17 secrecy/repetition hypothesis. The complete FA-108 prerequisite
and execution requirements remain open.

## Native and durable APIs

The native broker exposes `capture_hosted_learned_stream_checkpoint` and
`reset_hosted_learned_stream`, with the sealed
`HostedLearnedStreamCheckpointHandle` and `HostedLearnedStreamResetRequest`.
The durable `FileOversight` owner exposes:

| Operation | Purpose |
| --- | --- |
| `capture_learned_stream_checkpoint(revision, checkpoint, actor_revision, authority_epoch)` | Capture and acknowledge the original quiet active prefix together with its exact confirmed audience basis. |
| `learned_stream_checkpoint(checkpoint)` | Reacquire a historical `FileLearnedStreamCheckpoint` under the current owner brand. |
| `check_learned_stream_checkpoint(&checkpoint)` | Check historical identity and owner, without claiming present reset eligibility. |
| `begin_learned_stream_reset(revision, &checkpoint, control, budget, first_incident_sequence)` | Persist the fixed stream reset instruction before any new audit or restoration. |
| `complete_learned_stream_reset(revision, operation)` | Complete only that unchanged, uninterrupted stream intent through original audited reset and persistence. |
| `reset_learned_stream_checkpoint(revision, &checkpoint, control, budget, first_incident_sequence)` | Combine the write-ahead intent and completion. |

`control` is the original `FileResetRequest`; `budget` is the original
`KvRestartBudget`. Completion returns the existing nested result:
`Result<Result<HostedLearnedResetReceipt, Error>, JournalError>`. An outer error
acknowledges no completion. An inner error is an acknowledged original refusal
or failure, including its actual retained costs and containment state.

The file handle exposes only `id()` and `info()`. It contains no numerical
state, actor, source, reviewer or effect rights. Old handles must be reacquired
after recovery. Stream and legacy checkpoint handles are distinct types. Legacy
capture/reset, cooperative preparation and `finish_pending_reset` cannot execute
stream operations. The separate `finish_pending_stream_reset` path below retains
the stream kind and independently selected incident frontier. Generic immutable
reset-result inspection can read either kind without obtaining execution authority.

## Exact audience and incident boundaries

Capture and reset bind the original target, stream profile, complete message
boundaries, exact confirmed bytes, continuation mode and text output offset.
Matching concatenated text is insufficient. The durable machine additionally
requires the actual endpoint view to equal the broker's confirmed view.

Pending sends, Unknown deliveries, published-but-unconfirmed output, a changed
confirmed boundary, Finished streams and changed target versions refuse. For a
later message, its original fixed request slot must remain unconsumed: resetting
cannot replace an already recorded request with a new proposal under the same
identity. A checkpoint immediately after a message-continuation transition also
remains unavailable until the original generator can produce a valid active
checkpoint, normally after accepting a new token.

The explicit incident parameter must satisfy:

```text
checkpoint.control_sequence < first_incident_sequence <= current.control_sequence
```

This is a supervisor-declared ordered control frontier. It is not authenticated
evidence of the incident's first occurrence. Numerical token steps alone do not
necessarily advance the control sequence, so an incident at the capture's same
control sequence cannot qualify. The declaration is fixed by the write-ahead
intent, its completion witness and exact operation retries.

## Original state, accounting and authority

The shared original reset engine performs complete-prefix learned auditing,
exact KV/sampler restoration, remaining-budget conservation, target narrowing,
authority-epoch advance, cancellation of undispatched work, stale-key withdrawal
and out-of-band incident escalation. Stream metadata shares the original bounded
checkpoint inventory and receives its own retained-state charge.

Historical numerical counters rewind with the restored prefix. Lifetime
numerical and telemetry work, including abandoned continuations and fresh reset
audits, do not rewind. Audit reservations and failed work are not refunded. The
external effect ledger, charged deliveries, confirmed publication and current
policy remain original history. Repeated resets reach the original suspension
threshold even when the restored actor state looks unchanged.

Restored source evidence starts `Empty`. The complete-prefix restart audit keeps
its original source identity and cannot become a fresh effect observation. Only
a new original accepted token can establish the next observation; the resulting
proposal still needs congress and both configured publication keys. Recovery
preserves numerical pause until an independently authorized explicit resume.

## Persistence, retries and compatibility

A new reset needs two ordinary journal event slots. Terminal recovery reserve
cannot pay for capture, intent or completion. Byte limits remain independent of
event capacity. Completion poisons the live file owner before fallible new
audit/restoration/encoding and uses the original candidate replacement boundary.
Storage failure never releases a successful result from an unacknowledged image.

Exact completed-operation retries return the original result without another
audit, incident, restored owner or journal replacement, including after ordinary
reopening and handle reacquisition. Changed checkpoint, instruction, budget or
incident frontier refuses. An interrupted intent remains a barrier after the
recovery Fence; neither a fresh clock nor a newer expected epoch can complete
that original ambiguous instruction. Existing settlement and terminal
containment remain available.

## Explicit recovery before the Fence

An acknowledged `BeginStreamReset` can survive a process failure without its
completion. The supervisor can deliberately complete that exact instruction at
its original predecessor, before any recovery Fence, with:

```rust
recovery.finish_pending_stream_reset(&expected_intent, first_incident_sequence)
```

The consuming finalizer is available on the original `FileLearnedRecovery`,
`FileGuardedLearnedRecovery` and plain `FileMediatedLearnedRecovery`. Use the
appropriate original `begin_open_*` constructor and advance its numerical replay
to `Ready`. Independently retain the checkpoint ID, `FileResetRequest` and
`KvRestartBudget` as `FileLearnedResetIntent::for_recovery`, plus the exact declared
incident frontier. Constructing this expectation neither creates an intent nor
selects a checkpoint on the caller's behalf.

Before a new audit, the finalizer requires the full original intent, stream kind,
incident frontier, control/actor/authority predecessor, uninterrupted status and
confirmed audience boundary to match. It also checks ordinary completion event
capacity, separate Fence capacity and the exact canonical bytes under the same
exclusive lock. The original stream reset performs the audit, restoration,
remaining-budget intersection and incident escalation. Its success or actual
failure becomes the original completion witness. The encoder checks every
prefix, including the completion's ordinary byte allowance; terminal reserve
cannot pay for the completion merely because a Fence follows it.

The original `finish` rechecks the same canonical input and publishes completion
and Fence in one acknowledged replacement. No intermediate reset-only owner,
reviewer or role bundle escapes. The guarded variants check their full independent
requirements both before and after private preparation, including effective
policy, credentials and history floors; mediation also retains its topology and
source contract. They provision only the original fresh role types after the
replacement succeeds. This adds no predictive reset path or weaker guard fallback.

After success, inspect `learned_reset_result(operation)` for the recorded inner
outcome. The recovered owner is paused and lacks fresh time. A successful
restoration starts with Empty source evidence; independently authorized time and
resume, a new accepted token and the original publication approvals remain
necessary. A failure remains a failure with its admitted work retained.

After an ambiguous write, the exact already-completed intent and incident
frontier receive only ordinary Fence recovery. No second audit, incident or
refund occurs. A different pending instruction takes precedence and refuses an
attempt to recover an older completed command. Incomplete or failed replay,
missing intents, changed commands/frontiers, old checkpoint kinds and a prior
Fence or Stop return no owner. Ordinary `finish` and synchronous open still
quarantine unresolved intents; this explicit method cannot rebase one into the
new epoch.

The cursor is consumed on failure or unwind. Failed storage can expose the old
pending image or the complete new image, never an acknowledged intermediate
reset result. Physical replay and unacknowledged audit work can recur after a
process failure; logical retained charges are not a cross-process CPU escrow.
Storage and an individual original numerical event remain synchronous.

See [pending learned reset recovery](PENDING_LEARNED_RESET_RECOVERY.md) for the
shared composition, independent custody and retry contract.

## Wire compatibility

Legacy event tags 0/1/2 and legacy witness layouts remain unchanged. The explicit
new forms use tags 3 (`StreamCapture`), 4 (`BeginStreamReset`) and 5
(`StreamReset`). New capture/reset witness domains are `FALSCP\0\x01` and
`FALSRS\0\x01`. The reset witness contains the unchanged original reset witness
plus the exact stream basis and incident declaration. Replay executes original
capture, audit and reset operations and compares the resulting witnesses; it
does not import stored tensors or a claimed monitor verdict as live state.

## Authored verification and limits

Eight durable integration functions exercise real finite-model histories:
original restoration with fresh two-key approval; second-message offset and
spent-rights preservation; every audience-boundary refusal; owner/kind/frontier
checks; retries and reopening; interrupted intent; consumed continuation request;
and original incident-threshold suspension.

Eight private regression functions add capture, intent and completion failures
at all five original storage barriers; original/new wire forms and truncation;
corrupt capture/reset witnesses and changed otherwise valid incident inputs;
legacy recovery/completion separation; exact and insufficient event/byte limits;
terminal reserve preservation; installed predictor refusal; and acknowledged
failed audits without work refunds. Two compile-fail examples prohibit treating
a stream checkpoint as either a permit or a legacy checkpoint. Existing legacy
test assertions remain unchanged.

Nineteen additional runtime regression functions exercise the original completion
and Fence transaction, every partial replay cut, exact completed retries,
unrelated pending commands, guarded policy/credential/history requirements,
fresh-role continuation, all five storage barriers, canonical substitution before
and after preparation, exact ordinary event/byte limits, preserved audit failures
and a consumed candidate on unwind. The mediated positive uses actual original
file-source reads, identity and campaign guards, an original generated proposal
and independently approved native policy promotion. After recovery, only the
fresh observer can install a newer graph and certify a new source cut before
resumed generation. A further compile-fail example prohibits reusing the consumed
recovery cursor. These are authored cases pending remote execution.

Independent source reviews found and corrected a child-method visibility error,
a test-only codec ceiling that was too small for native witnesses, and a mediated
fixture that incorrectly treated an authority-epoch advance as a control-sequence
advance. The corrected fixture explicitly checks the distinction and uses actual
native policy promotion to order its incident frontier. The initial fifteen
changed Rust files and the nine recovery-increment Rust files pass syntax
parsing. These are authored tests, not an executed pass count. The latest required
targeted and canonical commands both exited 127 because `rch` is absent:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib --test learned_text_stream_durable --test mediated_learned_receipts
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Compilation, Rust tests, Clippy and canonical qualification remain unexecuted.
The scoped Bead remains in progress. Synthetic fixtures and source inspection do
not establish empirical restart fidelity, trained-detector quality, production
readiness, secret restoration or safety on another host/backend.

See [durable original learned reset](DURABLE_LEARNED_RESET.md) and
[paired native reset](HOSTED_LEARNED_RESET.md) for the original shared machinery.
