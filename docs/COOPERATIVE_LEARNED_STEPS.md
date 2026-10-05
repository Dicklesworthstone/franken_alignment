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
explicit task. In this increment, new intent admission still uses the original
synchronous `begin_learned_step` operation.

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

Mandatory targeted RCH tests and the full xtask gate were attempted but did not
launch: `rch` is absent (exit 127). Compilation, tests, rustfmt and Clippy are
unexecuted; no local compiler fallback, qualification or bead closure is claimed.
