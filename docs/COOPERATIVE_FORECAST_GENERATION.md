# Cooperative generation under an original pre-output forecast

This runtime scheduling extension serves FA-111 and FI-A12 together with the
external stop-the-line boundary. It implements bounded replay scheduling; it
does not change the predictor, evidence, authority or journal contracts in plan
sections 8.7, 12.7, 13.2 and 18.1. It adds no dependency, executor, wire version,
operator permission, forecast source or agent-facing command.

## Library contract

`FilePreOutputForecast::into_cooperative()` consumes the original process-local
forecast and returns `FilePreOutputStepper`. The owned K/V equivalent returns
`FileOwnedPreOutputStepper` and retains the original `LearnedForecastReport`.
Neither conversion samples, writes, forecasts again, or duplicates custody.

`advance_quantum(host, observed_revision, max_events)` executes at most the
specified number of original historical reducer calls and at most one original
finalizer. `FilePreOutputQuantum::replayed_events()` counts only reducer calls
made by that invocation, excluding a retained verified prefix and the finalizer.
A `ReplayingIntent` or `ReplayingCompletion` result has no new numerical outcome.
`IntentAcknowledged` records only the original write-ahead intent and always
returns before completion. Only `Completed` carries an acknowledged original
generation event or an acknowledged native numerical error.

The supervisor can service stop requests, observe time, refresh policy evidence,
and poll actors between quanta. Each next call supplies the actual current
journal revision. The original catch-up algorithm verifies appended history
without resetting its earlier cursor. The original retired-machine cache is
moved, not cloned, between stages. The first preparation starts cold and leaves
the live owner's existing replay cache available for intervening control/source
transactions; subsequent stages use the stepper's own acknowledged continuation.
These are separate historical candidates, not additional effect owners. They
retain extra memory; this change makes no memory-reduction claim.

Owner identity, exact numerical predecessor, original request, forecast deadline,
authority epoch, coverage and source eligibility remain mandatory. An expired or
consumed forecast, a stopped or recovered owner, a source interruption, or another
numerical step cannot be repaired by passing a larger journal revision. Invalid
quanta and stale/foreign calls perform no replay or journal write.

Replay failures remain in their original preparation. A failed or interrupted
consuming finalizer cannot be retried from an earlier candidate by this stepper.
Storage recovery determines whether a failed write became visible. Dropping the
stepper does not cancel an acknowledged intent, answer its pending forecast,
refund work, or authorize publication.

This is an event-count bound, **not** a wall-time, memory, CPU or cancellation-
latency guarantee. An individual historical event may perform numerical work.
Preflight checks, one new numerical step, journal encoding and storage replacement
remain synchronous. Forecast acquisition and recovery retain their existing
separate contracts. Release still requires original sidecar/native review, human
approval, current witnesses, dispatch and a confirmed original receipt.

## Publication operator

The existing `create-learned-generated` recipe `/5` now moves its original raw or
owned forecast handle into this scheduler. Its numerical loop invokes one
`advance_quantum` with a fixed maximum of 32 historical reducer calls, then
returns to the existing workflow deadline, authenticated stop service and fresh
policy-source observation. A separately acknowledged intent also yields before
its completion. Progress means only that this slice completed, not that a token
or publication is available.

The original catch-up path admits the extra source/clock events and checks their
full history without restarting the verified prefix. Every message still uses
its original forecast, and explicit finish still gets its own original forecast.
No recipe, model, wire schema, sample budget, authority, native review or human
key changes. Non-forecast generation, prompt processing, forecast acquisition,
source refresh and recovery retain their existing synchronous boundaries; this
change does not make the entire command preemptible.

Existing tests that explicitly call one complete sample keep that exact contract
through a test-only draining helper. The production operator has no such loop;
its new cold-replay tests call the same one-quantum method as the actual command.

## Verification status

Eight new Rust unit-test functions cover both original raw and owned predictors,
including byte-for-byte journal/output parity with the synchronous owner,
interleaved clocks, exact replay quotas, stale/foreign calls, consumed forecasts,
expiry on both sides of intent acknowledgment, held output, dropping preparations,
and every existing storage-fault barrier for both finalizers. Two compile-fail
doctests assert that the new raw stepper cannot be cloned or unwrapped.

Four new operator test functions use both loaded raw and owned predictor modes.
A cold history longer than the quantum must return without a sample or journal
write, then proceed through the original actual native reviewers, separate human
approval and confirmed stream publication. Causal controls withdraw the real
policy evidence file, expire the workflow deadline, or expire the forecast after
intent acknowledgment. A Linux test queues a real peer-verified stop client
between quanta and checks original stop/drain with no sampled or published output.
Socket errors are not skipped. Existing per-sample and multi-message operator
tests remain unchanged.

These tests are authored, not executed. In this delivery environment `rch`,
`cargo`, `rustc` and `rustfmt` are absent. The prescribed remote-only invocation
stops before compilation with exit 127. No Rust syntax, formatting, type-check,
Clippy, test, performance, calibration or production qualification is claimed.
FA-111 and its existing in-progress operator beads remain open.
