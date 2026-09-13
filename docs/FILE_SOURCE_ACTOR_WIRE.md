# Fresh source admission at the existing actor wire boundary

Consumer: a trusted transport host driving ActorWire or ActorChannel backed by
FileActorPort<FileOversight>. exchange_actor_from_file and feed_actor_from_file
are available on its original FileActorSupervisor and FileSupervisedDriver. They
connect the intake primitive in FILE_SOURCE_INTAKE.md to actual decoded commands;
no new parser, command, ticket type, authority reducer or source format is added.

## Only new submissions acquire evidence

The existing JSON decoder and connection-local ticket-capacity check run first.
Only a valid Submit whose request is absent from the original durable RequestBook
calls prepare_file_intake. The same synchronous handoff then invokes the original
port submit and projection. A caller cannot reuse a manually primed snapshot to
skip this new capture. No callback/await occurs between successful preparation
and submission. Source capture and submission remain DISTINCT journal operations,
not an atomic multi-operation commit. A capture can commit while submission fails.

A known key proceeds directly to original deduplication without source I/O, clock
sampling, or source renewal. Every execution-bearing field is still compared by
the original submit_request; conflicting bytes cannot replace a request. This
also preserves recorded admission refusals. A preparation failure before submission
is different: it returns unavailable/capacity and has not created that request.
After actual recovery, that still-new key may be submitted with a fresh capture.

Poll and Cancel preserve the existing ticket visibility and lifecycle behavior.
Malformed/oversized documents, unknown fields, partial frames, pending response
writes/flushes and exchange limits cannot trigger new capture. Foreign or old
supervisor/gateway pairings refuse before any parsing, cancellation or source work.
The equality check uses the exact original weak-owner allocation, not numeric IDs.

## Shared framing, host diagnostics and interruption

FileActorExchange returns the original WireResponse plus an optional intake report.
Only the response is for the actor. FileActorFeed returns the original consumed
prefix/state plus that optional report; pending output remains in ActorChannel.
A report's successful result means source preparation, not durable submission or
permission to execute. Complete FileSourceError details remain supervisor-only;
source Binding errors are unavailable, not misleading idempotency conflicts.
Only quota/overflow refusals map to the existing capacity response.

The internal admission handoff is crate-private and cannot be installed by an
actor. The unchanged public exchange/feed APIs use a no-op handoff and preserve
existing manual integration. Native JSON validation, per-session tickets, port
submit/poll/cancel, response encoding and channel write/flush methods are shared.
Transport hosts retain bytes beyond FeedResult.consumed, then drain and flush the
original channel before feeding another frame. No new unbounded buffer or executor
is introduced. New admissions still perform synchronous file/journal work.

A lost response does not erase the durable request or restore its permissions.
A new wire session must retry identical bytes to reacquire ticket visibility;
knowing an ID alone still returns Withheld. Reopening the authority requires its
existing fence and discards live permissions. Retry and outcome reconciliation
need no source read, even when the file is now absent. These paths do not restore
an earlier lease or repeat publication.

## Regression source and qualification boundary

Ten test functions cover malformed and observation-only commands, exact/conflicting
retries, one-use consumption, newline/flush backpressure, seven-byte actual Unix
socket reads with a lost response, foreign gateways, redacted source failure,
recorded admission refusal, successful capture followed by failed submission,
native source quota, and complete owner reopening after publication. The latter
runs original socket helpers, independent human approval and guarded publication,
then verifies query-only recovery with no second capture or effect. Fixtures are
synthetic, not authenticated peers or measured trained-helper accuracy.

One new compile-fail example prevents external access to the admission handoff.
Together with the intake increment there are seventeen regression test functions
and two compile-fail examples. No original test bodies or assertions changed.
Rust compilation, formatting, Clippy, tests and doctests remain unexecuted. The
required RCH command cannot start because rch is unavailable (exit 127); source
integrity and remote commits are not qualification. No Beads or gates are closed.
The existing operator-owned storage, clock fidelity, finite journal/capture budgets,
anti-rollback limitations and absent hostile-process containment are unchanged.
