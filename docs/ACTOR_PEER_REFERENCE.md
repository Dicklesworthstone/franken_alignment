# Linux peer-bound actor sessions

Consumer: the host accepting local actor connections into the existing ActorWire/ActorChannel/UnixActorConnection path. This is scoped perimeter and integration work for FA-011/FA-107, serving the separate-actor/control requirement in plan 9.3 and 17.1. It adds no effect authority, scheduler, cryptographic protocol, foreign function interface or Cargo dependency.

## Kernel-observed admission

PeerSession owns one existing ActorWire and a fixed PeerPolicy. Every new socket is checked using UnixStream::peer_cred before reading any actor frame or moving that wire into a channel. UID and GID must both match; a configured positive PID further narrows the rule. Root and same-shaped numerical actor IDs get no exception. Credentials returned by the API are observations of the actual socket, not fields supplied in an actor command. A client can also call PeerPolicy::verify before sending its own payload.

The implementation uses the safe standard-library peer_credentials_unix_socket API under the repository's nightly toolchain. The feature attribute and public module are Linux-only. Other targets retain the prior uncredentialed transport, explicitly requiring an externally authenticated peer; they do not silently receive a successful peer check.

A credential/setup refusal closes only the candidate socket. It cannot displace an active session, reset its counters, admit a queued command, or destroy its retained ticket visibility. Authentication and nonblocking setup finish before the original wire is moved. Only successful admissions consume the session's finite connection allowance. Allocation aborts remain outside Result-level recovery.

## Reconnection without new rights

One session owns at most one active socket. Explicit disconnect drops transport fragments/replies and retains the original ActorWire, tickets, mailbox and idempotency domain. Every reconnect is authenticated again. Exact retries use the original key; altered proposals conflict, and exhausted original mailbox capacity is not replenished. The maximum successful connection allowance is 1,024, configurable downward. Existing frame, reply, I/O and actor-intake limits remain in force. Failed authentication attempts require caller-side scheduling/rate limits and are not counted as successful connections.

revoke permanently withdraws socket ingress for this session and is idempotent. It does not revoke an effect permit, cancel already accepted requests, declare an uncertain effect unexecuted, or refund resources. Those operations remain with the original supervisor and endpoint. This owner exposes no port/wire/policy-widening or supervisor getter. Borrowed descriptors are scheduler capabilities and must not be given to an actor.

## Accepted sockets and original effect execution

UnixPeerListener owns an already-bound UnixListener and the same PeerSession. The host provisions the socket namespace and permissions outside this module. Construction sets nonblocking mode; if it fails, ListenerSetupFailure returns both original owners. Each accept_once performs at most one accept syscall and the original peer check. WouldBlock is Idle, not proof that no effects are pending. Interrupted returns to the scheduler instead of entering a retry loop. A rejected candidate is closed without parsing its buffered request or sending a response. A busy listener does not evict its current peer.

The host can borrow listening/connected descriptors, observe the original transport scheduling status and drive the bounded channel. No thread, accept loop, new JSON schema or automatic review is introduced. Moving the session to a newly provisioned listener preserves any active connection, fixed policy, revocation and successful-connection count. It cannot renew an exhausted allowance. Listener revocation closes both listening and active ingress; socket paths are never automatically unlinked or rebound.

Authentication supplies access to the original proposal mailbox, not permission to publish. Actual requests still pass the original policy and whole-input congress, normal effect permit, optional separate human key, endpoint preconditions and one-use accounting. Peer policy is provisioned together with one ActorWire; incoming identity strings never select a different scope or ActorPort. Responses remain the original redacted Knowledge projection and contain no helper contracts, verdicts, exact policy values or live permits.

An actor can reconnect with an effect already unknown. Its original cancellation request cannot refund that effect. Nonexecution is established only by the registered endpoint's terminal receipt, including sealing against delayed delivery; an existing execution receipt wins. Revoking ingress leaves those supervisor-side recovery operations available. The filesystem integration retains the same supervisor and endpoint recovery handle, observes a fresh clock, confirms a new dispatcher fence, and reconciles the original publication without a second execution or actor request. This is not recovery after losing the entire authority process.

## Scope of identity

SO_PEERCRED is a connection-creation credential snapshot, not proof of executable integrity, a particular human, later credential changes, or the process currently holding a transferred descriptor. PID matching is not a non-reusable process-incarnation token. UID/GID-only mode admits every process connecting under that identity, and a privileged or compromised host can provision the wrong ActorPort. Namespace mapping, account separation, descriptor inheritance/transfer, authenticated policy configuration, socket namespace permissions and the rest of the filesystem/network perimeter remain host obligations. This module does not sandbox an actor or prove a complete mediation cut.

## Verification status and increments

The first increment adds nine Linux integration-test functions and one compile-fail doctest. They use actual socket pairs to cover matching credentials, each mismatched credential, no command intake on rejection, active-connection preservation, duplicate request recovery, truncated input, lifetime quotas, original-reservation cancellation after ingress revocation, and client-side verification. Helper verdicts use the existing explicit reference fixture, not live inference. The original transport drive loop and tests are unchanged apart from a shared internal post-setup constructor.

The second increment adds eight scenario tests, one subprocess-entry fixture and another compile-fail doctest. Named Unix listeners accept an explicitly allowed child PID and refuse a same-account different process without consuming the successful-connection allowance. The child independently verifies its parent before writing. Other cases exercise actual reference congress-to-publication, mandatory human approval, no duplicate publication after an exact retry, ingress revocation with a disclosed unknown effect, original-key recovery followed by nonexecution sealing, listener replacement without new capacity, and real temporary-file publication/reopening/reconciliation. The subprocess-entry fixture is plumbing, not separate authentication evidence. OS operations in these sources have not been executed here.

In total there are seventeen scenario-test functions, one subprocess-entry fixture and two compile-fail doctests. Rust compilation, rustfmt, Clippy and all tests are UNEXECUTED: this editing environment has no configured Rust/RCH runner. No bead is closed, no gate is promoted and historical execution receipts do not qualify this source. Real operating-system adversarial campaigns, native runtime scheduling, durable controller identity and full credential isolation remain outstanding.
