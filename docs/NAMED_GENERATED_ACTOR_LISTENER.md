# Source-backed generated actors on named Unix listeners

`FileActorSupervisor::poll_generated_listener_from_file` connects the existing
`UnixPeerListener<FileGeneratedTextActorPort>` to the original registered-file
intake and native-generation request path. `FileSupervisedDriver` exposes the
same operation and retains its helper cleanup on every result. This extends
FA-107's one-directional actor boundary (plan 8.7, 9.11 and 17.1), not the actor's
authority. The named socket is bound by the operator in a controlled namespace.

## One bounded scheduler turn

`ListenerPollBudget` independently enables at most one nonblocking accept attempt
and supplies the existing socket `DriveBudget`. A fully unscheduled turn performs
no accept, read, write or flush. Invalid socket budgets and foreign supervisor
ownership refuse before acceptance. Kernel peer credentials are still checked
before reading any actor bytes. Only complete new source-reference/finish frames
acquire evidence through the original validator, one-use snapshot and lease path.

A rejected candidate, including a Busy rejection, does not prevent servicing the
original active peer. `ListenerPollReport` retains the acceptance result and the
separate drive result, including a drive failure after successful acceptance.
A missing result means the operation was not scheduled or no peer was active;
it is not success. Accept attempts are charged separately from socket IO calls.
An accept IO error is retained as its error kind; there is no hidden retry loop.
The host still explicitly disconnects terminal transports before reconnecting.

The original write/flush backpressure, connection-time UID/GID/PID checks, lifetime
connection limits, request tickets and revocation survive listener service. No
public port/session getter or admission callback is added. Raw `poll` services
ordinary ports; source-backed generated actors use the supervisor operation.
Neither accepting a socket nor preparing evidence generates tokens, grants either
publication key, cancels an effect or refunds an unknown result.

## Regression scope and execution status

Seven Linux tests in `source/peer/listener_tests.rs` use actual named Unix sockets,
source and journal files, and the unchanged synthetic-weight native-generator
fixture. They cover two-key publication, rejected and admitted credentials,
foreign owners and budget preflight, fragments and pending replies, changed policy
evidence, Busy handling, reconnect tickets/limits, revocation, nested diagnostic
retention and anchored recovery with a fresh reader below the durable source floor.
The nested-error case deliberately injects a trusted drive callback failure; it
does not claim a physical storage fault. One compile-fail example specifies that
external callers cannot replace admission through `poll_with`.

Attempted command:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference named_
```

The attempt failed before compilation because `rch` is unavailable (exit 127).
Rust compilation, all seven new tests, the compile-fail example, rustfmt, Clippy
and the full repository gate remain **UNEXECUTED**. No historical execution log
qualifies these changes. No bead is closed and no production feature is qualified.
Same-process peers do not prove hostile-process isolation; producer authenticity,
clock fidelity and filesystem durability retain their existing assumptions.
Socket budgets do not bound synchronous source or journal latency.
