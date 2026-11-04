# Owned K/V forecasts after a prompt-boundary reset

An explicitly selected native learned-text recipe can now restore a checkpoint
at the **complete original prompt boundary**, forecast from a fresh audit of the
actual restored cache, and generate its first sampled token afterward. This
closes the ordering gap where a restored owner had no current accepted event,
the predictor required that event, and the next token required a forecast first.

There are separate immutable recipes for native single-message learned text and
cumulative text streams. Both accept only the complete original prompt before
**any** sampled work. The stream recipe retains the original exact confirmed
audience boundary and can continue through independently forecast messages after
restoration. Resets after sampling, including later-message resets, remain
unsupported. There is no new calibration claim, role, or effect shortcut.

## Independent recipe selection

Build the original text recipe and mandatory sidecar, pin its owned K/V
pre-output predictor, then select both source-successor and fresh-audit behavior:

```rust,ignore
let generation = FileLearnedConfig::new_text(model, tokenizer, source, limits)?
    .with_required_sidecar()?
    .with_required_owned_pre_output_forecast(predictor)?
    .with_forecast_reset_successors()?
    .with_owned_forecast_restart_audit()?;
```

The final wrapper binds `FALKRFA\x01` and the complete preceding recipe bytes.
Recovery must supply the exact independently retained recipe and predictor.
Omitting the wrapper, switching to raw residual prediction, or changing its
order cannot reinterpret an existing history. Every legacy recipe retains its
original bytes, one-audit reset, and accepted-token source requirements.

The profile accepts a quiet checkpoint only at the complete original prompt,
with zero sampled work and a `functional_restart` or stronger registered host
profile. The existing paired checkpoint API remains unchanged. The original
control predecessor, actor revision, authority epoch, retained target ceiling,
review binding, incident counter, and restart budget still govern restoration.

## Cumulative stream selection and original custody

For a cumulative stream, pin the original owned predictor with its exact stream
message profile, then select the separate stream wrapper:

```rust,ignore
let generation = FileLearnedConfig::new_text_stream(model, tokenizer, source, limits, stream)?
    .with_required_sidecar()?
    .with_required_owned_pre_output_forecast(predictor)?
    .with_forecast_text_stream_continuation()?
    .with_owned_forecast_stream_restart_audit()?;
```

Per-message continuation is optional; the stream wrapper also supports an
original one-message stream and its separately reviewed finish. `FALKRFS\x01`
binds the entire preceding recipe and atomically selects source-successor and
fresh-audit behavior. `requires_owned_forecast_stream_restart_audit()` reports
this immutable choice. The original nonstream wrappers still refuse streams;
raw, optional, and legacy stream predictors remain ineligible. Recovery cannot
add the new selection to an old journal or omit it from a selected one.

The existing branded stream checkpoint binds the original target, stream
profile, exact confirmed message boundaries, output offset, continuation mode,
and capture control sequence. The endpoint's published state must equal the
confirmed view; a pending or unknown send cannot be rewound. The selected
first-incident sequence must be strictly later than capture and no later than
the current control sequence. It remains a supervisor declaration, not an
authenticated detector. A real pre-output containment operation now supplies
that later control cut while retaining the pinned policy generation.

`FileLearnedStreamNarrowRequest::new(revision, checkpoint_info, control)` binds
the entire original capture and exact current journal revision, control
sequence, actor revision, authority epoch, operation, review binding and retained
target identities. `FileOversight::narrow_learned_stream_checkpoint(&request)`
uses the original paired checkpoint and containment gate. Its immutable
`CheckpointNarrowReceipt::sequence` identifies the actual later incident
frontier. The gate intersects the **current** ceiling, revokes the authority
epoch and old keys, and cancels all original undispatched attempts. Only
pre-dispatch reservations are refunded; dispatched, Unknown and spent effects
retain their original accounting. Actor state, policy and predictor generations,
numerical and audit work, statistical coverage and the reset incident counter
remain unchanged. A later original reset alone increments that incident count.

This API admits only the separately selected owned stream prompt profile with
the same complete unsampled original prompt and exact confirmed audience. It
refuses pending work or forecasts, lost coverage and any prior sampled work.
A mediated owner also loses its accepted topology cut at the original narrowing
transition; graph availability and source expenditure remain unchanged, and the
original checker must certify the graph at the new authority epoch before an
effect can be admitted. Retained targets must name the same adapter, object,
contract and generation. Endpoint version is part of target identity: a planned
two-message stream and finish must explicitly retain all three required versions
in both narrowing and reset requests. An empty selection contains all future
effects; neither a larger later request nor reset can reopen it. Policy
replacement is not a substitute for this operation, because the original
predictor remains bound to its selected policy generation.

