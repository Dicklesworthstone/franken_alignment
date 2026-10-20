# Preserve independent recovery requirements across separate stop and drain

FA-014; plan 16.4, 16.5 and 25.4; serves FI-A02. The existing clock-independent
terminal operations can now consume the original base or mediated guarded
reconstruction, without downgrading to an ordinary recovery entry point.

`FileGuardedRecovery::finish_admission_stop` and
`FileMediatedRecovery::finish_admission_stop` require complete original replay
and recheck their full independent requirements before calling the original
Stop/Fence finalizer. They acquire no time, return no live owner or role, and do
not manufacture a drain receipt. Matching already-stopped cuts are confirmed
unchanged. The immutable result includes the exact acknowledged history anchor.

Both cursor types also expose `finish_stopped_drain(request, clock)`. It requires
an existing matching stop and retains the original one-event StopProgress path.
The fallible clock is called only when native settlement needs it; drained retries
skip it and do not append a fence or event. Two stop events plus one drain event
fit the original three-event reserve. Unknown/expired and executed effects keep
their original accounting. A missing clock cannot undo an earlier admission stop.

The guarded checker runs again at each consuming entry. Anchors, external floors,
effective policy, credentials and the entire selected guard inventory remain
mandatory. Mediated recovery additionally checks its initial/current topology,
availability and optional prediction/evaluation. The independent drain profile
must describe the actual stopped availability, not reuse an old permitting cut.
Publication-profile and reserve pins selected before replay still bind the same
immutable events. Learned recipes retain their separate reconstruction APIs.

The internal handoff transfers the same decoded history, native machine and
exclusive Store into the existing terminal consumer after all checks pass. It
performs no reopening, numerical replay, initial fence or public state import.
No partially reconstructed owner, specialised inner cursor or caller-selected
checker is exposed. The original terminal implementation owns canonical-byte
comparison, source/consistency admission, encoded-prefix limits, exact retries,
file/directory synchronization and failed-write uncertainty.

Native acknowledgment and external anchor retention remain separate. A failed
storage operation may leave a visible result; an error supplies no candidate
receipt and never proves nonexecution. Keep previous custody until its successor
is acknowledged. Storage isolation, authentic independent requirements and clocks
remain operator assumptions; cooperative replay is not a real-time bound.

Eleven new native regression functions use original two-key effects, original
Machine transition/encoding oracles, independent anchors and real files. They
cover all liability classes and replay quanta, exact reserve exhaustion, final
guard/topology/floor rechecks, incomplete/failed/interrupted replay, cancellation,
fallible/stale clocks, canonical changes, same-counter history forks and all five
storage-failure barriers for each phase. Existing tests/assertions are unchanged.
These tests are authored, not executed. Required targeted and full xtask RCH
launches cannot reach Cargo in the editing environment (rch and Rust tools are
absent). Source checks and commits do not qualify compilation or production;
FA-014 remains open pending exact-revision execution.
