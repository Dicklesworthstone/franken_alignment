# Journal-checked work intake for generated actors

`FileActorInbox<FileGeneratedTextActorPort>` now supports native generated-text
requests. Its `drive` and `next_request` operations use the original registered
source acquisition, fixed generated-intent decoder, authenticated peer session,
request journal and full-input/two-key supervisor. `FileActorInbox` without a type
argument retains the ordinary durable actor port and existing API.

## Work discovery, not a new authority path

The inbox retains a bounded FIFO of request IDs, in observed submission order,
not numeric ID order. After framed intake it consults the ORIGINAL journal to
identify admitted requests needing supervision. Source preparation and socket
acknowledgment are not substituted for durable admission. An unsent or lost
reply therefore does not suppress a work hint for an acknowledged request.

Repeated IDs are deduplicated while queued. A retry can wake a dequeued request,
but `next_request` checks its current journal stage before handing it to the host.
Cancelled and terminal requests are skipped. Dispatching or Unknown requests are
reconciliation-only; they must not be reviewed or dispatched again. A hint is not
proof that the most recent submission succeeded: a conflicting retry may identify
an already existing request, but never creates another request or effect right.

Invalid socket budgets and foreign gateway owners refuse before socket reads.
All per-drive hint and diagnostic storage is reserved before a frame can change
the journal. Malformed generated references, fragments, polls, cancellations and
recorded retries retain their original source-read boundaries. A new failed
source observation or failed journal write supplies no candidate admission.
The generated adapter cannot supply arbitrary message bytes or bypass its fixed
reference decoder. The shared preparation callbacks are private to the owning
actor integration; callers cannot replace them.

Dequeue validates the original owner before consuming a hint. A faulted owner
leaves the front hint intact. Disconnect and ingress revocation retain accepted
obligations, request tickets and lifetime connection limits; neither operation
cancels an effect or refunds reserved resources. Helper cleanup remains on the
original supervised driver and runs after every drive result.

## Scheduling scope

Each inbox is one independently budgeted, credential-bound session. A host can
service multiple sessions using finite frame and socket allowances without first
draining a chatty peer. The added two-client regression deliberately schedules
each session; this is not an automatic fairness scheduler or a wall-time bound.
Synchronous source/journal IO and replay latency are outside the socket budget.
The inbox does not create a listener, task, runtime, numerical-generation step,
reviewer, permit, alternate request book or public admission callback.

The FIFO itself is only an in-memory scheduling hint. It does not survive a
process restart; the original durable request journal owns recovery, and exact
retries can reacquire work according to recovered status. A queue item never
claims that an old generation is eligible to resume or publish.

## Regression coverage and execution status

Seven `generated_inbox_` functions are authored using actual Unix socket pairs,
source/journal files and the unchanged synthetic-weight native-generator fixture:
lost replies and ticket-preserving reconnect/revocation; observed FIFO ordering,
retries/conflicts and cancellation; malformed/fragmented input and missing-source
repair; foreign owners and invalid/zero budgets; a real staging-file conflict
without a candidate ticket; original two-key dispatch/publication/reconciliation;
and independently bounded clients sharing the original supervisor.

The original inbox's queueing and status-selection algorithm is shared rather
than copied into a second implementation. An additional compile-fail example
specifies that public callers cannot replace source preparation.

Attempted command:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generated_inbox_
```

The attempt failed BEFORE compilation because `rch` is unavailable (exit 127).
All seven new tests, compilation, the compile-fail example, rustfmt, Clippy,
ordinary-inbox regression execution and the full repository gate remain
**UNEXECUTED**. Source inspection and matching blob hashes do not qualify a runtime
boundary. No bead is closed and no production qualification is claimed.
Same-process sockets do not establish hostile-process isolation or remote
identity; source authenticity, trusted clocks and storage fidelity remain the
original operator assumptions.
