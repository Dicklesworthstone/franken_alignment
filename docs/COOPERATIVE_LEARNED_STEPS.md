# Cooperative preparation of durable learned steps

Source implementation, not execution-qualified. This advances the cooperative
ownership and scheduling requirements of plan sections 6.3, 10.6 and 18.3 using
the existing reference journal and original numerical engine. No runtime,
executor, journal tag, numerical algorithm or permission path is added.

## Pending-step completion

After the original `begin_learned_step` acknowledges an intent, call
`prepare_learned_step_completion` with that owner's current journal revision and
the exact pending actor revision/position. Preparation starts with an empty
private Machine and performs no historical inference or storage write.

Schedule `FileLearnedStepPreparation::advance(&host, expected_events, max_events)`
for bounded historical-event quanta. Every event goes through the original
Machine reducer, including numerical recomputation and exact witness comparison.
The candidate is neither deserialized state nor a second interpretation of the
journal. The count advances only after a complete original transition returns.
A failed reducer is sticky; an unwound reducer leaves Interrupted, never Ready.

The task holds no host borrow between advances. The same actor gateway can be
polled, and the supervisor remains able to mutate its original owner. Any live
journal mutation, including time observation, cancellation, fencing or source
refresh, invalidates the task's exact predecessor. It must be discarded and
prepared again, not rebased over an unseen history. Source interruption and
storage failure are checked even when no revision changed. A new owner after
recovery cannot adopt the old task, even with the same path or recipe.

Ready means only that history is reconstructed. `finish(&mut host)` separately
rechecks original admission, executes the one next original numerical operation,
encodes its original witness and uses the existing poison/acknowledgment storage
boundary. It does not replay historical inference a second time. No output or
candidate key is accessible before successful acknowledgment. A numerical error
may itself be an acknowledged inner Err; a persistence failure is an outer Err
with no candidate output, and the original live owner remains unavailable.

Dropping a task, including after premature finish or while Ready, changes no
canonical bytes, numerical state, effect balance or pending intent. The original
intent remains the barrier and can be completed by a new task, the synchronous
API, or the existing independently configured recovery path. Two preparations
from the same predecessor cannot commit twice: the first completion changes the
journal revision and invalidates the second. A task's identity marker does not
retain the file lock after the live owner is dropped.

The synchronous `complete_learned_step` consumes this same preparation. It keeps
its existing return type, event count, original algorithm and storage boundary.
It remains synchronous by choice; callers needing scheduling points use the
explicit task. Synchronous intent admission now consumes the same reconstruction
machinery through the separately typed intent preparation below.

## New-step intent admission

`prepare_learned_step_intent` produces a `FileLearnedIntentPreparation` before
any new intent is recorded. It checks the original next-position, pause/reset,
source, and two-ordinary-event-slot admission laws before reconstruction. The
same reducer and bounded-event advance power both stages; the distinct public
types have distinct finalizers. An intent preparation cannot be converted into
a completion preparation or used to extract a token.

After the intent preparation becomes Ready, its `finish(&mut host)` applies and
persists only the original Begin event. No new numerical step runs during this
operation. The caller then creates a completion preparation against the new
journal revision, advances it to Ready and finishes that separate operation.
Both histories can be scheduled without a retained mutable host borrow. The two
canonical acknowledgments and the completion's original witness format are
unchanged. There is no interval in which new inference ran before its intent
was acknowledged, and Ready never means a next token or effect was approved.

Dropping an intent preparation before acknowledgment leaves no new pending
operation. Dropping a completion preparation leaves the acknowledged intent
pending. A failed intent write may or may not be visible; the live owner stays
unavailable and the original recovery path determines the canonical result.
The synchronous begin/advance/complete APIs remain source-compatible and consume
the same implementation; synchronous callers still receive synchronous behavior.

A host loop may poll actor tickets between replay quanta. An intervening actor
cancellation invalidates either preparation, preserves the original cancellation
outcome, and does not grant a refund for any unknown dispatch. The caller may
explicitly construct a fresh task at the new revision. Frequent writer changes
can repeatedly invalidate preparation: there is no liveness or latency guarantee
under arbitrary journal churn and no automatic retry that hides repeated work.

## Bounds and nonclaims

One historical event can contain a complete original inference or review
operation. Event count is not a wall-clock, instruction, allocation or latency
bound. Candidate construction, capacity checking, one next numerical operation,
encoding and storage replacement remain synchronous. Total historical replay
work is not reduced, and an invalidated task's physical work is not a durable
compute charge. The original logical budgets and unknown-effect balances do not
change merely because reconstruction is scheduled in multiple quanta. There is
no parallel execution or publication-capable background task.

## Validation

Eight authored regression functions compare every completed token against an
independent original generator; keep storage unchanged between quanta; exercise
drop, stale and foreign tasks, competing completions, source interruption,
corrupt original witnesses, all five replacement barriers and recovery; and
preserve both already-published and unknown dispatched effects. Two compile-fail
examples deny sample extraction and task cloning. Synthetic weights are causal
controls, not detector-effectiveness evidence.

Seven additional regression functions cover fully explicit intent/completion
execution, byte-identical original transaction records, intent abandonment,
exact versus insufficient ordinary capacity, all five intent-write barriers,
actor polling/cancellation in both stages, original numerical alarms paired with
quiet controls, and acknowledged telemetry-budget failure across recovery. The
existing tests and original numerical/witness/codec bodies are retained. A third
compile-fail example rejects intent-to-completion conversion.

Mandatory targeted RCH tests and the full xtask gate were attempted but did not
launch: `rch` is absent (exit 127). Compilation, tests, rustfmt and Clippy are
unexecuted; no local compiler fallback, qualification or bead closure is claimed.
