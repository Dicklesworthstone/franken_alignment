# Computed learned congress through durable actor supervision

Status: unqualified implementation, advancing FA-113's live composition without
closing that packet or its prerequisites. This consumes the existing source-bound
congress and publication contracts in plan sections 8, 9, 10.15 and 17; it changes
no founding semantics, runtime, dependency, policy or journal encoding.

## One request and one original authority path

`FileSupervisedDriver::start_learned_probe_review` now accepts a durable actor
request and an opaque current `FileLearnedSidecar`, together with the existing
frozen member definitions, schedule and allowances. The original request must
be Reviewing and name precisely the sidecar's attempt and action. Current-source
validation precedes native setup. The driver does not overwrite input bytes,
manufacture a provenance marker or expose a verdict callback.

The driver owns the original `FileLearnedProbeReview`. Its explicit
`advance_learned_probe_review` performs the same bounded admission/probe/protocol
quantum and the same original durable finish/refinement as direct native use.
Original source checks run before scoring; all required commit/reveal and
refinement writes still require canonical acknowledgment. Original replay remains
synchronous, and no wall-clock latency or performance qualification is implied.

A refined packet updates the driver's current input revision but stays Reviewing.
Only a later original applied Continue enters AwaitingDispatch. The original
control sequence and exact final input are then used by the existing driver to
request a separate human approval, authorize, dispatch, publish and reconcile.
There is no automatic permit while waiting for that approval, no one-key fallback,
no alternative send path, and no numerical vote that itself publishes anything.
A running computed review cannot fall through the socket/provider step API.

Original actor tickets and the single request book remain unchanged throughout.
Cancellation of a dispatched request stays query-only and charged; releasing a
local review is not an endpoint outcome and cannot refund it. Policy, source,
stop and terminal-retirement methods still mutate the same original authority.

## Custody, terminal reports and handoff

`learned_probe_review` exposes read-only original status, per-member work, frozen
inputs and acknowledged round outcomes. `take_learned_probe_review` moves that
record only after computation is terminal. A new computed start refuses until
the previous record has been explicitly taken; it never silently drops evidence.
The returned `RetainedReview` has no mutable evaluator accessor.

Every existing worker-maintenance path also maintains the computed owner.
Original cancellation, stop, storage failure, retirement and release terminate
unfinished local evaluators while retaining their completed work and first native
failure. Interrupted native advances cannot fall back to socket driving. Cleanup
checks current original request state rather than an old retry receipt.

`FileDriverRelease` additionally carries the retained terminal computed review,
with the same supervisor and any child-reaping obligation. The actor port stays
bound to that supervisor. A newly constructed driver does not resurrect an old
review, automatic permit or publication phase. These reports are local evidence,
not durable total physical-computation escrow or independent helper processes.

## Regression sources and verification

Eight regression functions use the original history-sensitive decoder, fitted
codec, numerical helper probes, durable actor gateway and real journal files.
They cover actual uncertainty/refinement followed by mandatory two-key publication;
a standalone positive control paired with a computed alarm; stale calls and
partial cancellation; source advancement; missing deadlines; all five original
commitment-write failure barriers and fenced reopening; report-preserving handoff;
and dispatched cancellation without repeated scoring, publication or refund.
The fixture's parameters are synthetic controls, not trained-detector evidence.

The targeted command `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p
fa-reference driver::learned` cannot launch because `rch` is absent (exit 127).
Rust compilation, tests, rustfmt and Clippy are UNEXECUTED. Source hash comparisons
and whitespace checks are not runtime qualification. The full required RCH gate
must execute against the final revision before qualification. No existing gate,
assertion, dependency admission or license is weakened. The Beads CLI is absent;
no bead is edited or closed by this source addition.
