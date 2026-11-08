# Live Tower publication service and client

On Linux, the existing `supervise_publication` executable can serve ordinary
publication through the [Tower request schema](TOWER_REQUEST.md). The original
file owner, source admission, helper/human review, two publication keys,
independent Stop service and cleanup remain the execution path. The host fixes
the actor's grants before accepting a connection.

## Select the live protocol

Using an already qualified `supervise_publication` executable:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation 41
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation 41
```

The configuration, actor/reviewer profiles and optional witness profile are the
existing [live actor service inputs](LIVE_ACTOR_SERVICE.md). The generation is
an explicitly supplied nonzero canonical decimal u64 identifying this route
registration. It is independent of policy epochs, native request generations,
journal revisions and clocks. It must match the generation in a proposal's
mounted run address. The host prints that address and the selected request's
poll address when it opens the listener.

For example, native tenant `1`, run `3`, principal `2`, branch `4`, authority `5`
and selected request `7` at registration `41` produce:

| Operation | Address |
|---|---|
| `propose` | `fa://1/run/3@41` |
| `get` after acquiring the ticket | `fa://1/attempt/request-3-2-4-5-7@41` |

These are examples; use the addresses derived from the actual configured scope.
The proposal's numeric request and `idempotency_key` must both be `"7"` in this
example. The complete target, payload, units, policy epoch and native deadline
still undergo original validation and review.

The four ordinary service modes above also accept an explicit bounded request
schedule or single-request precheck and operator sidecars, described below. Unsupported
combinations with multiple peers, receipt-only services, topology, native text or learned
generation refuse before opening their files. The grammar never silently selects
a different protocol. Existing native actor commands retain their current mode.

The selected host grants only its configured tenant's control-ledger reads and
existing actor-intake purpose. It supplies no operator status, experiment
context, precheck disclosure or supervisor capability. Unsupported operations
return ordinary Tower refusals. A caller cannot add authority fields to JSON.

An explicit `--operator-profile OPERATOR_PROFILE` on a single-request Tower
service now provisions a separate authenticated read endpoint. It installs
only the selected native inspection and optional admission-explanation grants;
the actor session stays unchanged. The operator profile can compose with a
precheck sidecar, with distinct sockets and independent budgets. Use
`tower-operator-exchange OPERATOR_PROFILE JSON [JSON ...]` for explicit reads.
See the [live operator contract](LIVE_TOWER_OPERATOR.md) for the closed profile,
disclosure selection, request examples and Stop-first lifetime.

## Check exact policy before a Tower proposal

The single-request forms can explicitly select the existing authenticated
[native precheck service](LIVE_ACTOR_PRECHECK.md):

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --precheck-profile PRECHECK_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --precheck-profile PRECHECK_PROFILE --tower-generation 41
```

Both options follow the positional arguments and may appear in either order.
The same composition is available for `serve-open` and `serve-create-checked`.
Select one of the existing uncached, cached or full-policy precheck profiles;
its original disclosure, socket, policy epoch, query and lifetime limits apply.
`--requests` cannot be combined with this option: disclosure belongs to the
single selected admission and is never renewed at a sequence handoff.

The actor calls `actor-precheck ACTOR_PROFILE PRECHECK_PROFILE PRECHECK_JSON`
through the separate native socket, then sends its actual proposal with
`tower-exchange ACTOR_PROFILE PROPOSE_JSON`. Tower actor grants stay unchanged;
the option does not install an operator or disclosure capability in that Tower
session. Both connections authenticate against their original peer policy.

The original supervisor bootstraps the source once before exposing precheck.
Queries and reconnects share the same finite allowance and source lease. A
passing precheck still requires fresh source acquisition at Tower submission,
independent helper/human review and both publication keys. Checked service modes
also keep their final-publication witness checks. A changed source or witness
can therefore prevent publication after an earlier precheck pass.

Independent Stop is checked before either ingress. Malformed precheck traffic
retires that sidecar without cancelling ordinary Tower work; review and every
service exit retire its disclosure. A recorded request cannot acquire a new
precheck grant during recovery. Socket collisions and invalid combinations
refuse before source acquisition or store creation.

## Publish an explicit request sequence

Add `--requests` to any of the four Tower service modes:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --requests 7,8
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --requests 7,8 --tower-generation 41
```

Both options follow all positional arguments and may appear in either order.
The schedule contains one to 64 distinct nonzero canonical decimal u64 keys in
execution order. Its first key must equal the actor profile's `request`.
Malformed schedules refuse before configuration reads; all scheduled actor,
reviewer and Stop socket bindings are checked before creating the actor socket
or publication store.

The service uses the original [sequential actor lifecycle](SEQUENTIAL_ACTOR_SERVICE.md)
with one authority owner and one Tower session. Tickets and cumulative candidate,
connection and exchange allowances survive every handoff. Rights, journal
capacity, recovery reserves and service deadlines retain their original limits.
Only the currently selected new request can acquire fresh source evidence.
Earlier requests remain available for exact retries and ticket-scoped reads;
conflicting retries still compare the complete retained proposal. Future or
unlisted proposals cannot acquire a snapshot or enter a speculative queue.

Each admitted publication still requires the original helper review, independent
human approval, both publication keys and receipt settlement. Checked modes also
retain their original witness guard. Before advancing, the driver verifies the
prior request's terminal disposition and confirms direct helper children were
reaped. Pending review, dispatching, Unknown and irrecoverably Unknown liabilities
cannot be supplied as a completed prefix. An unfunded proposal is retained as
`NotAdmitted(Limit)` by original admission and does not launch review or refund
earlier spending.

