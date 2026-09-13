# Full-input and mandatory two-key oversight behind the original actor wire

Consumer: a supervising host using FileOversight with the existing actor protocol
and nonblocking Unix transport. This joins FILE_OVERSIGHT_REQUESTS.md to the
existing FILE_ACTOR_GATEWAY.md path without replacing either authority reducer,
command parser, response encoder, frame channel, or socket drive implementation.

## Same gateway, distinct authority owner

FileActorPort, FileActorTicket and FileActorSupervisor now accept a sealed host
parameter defaulting to FileDelivery. Existing callers keep their original types.
FileOversight::into_actor_gateway returns the same gateway parameterized by the
full-input, mandatory-human-key owner. The shared sealed FileRequestHost interface
contains only durable request methods and read-only revision/fault access. Only
the two original durable host types implement it; no external callback/backend
can add a permit, clock, verdict or publication method to the actor role.

The host is moved, not copied. Its original reviewer role remains separately
owned and is not stored in the gateway. Only the trusted supervisor can obtain
the host to capture full helper inputs, commit/reveal their judgments, authorize,
request human approval, dispatch with both keys, and publish. No in-memory or
one-key fallback exists for a FileOversight port. The request book still binds
all exact action fields while the original scope comes from independent bootstrap.

All submit/poll/cancel bodies, snapshot replacement rules, diagnostic redaction
and Knowledge projection remain unchanged. A new admission consumes one explicitly
supplied bounded complete snapshot. Privileged host mutation or cancellation
invalidates an unused observation. Snapshot data cannot enter from actor bytes.
Exact retries, polls and terminal/inflight cancellation need no new observation.
A completed review or human approval still appears as Pending, never permission.

## Transport and recovery

Construct ActorWire::new(port), then the original ActorChannel and
UnixActorConnection. They need no source changes. Partial reads/writes, complete
frame admission, ticket visibility and the write/flush barrier use their existing
implementation. A completed request can perform the original synchronous bounded
journal I/O; socket budgets do not constitute filesystem latency guarantees.

Weak ports/tickets cannot retain the authority lock after supervisor loss. Existing
handles become unavailable. Reopening still reconstructs original full-input
history and withdraws old human keys, reservations and sendable envelopes before
returning a new owner and reviewer role. The latter is never recovered from a
port. A new session must submit the identical original proposal to regain its
observation ticket. That does not rerun admission, refresh evidence, issue keys,
refund uncertain effects, or publish again. Knowing a numeric key alone cannot
import another session's ticket.

Neither missing helper data nor loss of the reviewer role prevents original
endpoint reconciliation. Unknown effects remain charged until their original
endpoint outcome is established. A storage error cannot expose a speculative
ticket/status, even if the journal replacement may already have become visible.

## Verification boundary

Five new wire/gateway/transport scenarios cover complete two-key publication and
redacted polling, weak-owner lock release and exact recovery, one-use snapshots
and borrow failures, real staging-write refusal, and bounded seven-byte reads /
three-byte writes through the existing Unix adapter followed by publication.

A sixth scenario runs the actual test executable as an owned subprocess, waits
for its PID-marked durable stage, kills and waits for that process, then opens a
new owner without surviving keys or controller objects. Four stages cover actor
submission, both approvals, dispatch, and publication. The subprocess entry point
is a separate test function, not an independent seventh scenario. The source
requires one original request and zero duplicate publication after every recovery.
A compile-fail example excludes supervisor extraction from the two-key port.

Combined with the request-book increment, twelve scenarios occupy thirteen new
Rust test functions plus one compile-fail example. None has been compiled or
executed here; RCH is unavailable. Source-body/hash checks are not Rust or crash
qualification. Original tests, bootstrap bytes, dependency admission, historical
execution records and Beads statuses are unchanged.

This is still the operator-controlled Unix publication-file reference profile.
The serialized commitments remain reference-only and peer/helper/human provenance
is not authenticated by this adapter. Anti-rollback storage, hostile-process
containment and durable migration of the hosted decoder remain separate work.
