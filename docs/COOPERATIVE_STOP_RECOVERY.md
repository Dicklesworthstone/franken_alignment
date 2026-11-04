# Stop-only cooperative recovery

The offline supervisor's stop command needs the complete original journal to
select its exact stop predecessor. Reading a publication snapshot and then
reopening for stop used to replay that history twice before terminal processing.
`FileOversight::begin_stopped_recovery` instead retains one exclusively locked
canonical image and one private reconstruction through selection and completion.
This is FA-014 / plan 16.4, 16.5 and 25.4 reference capability, not qualification.

`advance(expected_events, max_events)` invokes at most the requested number of
original `Machine::apply` transitions. Construction still performs synchronous,
bounded file acquisition, canonical decoding and configuration admission. A
single event may perform substantial work; event quanta are not hard deadlines
or a promise of interruptible system calls. Progress contains only original event
counts and status. No partial snapshot, owner, permit, helper or human role escapes.

Cancellation or dropping, including at Ready, releases the original Store lock
without changing canonical bytes, cleaning staging files, installing a fence or
acknowledging a stop. Errors and caught transition unwinds retain a failed or
interrupted state; retrying a poll cannot reuse that partially changed machine.
A stale predecessor or invalid quantum performs no event work. An optional
independent history anchor and recovery-reserve requirement are checked before
replay, never installed or inferred from the untrusted history by these calls.

At Ready, `stop_request(operation)` selects the exact original control/authority
predecessor under the same lock. A retained stop preserves its original request;
a different operation cannot take it over. `finish_stopped` and
`finish_stopped_with_anchor` consume the cursor through the existing fixed
Stop/Fence/StopProgress finalizer, with no ordinary recovery fence or second
historical replay. Every original limit, reserve and accounting rule still applies.

A matching drained retry returns the unchanged canonical observation without a
clock call, event or replacement, even at capacity. Otherwise finalization checks
intent and retained canonical bytes before calling time, then repeats canonical
comparison after original terminal preflight and before cleanup/replacement.
Failure may leave a visible but unacknowledged terminal replacement. Recovery
must inspect that same history, not resend an effect. Executed charges remain
charged; expired unknown effects are not refunded or reported drained.

The optional returned anchor names exactly the acknowledged stopped cut. Its
external retention remains a separate operation. The ordinary `recover_stopped`
API uses this same path synchronously; existing request and return types remain
unchanged. This is not a live guarded-recovery shortcut. Learned or predictive
histories needing external reconstruction recipes retain their explicit APIs.
No producer/evaluator authentication or complete mediation follows from replay.

Eight native regression functions cover original-transition parity across
quanta and reserved/sent/executed/expired liabilities; cancellation at every
boundary; invalid polls and early completion; failure/unwind latching; exact
full-capacity retries and anchors; canonical changes before/during the final
clock; all five native storage barriers and visible-replacement retry; and
independent profile, reserve and valid-fork anchor refusals. They use the original
two-key journal fixtures and leave existing tests unchanged. Rust compilation,
runtime tests, formatting qualification and Clippy remain pending the exact-
revision remote-only RCH verifier. Source review is not executed qualification.

## Ordinary offline command

`recover-stop CONFIG OPERATION` now uses this stop-only cursor directly. One
monotonic elapsed budget from CONFIG's existing `timing.runtime_ms` is checked
before open, before each original event, at Ready, and immediately before and
after a required terminal clock acquisition. The command executes one event per
checkpoint; it does not reset the budget on progress or sample logical time during
replay. Expiry before acquisition makes no terminal clock call. Expiry during
acquisition can follow one clock call, but its tick is not accepted and no cleanup,
new stop or output acknowledgment follows. A later explicit retry starts from the
actual canonical file, not a serialized partial replay or saved permit. Matching
already-drained retries skip acquisition and its two checkpoints entirely.

After Ready, the exact stop request comes from the same locked reconstruction,
not a separate `read_publication` scan. Finalization and JSON reporting keep their
existing meaning, including acknowledged stops whose reporting failed and
unresolved charges that return pending output plus an error. Initial file decode,
one event and synchronous final storage work can exceed the elapsed budget; this
is cooperative responsiveness, not a hard real-time shutdown guarantee.

The independently selected `--guarded` path is preserved byte-for-byte. Its
external anchor, complete guard/floor requirements and custody export remain
mandatory there; no failure automatically retries through ordinary recovery.
Learned recovery keeps its original separately supplied reconstruction recipe.
No command/schema, authority format, helper dependency or policy field is added.

Six additional command tests reuse the original native helper/reviewer fixtures:
all deadline checkpoints, cumulative expiry neighbors and uninterrupted lock
custody, a valid canonical replacement at Ready, full-capacity clock-free retries,
actual executed versus irrecoverably unknown effects, and elapsed/terminal-clock
unwinds with corrected retries. Existing command tests are unchanged except for
registering this new test module. These 14 new tests and the ownership doctests
remain unexecuted until exact-revision RCH verification; missing RCH is not a pass.

On 2026-10-24, source `81365a31` executed the complete original 50-function
operator stop namespace on qualified RCH: 48 passed, two retention-state
disagreements failed, zero ignored. The cumulative budget/lock case and original
executed/unknown accounting case passed with all original state assertions.
The complete operator inventory at this source failed 726/8. This is later scoped
operator evidence; the preparation-time nonexecution statements above remain
historical, and no library/doctest or full qualification pass is inferred.
