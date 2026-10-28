# Explicit abandonment after learned recovery

The original cooperative learned recovery cursors can now complete an explicit
`Unknown -> IrrecoverablyUnknown` disposition without recovering a live owner.
This is a durable statement that the existing charged obligation will remain
unresolved. It is not evidence of nonexecution, a refund, a cancellation receipt,
or permission to send the effect again.

The disposition applies only after the native attempt has actually entered
`Unknown`. An actor-facing unknown observation of a still-`Dispatching` attempt
is not enough. Recovery does not silently fence a dispatch to make it eligible.
See [durable unknown abandonment](DURABLE_UNKNOWN_ABANDONMENT.md) for the original
request, receipt, reducer, and storage contract and the operator entry points.

## Native recovery API

These six existing recovery types expose both finalizers:

| Recovery type | Original independent recovery requirements |
| --- | --- |
| `FileLearnedRecovery` | Exact learned recipe and complete original numerical replay |
| `FileGuardedLearnedRecovery` | Recipe, complete guard inventory, effective policy, credential epoch, and counter floors |
| `FilePredictiveLearnedRecovery` | Guarded learned requirements plus the required raw predictor and evaluator inventory |
| `FileOwnedPredictiveRecovery` | Guarded learned requirements plus the owned K/V predictor, representation, and work limits |
| `FileMediatedLearnedRecovery` | Guarded learned requirements plus original/current topology and availability |
| `FilePredictiveMediatedLearnedRecovery` | Mediated requirements plus the selected raw or owned predictor and evaluator inventory |

```rust,ignore
// Supply the same independently retained profile, requirements, recipe and
// history anchor used for ordinary guarded learned recovery.
let mut recovery = FileOversight::begin_open_guarded_anchored_with_learned_generation(
    directory, profile, &requirements, &recipe, &anchor,
)?;
while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
    let completed = recovery.progress().replayed_events;
    recovery.advance(completed, 1)?;
}
let history = recovery.finish_abandoned_with_anchor(request)?;
// Independently retain history.history_anchor(); inspect history.receipt()
// and history.snapshot(). This result contains no live owner or guard roles.
```

`request` is the original `FileAbandonRequest`: a nonzero operation ID, the exact
attempt, and its expected control sequence and authority epoch. Complete replay
must qualify before the finalizer can proceed. The guarded types retain all
their original checks; the new finalizer does not substitute a base recovery
cursor or infer requirements from the history it is checking.

The finalizer rechecks the locked canonical bytes and uses the same persistence
engine as original guarded abandonment. A new disposition appends exactly one
original `AbandonUnknown` event through **ordinary** journal capacity. It adds no
recovery Fence, clock observation, endpoint lookup/send, numerical resume/reset,
forecast, or source refresh. It preserves the charged units, execution history,
and both published and receipt-confirmed stream cuts. An abandoned message does
not make a finish or another stream successor eligible.

Exact operation retries return the original receipt and current acknowledged
history without appending another event, including when ordinary capacity is
exhausted. A new disposition cannot consume the terminal recovery reserve.
Conflicting operations, stale predecessors, missing/non-Unknown attempts,
incomplete or failed numerical replay, and changed canonical bytes refuse.
Contract refusals occur before staging cleanup. Storage failures expose no
candidate receipt or anchor, even when a replacement might already be visible;
reopen and retry the exact original operation to resolve that ambiguity.

## Retaining source custody

For an independently retained topology checkpoint, use the consuming
`finish_abandoned_with_checkpoint(request, &saved)` finalizer. Its preflight
accepts only the exact saved cut or that cut followed by **one matching original
`AbandonUnknown` event**. It rejects unrelated work, including Fence, Time, source
changes, and other abandonment operations, before cleanup or mutation.

After success, derive the paired checkpoint from the immutable acknowledged
result:

```rust,ignore
let history = recovery.finish_abandoned_with_checkpoint(request, &saved)?;
let successor = saved.after_abandoned_history(&profile, canonical_store_path, &history)?;
// Retain both history.history_anchor() and successor through independent custody.
```

The successor preserves the original topology and inventory documents, graph,
availability, spent source work, and last source operation. Active availability
stays active; withdrawn availability stays withdrawn. This derivation performs
no new source read or semantic replay and grants no freshness or authority.

Archive retention is separate from the durable disposition. If export fails,
report that failure without repeating the effect or undoing its charge. The old
exact anchor/checkpoint pair can retry across the single acknowledged event;
an already retained successor pair can retry at that same cut. Additional
history cannot borrow an old checkpoint's source expenditure.

## Regression controls

`crates/fa-reference/tests/learned_unknown_abandonment.rs` exercises original
native generation, native committee review, separate human authorization,
credential-bound publication, and learned recovery across all six cursor types
and both mediated predictor modes. Its cases cover unexecuted and executed
Unknown liabilities, exact retries, stream conservation, original guard and
credential mismatches, premature replay, wrong stages and predecessors,
post-readiness canonical changes, ordinary capacity, and active/withdrawn source
custody with unrelated-suffix refusal. These bounded synthetic fixtures test
local contract behavior; they do not establish detector quality, authenticated
deployment topology, or remote outcome knowledge.
