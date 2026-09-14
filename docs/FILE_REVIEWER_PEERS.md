# Kernel-bound reviewer connections

## Connected capability

The Linux `reviewer::peer` module checks the connected process before any reviewer-protocol I/O. It reuses the existing `actor_peer::PeerPolicy` and `PeerCredentials::observe` implementation rather than interpreting in-band identity fields or adding a second credential reader. UID and effective GID must both match; a configured PID narrows them further. There is no root exception or authentication downgrade.

`VerifiedReviewerSocket::verify` owns the exact socket it checks and configures it nonblocking. It has no unchecked constructor, clone, mutable credential setter or raw-stream extraction. `into_connection` feeds that socket to the original `ReviewerConnection` with the separately held `FileHumanReviewer` and original request. `into_client` feeds it to the original `ReviewerClient`, which still validates its independently configured logical reviewer, scope and clock domain. Neither operation sends a byte or issues a key.

A process credential check does not approve an action. The client still stops at NeedsDecision, an explicit decision still uses the original nonce-bound packet, and the supervising host still receives a key only after the original durable transition commits. Both the automatic and human keys remain necessary for dispatch. Changed evidence, original expiry, revocation and owner bindings are unchanged.

## Bounded admission

`ReviewerPeerAdmission` pins one rule and a nonzero candidate quota no larger than 1,024. It accepts at most one peer. Wrong candidates are closed without consuming a human request or touching the journal; the host can accept another candidate under the same rule and original deadline. After success, later candidate sockets are closed without displacing the accepted socket. Credential and socket-setup failures consume a candidate slot. Exhaustion refuses; it never selects the unchecked transport.

The embedding host owns nonblocking listener scheduling and a deadline. A hostile process can exhaust the candidate quota or listener backlog, causing denial of service, not consent. A new admission gate is an explicit host action, not an automatic retry in this component.

## Identity and trust boundary

The observed values are Linux connection-time credentials, not proof of a human's identity, presence, judgment, executable image or per-message sender. Descriptor passing and PID reuse retain the limitations of the original peer implementation. Socketpair credentials identify creation-time peers; do not mistake a parent's inherited descriptor for a separately connecting child's identity. Tests use an actual child connect for PID separation.

UID/GID-only rules admit all connecting processes with that credential pair. To separate a same-account actor from a reviewer process, supply an independently pinned PID or isolate their accounts/OS authority. A numeric reviewer ID in the packet does not authenticate its sender. The host, configuration, socket namespace and relevant ancestors remain operator-controlled. This does not add encryption, remote authentication, a sandbox or hostile-root resistance.

The existing generic and `from_unix` constructors remain explicitly unchecked compatibility APIs. Merely using Unix sockets does not activate peer verification. Checked callers must enter through VerifiedReviewerSocket and independently configure their peer expectation.

## Source tests and execution status

Six public scenario tests plus one inert subprocess-entry fixture exercise both checked directions through real sockets and the original authority: successful full-input/two-key publication, each UID/GID/PID mismatch with a valid decision queued beforehand, invalid server credentials, wrong logical audience and changed evidence after connection, one-success/quota behavior, and a same-account parent rejected while the pinned connecting child succeeds. One compile-fail example checks that the verified socket cannot be cloned. The child generates a synthetic explicit test decision; it is not a human reliability experiment.

The required command was attempted during this increment:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_reviewer_peer
```

It failed before compilation with `rch: command not found`, exit 127. New Rust compilation, tests, formatting, Clippy and revision-bound qualification remain unexecuted. No Beads or production gate was closed. No dependency, unsafe code, original reviewer transition or existing test assertion was changed.
