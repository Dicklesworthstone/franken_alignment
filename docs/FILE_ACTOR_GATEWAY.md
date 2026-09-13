# Actor requests across complete file-authority process loss

Consumer: an operator attaching the existing actor protocol to FileDelivery's
original durable publication lifecycle. This extends FILE_DELIVERY_REQUESTS.md
and the existing ActorWire/ActorChannel, not another codec or authority reducer.

## Roles and one-use observations

FileDelivery::into_actor_gateway consumes the exact exclusively locked host and
returns FileActorPort plus its noncloneable FileActorSupervisor. Only the trusted
supervisor can access FileDelivery, install a policy snapshot, review a request,
reserve a permit, dispatch, publish, fence, stop or reconcile. The port exposes
only the original submit/poll/cancel vocabulary and Knowledge<ActorOutcome>.
It supplies scope from the fixed host bootstrap; actor bytes cannot choose it.

Before each new synchronous submission, the supervisor installs one explicitly
current, complete and bounded policy snapshot with the expected journal revision.
Submission consumes this observation. Exact retries, polling and cancellation do
not need one. Invalid snapshot replacement withdraws the older snapshot; mutable
host access and committed actor cancellation invalidate any unused snapshot.
There is no inference that persisted history is a current observation and no
promise that the supplied snapshot completely covers the external world.

A ticket is returned only after the original journal replacement succeeds. The
original ledger's admission refusal is a retained NotAdmitted outcome, not an
opportunity to silently retry with better evidence. Reservation and review remain
Pending, dispatch uncertainty remains Unknown, and only original terminal states
become Known. Private attempt IDs, ballots, salts, budgets, snapshots and reasons
are not interpolated into the actor response. A mutable supervisor borrow yields
Unavailable/Unknown at the port, never a RefCell panic or leaked authority.

## Original bytes and transport behavior

ActorWire and ActorChannel now have a sealed port parameter defaulting to the
original in-memory ActorPort. Their command parsing, JSON response encoding,
connection-local ticket visibility and framing/I/O method bodies are unchanged.
The durable port uses exactly the same decimal-string IDs and hex payloads.
External implementations cannot attach an arbitrary authority backend to this
sealed interface. Existing memory-port callers retain their original defaults.

Construct ActorWire::new(port) and ActorChannel::new(wire, limits) after providing
the trusted snapshot. Complete submit frames make durable admissions; incomplete
frames never do. Output backpressure stops new intake, partial output resumes, and
an output failure never replays the already committed request. These synchronous
submissions can perform bounded journal replay and filesystem writes; channel I/O
budgets are not filesystem latency, allocation or scheduler-fairness guarantees.
No replacement executor, listener, authentication system or dependency is added.

## Recovery without resurrection

Ports and tickets retain only Weak ownership. Dropping the supervisor releases
the original host/lock even when its ports, channels or tickets remain alive.
Old handles return controller-unavailable observations. Reopening uses the existing
exact-bootstrap/location checks and the durable original authority fence before
a new gateway is returned. No snapshot, permit or input admission is restored.

A new wire session cannot poll an old request merely by knowing its numeric ID.
An exact original submit reacquires its observation ticket without another durable
event, proposal or permit. A foreign ticket cannot cross gateway identity. Existing
cancelled reservations stay cancelled, dispatched unknown effects stay charged,
and original publication receipts still govern reconciliation. A fresh request
requires a new key, the current epoch, a fresh clock and a fresh supervisor input.
Neither actor cancellation nor connection loss is nonexecution evidence.

The new process regression starts the actual Rust test executable, observes its
PID-marked durable stage, kills and waits for that owned process, then reopens the
store without any surviving controller/key objects. Its four cases cover death
after submission, reservation, dispatch and publication. Exact wire retry must
retain one original request and produce no second effect. This is a test scenario
in source, not an executed crash qualification or simulated power-failure result.

## Verification and retained limitations

Seven wire/port/channel scenarios plus one four-stage subprocess scenario and
its subprocess entry point accompany the nine durable-request core scenarios:
seventeen scenarios, eighteen Rust test functions and one compile-fail example.
Rust compilation, formatting, Clippy and execution have NOT run in this editing
environment; the required RCH runner is absent. Original tests and bootstrap/event
bytes (other than the new request tag) are preserved. No bead or production gate
is closed, and no historical execution evidence is promoted.

This is the existing narrow operator-controlled Unix file-publication profile.
Reference ballots are trusted observations, not authenticated helper processes.
Full OversightBroker, human two-key and hosted-decoder state are not migrated by
this adapter. Peer authentication, hostile-process containment, anti-rollback
storage and journal authenticity remain separate requirements. The supervisor
can still access all privileged host operations; it must never be given to the
actor. A configuration or response is not a production safety certificate.