The new event uses its own tag and `FALSNW\0\x01` comparison witness. Both journal
encoding and decoding check its exact prefix index; semantic replay recomputes
the original consequence, full request, numerical state and stream basis.
No general public authority setter or actor-facing role is added. Supervisor
custody remains the trusted native reference profile; this does not qualify
production `PermitCx` authentication.

`FileOwnedPredictiveRecovery` keeps both independent recipes, all guards,
current policy, credential epoch, recovery floors and retained reserve checks
while composing four original operations:

- `finish_captured_stream_checkpoint` records the exact selected native capture.
- `finish_narrowed_stream_checkpoint` records the original gate's bounded
  containment consequence and later control frontier.
- `finish_begun_stream_reset` records only the exact selected reset intent.
- `finish_pending_stream_reset` completes that intent and the ordinary recovery
  fence in one canonical replacement, returning a paused owner and fresh roles.

The first three return only immutable receipts and anchors. They produce no live
owner, forecast, fence, inference, or source substitution. Exact capture,
narrowing and begin retries append zero events, including historical retries after successful
publication. Completion retries retain the original audits and incident count
and add only the usual recovery fence. Changed or interrupted instructions
cannot be rebased. Ordinary capture/narrowing/intent/completion events cannot spend the
terminal recovery reserve.

The stream reset comparison retains `FALSRS\0\x01` around the version-two native
reset witness, binding both fresh audits and the complete original audience
basis. The retained predictor's observation sequence follows the actual new
source stream. Its jobs, numerical work, retained inventory and likelihood
process are never replaced. After the first new draw clears the private restart
audit, subsequent message and finish forecasts use their ordinary newly accepted
source evidence and exact receipt-bound continuation requests.

## Complete topology custody through native stream control

The explicit owned stream profile also composes with the original
`FilePredictiveMediatedLearnedRecovery`. Before event zero,
`require_stream_control_checkpoint(saved, request)` seals that cursor to one
`FileMediatedStreamControlRequest`: `Capture`, `Narrow`, or `BeginReset` with its
exact intent and incident frontier. The complete independent predictor,
mediation, guard, policy, credential, floor and terminal-reserve requirements
remain mandatory. A plain recovery cursor cannot discard installed mediation.

`finish_stream_control` returns an immutable `FileMediatedStreamControlHistory`
containing both the original control result and a `TopologyCheckpoint` at the
same acknowledged cut. It checks all guards and source custody on both sides of
private preparation before the sole canonical replacement. Source documents,
graph, spent work and last source operation remain the trusted saved values;
there is no source read, new allowance, forecast, Fence or live role. An old pair
can explain only an unchanged exact cut or the single exact selected control
event. An unrelated numerical, time, source, campaign or control suffix refuses.
Exact acknowledged retries return the original operation without another event.

The same predictive cursor's `require_stream_reset_checkpoint` selects original
completion and its reset-plus-Fence transaction. The existing stopped-reset
selector and checked Stop handoff retain priority. Lost completion export can
be recovered only through the original matching reset/Fence suffix. Completion
retries preserve both audit charges, statistical evidence and incident count.
The paused owner must obtain fresh time, source qualification, topology
certification and its original owned forecast before ordinary reviewed output.

Topology availability means that the original graph remains available; it is
not an accepted cut. A narrowing clears the broker's actual accepted cut while
leaving `last_check` as historical evidence, and advances no extra authority
epoch. Neither an old cut nor old reviewer roles can authorize the successor.
The checkpoint archive requires independent trusted custody. Its framing does
not authenticate a coherently forged work history or establish physical
containment of undeclared operating-system paths.

## Fresh observation without a sampled-token dependency

The original checkpoint reset first audits the retained checkpoint cache under
its original stream and restores the exact numerical cache and sampler. The
new profile then performs a **second** compression, source check, and complete
all-layer K/V monitor pass over the restored owner's actual cache. That cache
names the successor stream. A saved observation is never relabeled as evidence
of the new stream, and the earlier accepted generation event is not restored.

