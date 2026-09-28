# Paired containment reset for learned generation

`OversightBroker::capture_hosted_learned_checkpoint` and
`reset_hosted_learned` connect the original learned generator's typed KV
checkpoint to the existing containment authority (plan section 11.10, FA-108).
The broker owns both halves. The checkpoint handle exposes neither numerical
state nor effect authority, and no caller-supplied cache, sampler, model,
monitor verdict or replacement source is accepted.

The [actor supervisor and connected/offline driver integration](HOSTED_LEARNED_SUPERVISION.md)
updates queued tickets and active reviews around the same original reset, and
connects learned automatic stops to actual endpoint containment.

## Capture and restore the actual original state

Capture requires a nonempty, active, currently quiet original generation, exact
equality between its tokens/cache/sampler and the controller's actor copy, and
a registered restart grade above `AuditOnly`. The original controller stores
its own checkpoint; the broker privately pairs it with the original
`GenerationKvCheckpoint`. Logical retained numerical state and the controller's
existing state-copy limits both apply. These are not allocator or peak-memory
bounds.

The reset request names the paired handle, expected control sequence, actor
revision and authority epoch, original review binding, retained target ceiling
and a bounded fresh restart allowance. Foreign handles, stale predecessors,
reused rounds, insufficient cache capacity and the original complete-prefix row
limit refuse before restart work. A saved logical timeline ahead of the current
one also refuses. An active checkpoint may be restored from a currently finished
run; a finished checkpoint itself is not a valid paired destination because it
cannot produce the new accepted-token evidence required below.

An admitted reset withdraws the old observation before any numerical work.
It invokes the original checkpoint's complete-prefix learned audit and exact
all-layer KV restorer, retaining the original model, policy, tokens, logits,
sampler snapshot and generation spec. It never installs learned latent values
as a replacement numerical cache. The fresh audit must be completely quiet.
The original policy is unchanged: a checkpoint may be captured even when its
full-prefix audit will exceed that policy's fixed row capacity; reset then
refuses instead of omitting rows or widening the policy.

The staged candidate passes to the original `DeliveryBroker::reset`. That
transition controls incidents, target narrowing, epochs, revocation floors and
cancellation. Only `restored = true` installs the numerical candidate and
synchronizes the new-stream actor copy. Incident-threshold suspension discards
the candidate and leaves the old source failed. An admitted audit, restoration
or authority failure also seals the old owner; it cannot resume from older quiet
evidence or retry unreported work for free. Pure preflight refusals leave the
live source and its costs unchanged.

## Rewinding does not return spent work or rights

Historical counters must describe the exact restored prefix, so they rewind
with its numerical state. They are not lifetime totals. Each restored numerical
and telemetry ceiling is restricted to:

```
saved spend + min(saved remaining allowance, current remaining allowance)
```

This applies to decoder products, vocabulary scores and all eleven aggregate
telemetry counters. Repeated resets, including to a checkpoint from before an
earlier reset, cannot buy back abandoned continuation work. No sampler draw is
charged as committed unless the original token was accepted; attempted sampling
and numerical reservations remain in lifetime work.

`HostedLearnedInspection::cumulative_work` includes abandoned continuations.
`cumulative_telemetry` includes those continuations and completed fresh restart
audits. The existing `work` and `telemetry` fields retain their original active
timeline meaning, including the unchanged journal comparison witness.
`hosted_learned_recovery_usage` separately reports checkpoint storage, restart
attempts, reserved audit/cache allowances, completed audit work and actual
restoration byte/value counts. At most `MAX_CHECKPOINTS` attempts are admitted;
reservations are intersected with the original per-audit policy. Failed or
interrupted attempts do not refund their reservation. Unknown partial work is
not falsely presented as a measured completed audit.

The original authority cancels and refunds only undispatched reservations.
Dispatched or executed effects remain charged, including lost acknowledgments.
Reset does not alter the endpoint or manufacture an outcome. Reconciliation
continues through the original receipt path. Old permits, approvals, sidecars
and input bases cannot authorize the resumed owner. Historical evidence and its
cumulative retention charges remain stored; the required-sidecar policy remains
in force.

## A restored audit is not a new effect observation

The complete-prefix restart audit names the checkpoint's **original** stream.
Its descriptor remains in the typed restart receipt. The resumed generator has
a strictly newer stream and a new process-local observation owner, initially
`Empty`. No old or full-prefix audit is relabeled as its latest-position
evidence. Only a new original accepted token publishes the normal one-position
audit and makes this source `Ready`. A subsequent effect still needs a fresh
proposal, any required original sidecar, congress approval, and the existing
human key when enabled. The old observation handle becomes `Closed` on a
successful restore and unavailable on any admitted failure.

Plain completed-message learned text retains its original tokenizer, prompt and
output policy through this private restore. Cumulative text-stream owners are
excluded: their audience-visible frontier requires a separate protocol that a
numerical rewind cannot satisfy. Automatic-stop incidents are terminal; this
API never clears or replaces them. A held run without an installed automatic
stop may rewind to an earlier quiet checkpoint, but replaying the same next
token must still pass the unchanged original monitor.

## Verification and remaining scope

Thirteen regression functions exercise actual stochastic continuation, fresh
source/sidecar/two-key publication, repeated vocabulary and telemetry exhaustion,
abandoned numerical products, already-published unknown effects, incident
escalation, free stale/foreign refusals, admitted audit failure, fixed automatic
stop, text-stream exclusion, repeated genuine alarms, and an active checkpoint
restored from a completed run. They also cover exact plain-text regeneration and
renewed two-key publication, plus free complete-prefix row-cap refusal with a
near-identical successful shorter checkpoint. A compile-fail example rejects using a checkpoint
as a permit. These are authored source tests, not an executed qualification.
The required remote RCH check cannot launch in this environment; compilation,
tests, rustfmt and Clippy remain **UNEXECUTED**.

This is an in-memory reference composition. It adds no durable reset event,
cross-process recovery, secret restoration guarantee, empirical detector claim,
or independently attested restart grade. The original reset request has no
incident-observation frontier: selection of a checkpoint that predates the
incident remains the trusted supervisor's responsibility. Successful numerical
restoration does not close the wider FA-108 admission and qualification work.
