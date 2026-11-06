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

Tower selection is accepted only by the four ordinary single-request service
modes above. Unsupported combinations with request sequences, multiple peers,
precheck sidecars, receipt-only services, topology, native text or learned
generation refuse before opening their files. The grammar never silently selects
a different protocol. Existing native actor commands retain their current mode.

The selected host grants only its configured tenant's control-ledger reads and
existing actor-intake purpose. It supplies no operator status, experiment
context, precheck disclosure or supervisor capability. Unsupported operations
return ordinary Tower refusals. A caller cannot add authority fields to JSON.

An integrating host may provision a separate operator connection with an
explicit [native inspection grant](TOWER_PUBLICATION.md#inspect-the-original-live-owners-situation).
The generic peer/client can carry its `situation` observation. The ordinary
service above does not issue that grant or enable the operator role.

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

Qualified syntax/format and source review are separate from execution evidence.
RCH is unavailable in the current workspace; compilation, Clippy, runtime tests
and the full canonical gate remain pending for this increment. This service
does not claim the complete prospective `fa` CLI, all Tower verbs, learned
publication transport or signed receipt verification.
