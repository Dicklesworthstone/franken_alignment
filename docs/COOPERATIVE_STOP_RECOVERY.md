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
