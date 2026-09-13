# Independent reviewer client and integrated durable control path

## Implemented consumer

`observed::reviewer::client::ReviewerClient` completes the peer side of the
ReviewerConnection introduced in FILE_REVIEWER_TRANSPORT.md. It decodes the
original complete offer, waits for an explicit independent human UI action,
transmits one frozen decision, and distinguishes an acknowledged operation from
an unknown response outcome. It contains no FileOversight, FileHumanReviewer,
automatic permit or human approval key. No inference engine, UI framework,
listener, authentication service, reconnect loop or second runtime is added.

This implements transport/client support for the plan section 9.11 human second
key. It is source implementation, not a claim of native/foundation admission,
authenticated human evaluation or production qualification.

## Independently pinned audience and explicit choice

The caller supplies ReviewerExpectation with the expected reviewer ID, complete
scope and clock-domain identity. These values must be configured independently,
not copied from the received offer. The client checks all of them after exact
packet decoding and before exposing a packet or enabling a decision. Wrong
reviewer, tenant/principal/run/branch/authority/purpose or clock-domain input is
refused; incoming data cannot select its own expected audience.

The existing lossless ReviewPacket retains the original action and every helper
view, with full submitted bytes, ordered parts, input profiles, omissions,
authorization projections and provenance. The client exposes this immutable
presentation only after the original parser and pinned audience checks succeed.
No generated summary replaces the original evidence. Debug output excludes the
payload and evidence contents. UI code must still render untrusted bytes safely.

Reading a complete packet changes the client to NeedsDecision. Repeated step
calls at this point do nothing except report that state: no default Allow,
timeout consent, model decision or inferred human choice exists. The host UI
must call respond with an explicit Approve, Reject or Revoke. That choice is
frozen immediately and cannot be edited or resent even before the first write.
A packet reporting an already Approved request permits only Revoke in this
client; Approved is never a route for recovering a lost approval key.

## Durable result versus response outcome

The client sends the existing fixed-size, offer-bound decision once. The server
continues to use the original durable FileHumanReviewer operations. An opaque
FileHumanPermit goes only to the supervising host after a successful approval
transaction; it never appears on the client socket. The subsequent socket
receipt names the exact offer, decision and committed journal revision.

The client accepts a receipt only when both its full binding and decision match
the retained packet and frozen choice. Completion is an acknowledgment of that
historical reviewer operation, NOT dispatch permission, evidence freshness or
proof of external publication. Dispatch continues to require the original
automatic permit, human key, current observations and unchanged authority checks.
The concurrently added optional first-publication guard is not disabled or
modified by this client; the legacy integration fixtures do not enable it.

After ANY attempted response write, outcome_unknown remains true until the
matching receipt arrives. This is deliberately conservative for partial writes,
WouldBlock, Interrupted and terminal transport errors. An EOF before a receipt
must not be interpreted as denial, nonexecution or permission to issue another
grant. Terminal failures latch and close the stream; no automatic reconnect,
resubmit or key regeneration exists. A previously decoded packet remains
historical inspection data and cannot make a failed client respond again.

## Bounded I/O and scheduling

Offer headers are read and checked before allocating the declared body. The
existing complete-frame limit is 16 MiB. Each step performs at most one read or
write, capped at 4 KiB, and at most one final flush. The decision is 73 bytes and
the receipt 81 bytes. Partial reads/writes and transient errors retain exact
offsets; an interrupted flush does not resend already-written bytes.

interest reports Readable, Writable, HumanDecision or Finished for an embedding
host's scheduler. There is no hidden polling thread or deadline extension. These
are logical byte and operation bounds, not total RSS, blocking filesystem latency
or wall-clock guarantees. Generic streams require the caller's nonblocking and
bounded-I/O contract; from_unix configures the provided UnixStream nonblocking.

## Integration and negative source scenarios

Six public tests in file_reviewer_client.rs use the existing FileSupervisedDriver
fixture and real Unix socket pairs rather than replacing the actor, helper,
congress, human or endpoint implementations:

1. Original actor submission and helper review reach an independent client that
   waits for an explicit choice without reserving budget. The returned original
   human key then drives original dispatch, publication and reconciliation.
2. An acknowledged human approval cannot override a later changed provider input:
   the original driver refuses with no reservation, charge or execution.
3. Independently mismatched reviewer, scope and clock expectations are refused
   before presenting a packet or enabling any decision.
4. A lost receipt after actual durable approval remains Unknown at the client;
   the host's historical Approved state is not rolled back or reissued.
5. An oversized declared frame is refused before body decoding or any choice.
6. Real sockets wrapped with short reads/writes and injected WouldBlock and
   Interrupted errors, including interrupted flushes, retain exact data and
   one decision. Per-step counters assert the stated I/O operation bounds.

One internal test in reviewer/faults.rs runs the original approval transition
through all five existing storage barriers: stage, write, file sync, rename and
directory sync. It uses real reviewer response frames and an unchanged host
clock tick so the selected failure is specifically the approval transaction.
It distinguishes last acknowledged host state from the actual canonical file:
a directory-sync failure can leave Approved visible on disk, while no key or
success receipt is returned. Reopening must revoke that historical approval,
cancel the undispatched attempt and refuse resurrection through a new reviewer
role. These are API-error scenarios, not hardware power-cut experiments.

Together with the first increment, this adds seventeen test functions and two
compile-fail examples. The new examples check that neither the host connection
nor the reviewer client exposes an effect-dispatch or key-recovery API. Existing
human, worker, actor, driver, storage and endpoint test bodies are unchanged.

## Verification status

Both commands below were attempted in this editing session:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-07 test --locked -p fa-reference --test file_reviewer_client
RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-07 test --locked -p fa-reference --lib action::consequence::delivery::persistent::observed::reviewer::faults
```

Each failed before compilation because rch is absent, exit 127. The prior
file_reviewer_transport attempt failed the same way. No local compiler fallback
was used. All new Rust compilation, tests, doctests, formatting, Clippy and
revision-bound qualification remain UNEXECUTED. No Beads packet or gate is closed.
These scenarios are test sources, not passing execution evidence.

## Unchanged trust boundary

This is a plaintext reference protocol for a preauthenticated private channel.
The embedding host remains responsible for actual human authentication,
confidentiality, integrity, UI isolation, safe evidence rendering and correct
routing: actor/helper endpoints MUST NOT be attached as reviewer peers. Audience
pinning and an exact nonce comparison do not authenticate a person or prove that
the evidence was read. Nonces are caller-provided, nonzero and must be unique per
offer; this module does not generate entropy or implement signatures.

Original trusted storage, cooperative owner locking, clock correctness,
anti-rollback limitations, human-context deduplication and one-use key semantics
are unchanged. Human withdrawal does not refund reservations or prove that an
already-dispatched effect did not execute. The original stop/reconciliation
protocol remains responsible for those outstanding obligations.
