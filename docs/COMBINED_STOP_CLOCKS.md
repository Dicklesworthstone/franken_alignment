# Reject expired clock acquisition before combined stop recovery writes

FA-014; plan 16.4, 16.5 and 25.4. Ordinary and guarded `recover-stop` now
check their existing cumulative elapsed budget immediately before and after a
required terminal clock acquisition. Previously the last check occurred at replay
Ready, so a slow clock could supply a usable timestamp after the deadline.

The original `FileStopRecovery`, base `FileGuardedRecovery` and complete
`FileMediatedRecovery` expose `try_finish_stopped_with_anchor`. Its clock returns
`Result<ElapsedTick, JournalError>`. Refusal propagates before cleanup, Stop,
Fence, StopProgress or replacement and returns no candidate history. The same
consumed lock is released. A clock unwind likewise cannot acknowledge a stop.
No synthetic time, earlier cached tick or weaker recovery fallback is used.

The operation still consumes the original verified replay and fixed combined
transaction. Full guarded or mediated requirements precede the handoff; all
canonical-history, stop-identity, capacity and native consistency checks remain.
Existing infallible public methods keep their signatures and behavior. A matching
already-drained receipt skips acquisition, including at full journal capacity.
Thus expiry checks inside the clock callback cannot turn a completed retry into
another clock requirement or a new Stop/Fence transaction.

Both runnable combined paths preserve the same elapsed budget through their
native callback. Expiry produces no report of native success and leaves domain
canonical and staged files unchanged. A later explicit retry must reopen and
validate actual history. Successful native acknowledgment is still reported even
when subsequent output or anchor retention fails; these failures do not undo
settlement or authorize effect resend. Separate clock-free admission stopping
remains the available operation when time cannot be acquired at all.

This bounds acceptance of clock results, not callback duration. File intake,
one event, preflight and storage remain synchronous and may exceed the elapsed
budget. It is not a hard real-time guarantee, a distributed transaction or clock
authentication. The legacy combined command still accepts its existing trusted
infallible clock function; the newly fallible native boundary makes deadline
refusal possible without inventing a timestamp or using a panic for control flow.

The deterministic operator fixtures distinguish expiry before acquisition from
expiry after a completed callback. The latter legitimately makes one logical
clock call, while still preserving canonical/staged bytes and emitting no native
acknowledgment. The cumulative-budget case retains the original Ready and
before-clock cuts and adds the exact post-clock one-short/sufficient neighbors.
These clarify the existing checkpoints; no deadline or production guard changes.

Four authored regression functions use the original native file owners and
operator paths for ordinary, base-guarded and topology-mediated recovery. They
cover slow acquisition with exact full-capacity successful controls and clock-free
retries; both acquisition deadline boundaries; explicit native clock refusal and
unwind; original infallible byte/result equivalence; wrong intents and canonical
replacement during acquisition. Existing assertions remain unchanged. These tests
have not executed: remote-only RCH targeted and full xtask launches cannot reach
Cargo in the editing environment. Source/hash/patch checks are not compilation,
rustfmt, Clippy, runtime or production qualification. FA-014 remains open.

On 2026-10-24, source `81365a31` executed all 50 original operator stop functions
on qualified RCH: 48 passed, two retention-state disagreements failed, none were
ignored. The cumulative Ready/acquisition boundaries and mediated every-cut
function passed. This later scoped execution does not replace the historical
unexecuted status above or establish that every public/library target passed.
The complete operator inventory failed 726/8; canonical dependency admission
also failed, and a separate Clippy run failed on 80 library errors before later
targets. FA-014 and full qualification stay open.