The new audit is available only inside the original owned K/V forecast path.
The ordinary `LearnedObservation` remains `Empty` until a newly accepted token
publishes its usual current evidence. The forecast uses the last row of the
fresh complete prefix, fixed by the original predictor's layer and K/V side.
Its source includes the complete retained audit inventory, so the usual
prediction job, numerical work, retained-source bytes, age, actor cut, and
sequence checks all apply. The first actual generation step clears the private
restart-audit source before computation, including on failure or unwind.

After a fresh forecast, ordinary sampled generation, mandatory sidecar review,
native helper commitment/reveal, human approval, one-use authorization,
dispatch, publication validation, and reconciliation still run. A reset receipt
or fresh forecast alone cannot publish anything.

## Costs, failures, and statistical boundaries

Both audits use the request's per-audit allowance intersected with the original
monitor policy. The reset reserves **two** such allowances before either audit
runs. Failed attempts retain both reservations; completed reports contribute
their actual costs to the original `HostedLearnedRecoveryUsage` totals. Restored
cache bytes and original generation budgets keep their existing accounting.
No prompt inference or random draw is repeated by the new audit.

`HostedLearnedResetReceipt::restored_forecast_audit` retains the second report.
For this recipe only, the reset comparison witness uses `FALRST\0\x02` and binds
both complete audit reports and their actual source descriptors. The ordinary
reset witness remains version 1. Original semantic replay recomputes both
audits and compares them; no archived report supplies a live source.

Pending forecasts, lost coverage, crossed likelihood evidence, unreported
prediction work, and **any cumulative sampling** refuse this reset path before
new reset work. Predictor lifetime counters and the likelihood process never
rewind. In particular, restoring the same cache and RNG after observed output
could reproduce the same event; this profile does not count it again or hide it
behind a neutral likelihood factor. Post-sampling reset support requires a
separate, complete same-origin statistical contract.

The existing write-ahead intent, canonical replacement, recovery fence,
independent guard checks, role custody, and exact operation retry remain in
force. A completed retry does not run either audit again or add another
incident. Recovery still requires fresh time and explicit original resume.

## Verification scope

The regression sources in `tests/forecast_reset_successors/restart_audit.rs`
exercise actual prompt capture, reset recovery, successor-cache audits,
pre-sampling owned prediction, original sampled output, native model review,
human approval, publication, and independent journal replay. Causal controls
retain the legacy prompt-boundary refusal, reject missing/raw recipe selection,
reject pending forecasts and already answered sampled history, restrict capture
to the original prompt boundary, and retain both reservations on audit failure.

The cumulative-stream regressions in
`tests/learned_text_stream_durable/continuation/forecast/owned_native/reset.rs`
exercise actual prompt capture, real pre-output narrowing, interrupted-reset completion, fresh owned
forecasting, two original numerical messages, native helper reviews, independent
human keys, explicit finish and independent replay. Additional controls cover
offline capture/narrowing/initiation and zero-event retry after publication, exact
guard/policy/predictor inputs, legacy refusals, incomplete prompt capture,
pending and answered sampled history, exhausted prediction acquisition, and
failed audit charges. The permit checks and audience history remain original
product behavior; the finite model and neutral table are test controls.

The gate regressions in `gate/containment/narrow/tests.rs` check original
Reserved/Unknown/spent conservation, exact checkpoint actor/profile and
predecessor binding, monotone intersection through subsequent reset, and
overflow/capacity refusal before mutation. The owned durable regressions in
`guarded/predictive/owned/tests/stream_narrow.rs` cover live and offline narrowing
through all five original Store failure barriers, wrong capture/current cuts
and audiences, changed retries, zero-versus-ample ordinary byte/event capacity,
journal-index and witness tampering, and actual topology-cut withdrawal and
recertification with weaker plain-recovery refusal.

Four new regressions in `tests/mediated_predictive_learned_recovery/stream_control.rs`
exercise the complete paired lifecycle through actual native and human reviewed
publication, a matched bypass graph at the same epoch, original source-work
conservation, exact lost-export retries, independent guards, terminal reserve,
event-zero selection, changed requests and unrelated suffixes, and original
canonical/staging failure behavior. Two compile-fail examples protect the new
sealed cursor from live extraction. These remain authored source regressions.

These are source regression additions pending the repository's required RCH
gate. No Rust compilation, test execution, conditional-calibration validation,
restart-grade qualification, deployment containment, or reset-secrecy claim is
made by this change.
