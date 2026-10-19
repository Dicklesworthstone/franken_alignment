# Stop a fleet of active publication supervisors

`supervise_publication stop-fleet` sends a separately authenticated stop request
to each member of an independently selected, bounded local fleet. It reaches
the original live supervisor while that supervisor still owns its journal lock.
Use this path when `shutdown_fleet` cannot open the journals of healthy active
owners. The existing [offline fleet recovery](FLEET_SHUTDOWN_OPERATOR.md) remains
the recovery path after owners have exited.

This is scoped FA-120 source serving FI-A02 and plan sections 8.11, 15.3, 15.4
and 16.9. It uses the original `StopClient`, credential admission and
`StopControlConnection`; the original driver owns stop, fence, reserved drain
and unknown liabilities. It creates no fleet journal, new authority reducer,
lease policy or independent publication capability. Compilation and execution
qualification remain pending.

## Command and independent roster

From a qualified Linux build:

```text
supervise_publication stop-fleet /operator/live-fleet.json
```

The command accepts exactly this verb and one manifest path. It does not read
the private supervisor CONFIG, ask for a human publication key or launch any
helper. Invoking it is the explicit restrictive choice to request Stop.

```json
{
  "schema_version": "fa.live-fleet-stop/1",
  "runtime_ms": 30000,
  "poll_ms": 5,
  "steps_per_peer": 8192,
  "members": [
    {
      "id": "1",
      "operation": "101",
      "peer_profile": "/operator/domain-a/reviewer.json"
    },
    {
      "id": "2",
      "operation": "202",
      "peer_profile": "/operator/domain-b/reviewer.json"
    }
  ]
}
```

Each `peer_profile` is the existing version-one independent reviewer profile,
with its full scope, reviewer ID, clock, supervisor/reviewer kernel identities
and protected socket directory. Its `operation` is the original request ID
whose stop endpoint the live invocation owns: for example,
`stop-101.sock` inside the first profile's directory. It is not a new fleet
operation identifier.

All fields are required and the parser rejects unknown fields. Member IDs and
operations are nonzero canonical decimal **strings**, including values above
JavaScript's exact integer range. Paths are absolute and normalized. IDs,
complete authority audiences and concrete stop endpoint paths must be unique.
Different profiles cannot count the same original endpoint twice.

| Bound | Admitted values |
| --- | --- |
| Members | 1 through 64 |
| Overall runtime | 1 through 3,600,000 milliseconds |
| Fleet poll interval | 1 through 1,000 milliseconds, within the runtime |
| Native client steps per member | 1 through 65,536 |
| Manifest | At most 128 KiB |
| Each independent peer profile | At most 16 KiB |

The entire manifest and every independent profile are admitted before any
connection is attempted. A malformed, missing or duplicate selected
configuration refuses the invocation before any Stop request. File intake is
bounded by the manifest and profile limits; it does not inspect model assets,
policy evidence or journals.

## One original connection and one request per member

The command orders the admitted roster by member ID and makes at most one
connection attempt per member. The shared connection helper verifies the
connected supervisor's Linux kernel credentials before parsing a protocol
offer. The original client then validates the exact audience, operation and
fresh session binding before it can queue the restrictive request. A live
peer's wrong credentials or offer affects that member's result; it cannot
silently substitute its claimed audience for the selected profile.

Once connected, clients advance in rounds with at most one original protocol
step per member in each round. A silent or malformed connected member cannot
consume all the other members' client-step budgets. The next pause is bounded
by the fleet poll interval, each active member's original poll interval and
remaining deadlines. Per-member elapsed time begins before its connection
attempt, and the single overall elapsed budget begins before manifest intake.
No connection, deadline or failure grants a fresh time or work allowance.

There is no automatic reconnect or Stop resend. An error after transmission
preserves `request_may_have_been_sent`; it does not prove that the owner is
still running. A successfully parsed native receipt reports exactly the
original owner's stop and drain disposition. A source/transport failure cannot
manufacture either acknowledgement or a refund.

Unix `connect` and filesystem intake are synchronous. A stalled kernel
connection or file operation cannot be preempted by this operator. The
round-based fairness guarantee applies to **connected nonblocking clients**;
this implementation does not establish a hard wall-clock bound under hostile
filesystem or socket-backlog behavior. It introduces no second executor to
hide that limitation.

## Read every member's result

The command emits one `fa.live-fleet-stop-result/1` JSON object containing the
entire admitted roster, even when later members could not be reached. It
includes aggregate `acknowledged` and `drained` flags, plus
`deadline_reached` for the overall deadline.
Each member retains its ID, operation, independently selected audience,
reviewer/clock, connection-attempt flag, native step count, status, reason,
possible-transmission flag and its own deadline flag.

| Member status | Meaning |
| --- | --- |
| `not_attempted` | The overall deadline prevented a connection attempt. No acknowledgement exists. |
| `unconfirmed` | Connection/protocol/deadline/step-budget failure or native refusal. The possible-transmission flag remains material. |
| `stopped_drain_refused` | The original Stop is acknowledged, but reserved drain refused. |
| `stopped_pending` | The original Stop is acknowledged; unresolved work or cleanup remains. |
| `stopped_drained` | The original owner acknowledged Stop and reports its drain complete. |

Native receipts also expose their original control sequence, revocation floor
and dispatcher epoch as decimal strings. These are per-domain observations,
not a global ordering or a persistent acknowledgement frontier. The current
wire format does not export a fleet-wide list of post-fence effects.

A successful exit requires every selected member to be acknowledged and
drained without crossing its deadline or the overall deadline. A native
acknowledgement decoded after a deadline remains a factual acknowledgement in
the report, alongside the timeout; it cannot erase that timeout. Otherwise the
command emits the complete report and exits with an error. If output fails after control work, it returns an error and never
repeats a command to recreate the output. Inspect original member histories
and use independently bound recovery before deciding whether a later manual
invocation is appropriate.

## Source and validation boundary

The implementation is
[`stop_fleet.rs`](../crates/fa-reference/examples/supervise_publication/stop_fleet.rs),
with bounded manifest admission in
[`stop_fleet/profile.rs`](../crates/fa-reference/examples/supervise_publication/stop_fleet/profile.rs).
The existing `stop-peer` command and this fleet command use the same
`PeerProfile::connect_stop_client` authentication path.

Regression source exercises real file-backed live drivers, authenticated named
sockets and the original reserved stop/drain path, alongside protocol faults,
lost receipts, partial drain, full-roster admission, deadline/step limits and
output failure. Static source review is not runtime evidence. The workspace
executor was unavailable during this change and the required remote-only RCH
gate has no available worker; no test execution or production qualification is
claimed. The full FA-120 distributed fence, leases and durable frontier
acceptance conditions remain open.