Send the next proposal after the current result is terminal and use its actual
current target version and policy epoch. Independent Stop remains available while
waiting for each request and through the original execution checkpoints; the
service handles at most one complete actor frame per turn. Source failure on a
later request cannot reuse earlier evidence. Opening a recorded schedule follows
the original recovery fence: historical retries do not acquire source evidence,
launch helpers, repeat human approval or open an activation capsule. Every newly
selected request must still satisfy current admission and qualification.

## Exchange explicit request documents

```sh
supervise_publication tower-exchange ACTOR_PROFILE PROPOSE_JSON GET_JSON
```

This actor-only command accepts one to 64 complete Tower documents and emits
one original complete response per document as newline-delimited JSON. It reads
the existing public actor connection profile, validates and freezes the entire
document sequence before connecting, and verifies the connected server's kernel
credentials before sending bytes. Pretty-printed input is canonically encoded
into one frame; identifiers, integer values and payload bytes are preserved.

The client checks each response's bounded JSON syntax, exact common envelope,
schema, operation/address/key correlation and native result profile. The body
remains an opaque observation. A successful client exit means that its explicit
exchanges completed, not that a proposal was approved or an effect executed.
`Pending`, `Unknown`, native refusals and terminal outcomes remain present in
the original returned bytes.

There is no implicit polling, reconnection, replacement key or automatic resend.
Supply `get` documents explicitly. A lost response leaves the original request
identity in force; reconnecting clients retry the same complete proposal to
recover its ticket. A new process or Tower session cannot acquire a ticket by
guessing a poll address. An exact retry after original durable recovery compares
the full retained proposal and does not reread an unavailable evidence source.

The client's elapsed deadline and cumulative byte/I/O-attempt budgets cover all
documents. Short reads/writes and interruptions retain their original progress;
output failure stops before sending another document. Unterminated, oversized,
uncorrelated or unsolicited replies are not emitted as complete results.

## Library integration and scheduling

`tower::publication::peer::PublicationPeerSession` combines an original
`PublicationTower`, fixed host-created `TowerAccess`, the existing `PeerPolicy`,
`ChannelLimits` and a finite connection count. It authenticates every candidate
through the existing Linux UID/GID and optional PID gate before reading bytes.
This identifies the kernel-observed connecting process, not executable code,
a human or a remote identity.

The session exposes `attach`, `status`, `socket_fd`, `disconnect`, `revoke` and
`remaining_exchanges`. The trusted original driver uses
`drive_tower_request_from_file` during selected admission and
`drive_tower_request_observe` during review/recovery. The former acquires fresh
source/time only for a new valid selected proposal; the latter accepts neither
source nor clock and cannot admit unrecorded work. Both retain original helper
maintenance and keep source preparation diagnostics outside actor response bytes.

For a bounded schedule, use `drive_tower_sequence_from_file` and
`drive_tower_sequence_observe` with the current key and completed prefix. Both
check the exact original owner and the original journal's terminal dispositions
before socket I/O. They share the native sequence's predecessor validation and
the same peer session; the completed list is routing data, not settlement evidence.

The socket follows the existing `DriveBudget` / `DriveReport` scheduling contract.
It holds one bounded input frame, one reply and an 8 KiB unread socket buffer.
Input stops at the first LF. A buffered next frame cannot execute until the prior
reply is completely written **and flushed**. Every attempted socket operation
spends an I/O slot, including interruption and would-block. Buffered local work
and pending flushes remain schedulable without waiting for another read event.
The ordinary host admits at most one complete frame per turn, preserving its
independent Stop checkpoints even when a client sends a fragment or pipeline.

| Resource | Bound |
|---|---|
| Tower request frame | Host-selected, at most 256 KiB, excluding LF |
| Session exchanges | Host-selected, at most 4,096 over all reconnects |
| Authenticated connections | Host-selected, at most 1,024 |
| Concurrent sockets | One per session |
| Native response | Original publication limit: 16 KiB default, at most 1 MiB |
| Socket work per drive | Existing byte, frame and I/O-call budgets |

Malformed complete frames spend an exchange; incomplete EOF or over-limit
frames never execute. An encoding failure after native admission also spends
the entered exchange and closes transport without retrying or undoing the
request. Reconnection discards incomplete transport bytes and preserves the
same tickets, disclosure grants and spent lifetime allowance. Revoking ingress
closes transport only; stopping, cancelling or settling effects requires the
original supervisor transition. The peer retains no strong owner reference.

## Qualification scope

Regression sources cover real peer credentials, source-backed submission,
buffered-frame backpressure, lost replies, reconnect limits, foreign owners,
owner loss, actual helper/human publication, observation after recovery without
source, and independent Stop while a client holds a partial frame. Client tests
cover correlation, strict grammar, short I/O and bounded failure handling.

The sequential increment adds thirteen regression functions: five native socket,
six complete service and two option-grammar cases. They cover two real
publications, original cumulative rights, missing later source, historical
recovery, partial-frame Stop, unretired liabilities, foreign owners and schedule
preflight. One existing native budget regression is corrected to match original
pre-review admission. These new and corrected cases have not executed.

Qualified syntax/format and source review are separate from execution evidence.
RCH is unavailable in the current workspace; compilation, Clippy, runtime tests
and the full canonical gate remain pending for this increment. This service
does not claim the complete prospective `fa` CLI, all Tower verbs, learned
publication transport or signed receipt verification.

The precheck composition adds nine authored regression functions, including
the three disclosure profiles, shared work across reconnects, real two-key
publication, changed source/final witness, partial-frame Stop, socket preflight
and recorded-request refusal. Source review and a Rust 2024 syntax/format pass
do not establish execution: RCH has no configured workers and the required
remote command refuses with an unavailable daemon. Compilation and runtime
qualification remain open.
