# Linux peer-bound actor sessions

Consumer: the host accepting local actor connections into the existing ActorWire/ActorChannel/UnixActorConnection path. This is scoped perimeter and integration work for FA-011/FA-107, serving the separate-actor/control requirement in plan 9.3 and 17.1. It adds no effect authority, scheduler, cryptographic protocol, foreign function interface or Cargo dependency.

## Kernel-observed admission

PeerSession owns one existing ActorWire and a fixed PeerPolicy. Every new socket is checked using UnixStream::peer_cred before reading any actor frame or moving that wire into a channel. UID and GID must both match; a configured positive PID further narrows the rule. Root and same-shaped numerical actor IDs get no exception. Credentials returned by the API are observations of the actual socket, not fields supplied in an actor command. A client can also call PeerPolicy::verify before sending its own payload.

The implementation uses the safe standard-library peer_credentials_unix_socket API under the repository's nightly toolchain. The feature attribute and public module are Linux-only. Other targets retain the prior uncredentialed transport, explicitly requiring an externally authenticated peer; they do not silently receive a successful peer check.

A credential/setup refusal closes only the candidate socket. It cannot displace an active session, reset its counters, admit a queued command, or destroy its retained ticket visibility. Authentication and nonblocking setup finish before the original wire is moved. Only successful admissions consume the session's finite connection allowance. Allocation aborts remain outside Result-level recovery.

## Reconnection without new rights

One session owns at most one active socket. Explicit disconnect drops transport fragments/replies and retains the original ActorWire, tickets, mailbox and idempotency domain. Every reconnect is authenticated again. Exact retries use the original key; altered proposals conflict, and exhausted original mailbox capacity is not replenished. The maximum successful connection allowance is 1,024, configurable downward. Existing frame, reply, I/O and actor-intake limits remain in force. Failed authentication attempts require caller-side scheduling/rate limits and are not counted as successful connections.

revoke permanently withdraws socket ingress for this session and is idempotent. It does not revoke an effect permit, cancel already accepted requests, declare an uncertain effect unexecuted, or refund resources. Those operations remain with the original supervisor and endpoint. This owner exposes no port/wire/policy-widening or supervisor getter. Borrowed descriptors are scheduler capabilities and must not be given to an actor.

## Scope of identity

SO_PEERCRED is a connection-creation credential snapshot, not proof of executable integrity, a particular human, later credential changes, or the process currently holding a transferred descriptor. PID matching is not a non-reusable process-incarnation token. UID/GID-only mode admits every process connecting under that identity, and a privileged or compromised host can provision the wrong ActorPort. Namespace mapping, account separation, descriptor inheritance/transfer, authenticated policy configuration, socket namespace permissions and the rest of the filesystem/network perimeter remain host obligations. This module does not sandbox an actor or prove a complete mediation cut.

## Verification status and increment

The first increment adds nine Linux integration-test functions and one compile-fail doctest. They use actual socket pairs to cover matching credentials, each mismatched credential, no command intake on rejection, active-connection preservation, duplicate request recovery, truncated input, lifetime quotas, original-reservation cancellation after ingress revocation, and client-side verification. Helper verdicts use the existing explicit reference fixture, not live inference. The original transport drive loop and tests are unchanged apart from a shared internal post-setup constructor.

Rust compilation, rustfmt, Clippy and all tests are UNEXECUTED: this editing environment has no configured Rust/RCH runner. No bead is closed, no gate is promoted and historical execution receipts do not qualify this source. Real operating-system adversarial campaigns, native runtime scheduling, durable controller identity and full credential isolation remain outstanding.
