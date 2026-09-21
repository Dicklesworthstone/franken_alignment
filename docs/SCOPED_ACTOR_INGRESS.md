# Request-scoped live actor ingress

The original Linux `PeerSession<FileActorPort<FileOversight>>` can now be driven
for one independently selected request. Both operations are methods on the same
`FileActorSupervisor` and `FileSupervisedDriver`; they neither create a second
mailbox nor expose the supervisor to the actor.

`drive_peer_request_from_file` permits a new Submit only for the selected key.
It checks the matching gateway and nonzero selected key before socket work, and
rejects other keys before reading evidence, sampling time, or consuming an unused
admission snapshot. The existing frame decoder, ticket map, file-source intake,
request journal and redacted response remain the only implementations.

`drive_peer_request_observe` serves poll/cancel and recorded Submit retries while
new admission is closed. It accepts no source or clock. Even an already-installed
snapshot cannot enable an absent request here. Exact/conflicting retry comparison
still belongs to the durable owner; cancellation still cannot refund dispatched
or unknown work. Polls without a session-owned ticket remain withheld.

The runnable consumer is a bounded one-request supervisor. It needs this split
because admitting additional requests during a congress round could mutate its
shared control predecessor. Keeping observation/cancellation available is not a
license to start concurrent reviews. Reconnection belongs to the original peer
session and preserves its ticket visibility and lifetime connection limit.

Process credentials authenticate the connecting UID/GID and optional PID, not
executable identity, a human, or the current holder of a transferred descriptor.
Source completeness, freshness clock integrity and cooperating filesystem access
remain the original reference-profile assumptions.

Five regression tests use actual Unix sockets and native durable source intake:
selected admission then read-free polling/retry/conflict/cancellation; foreign
keys and closed intake with an allowed control; wrong gateway/zero key/invalid
budget before I/O; incomplete frames; and retained tickets after reconnect.
These tests have not been compiled or executed in the editing environment.
