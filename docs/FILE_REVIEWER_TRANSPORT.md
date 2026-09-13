# Independent reviewer channel for durable two-key publication

## Reference implementation

The previous file-backed host exposed human approval only through an in-process
FileHumanReviewer call. `observed::reviewer::ReviewerConnection` now connects a
separately provisioned reviewer stream to those ORIGINAL durable operations.
This is the bounded second steering wheel in plan section 9.11, not another
congress, automatic evaluator, effect executor, human-identity system or policy.

The host must authenticate and isolate the human connection before attachment.
Actor and helper streams MUST NOT be routed to this interface. Every call also
requires the separately held FileHumanReviewer and the original FileOversight.
Their process-local owner brands are checked before I/O or mutation. An old
channel cannot adopt the newly branded host returned after recovery.

## Exact presentation

Each connection retains ONE original FileHumanRequest. The transmitted packet
contains its complete frozen action (scope, resolved target, payload, units,
policy epoch and deadline), all original per-helper submitted bytes, profiles,
ordered parts, omissions, authorization projections and provenance metadata, and
the native request's reviewer/attempt IDs, control sequence, input revision,
policy generation, creation/expiry ticks and clock-domain identity. It also gives
the recorded Pending/Approved disposition and the offering journal revision.
There is no generated summary, replacement favorable context or new actor prose.

The original journal view codec is reused unchanged. Six private primitive
forwarders expose the existing u32/scope/target encoding within the persistent
family. The journal format and its original reducers are unchanged. Unsupported
private required-witness fields are REFUSED, never silently omitted; the existing
file profile already excludes them. The packet is read-only data, not a decoded
HumanRequest, approval key or authority-bearing context.

A native human request exists only after the original Continue judgment. The
packet does not invent vote tallies or claim to contain a congress transcript
that HumanRequest does not retain. Subsequent fresh evidence and policy checks
remain mandatory at dispatch.

The host supplies a fresh nonzero 32-byte session nonce and must not reuse it
for another offer. The nonce is placed AFTER the full evidence packet and binds
the response to the original request/reviewer/attempt/offer revision. This is an
exact correlation check, not encryption, authentication, entropy generation or a
cryptographic commitment. UI consumers must escape untrusted payload/evidence;
receiving a packet proves neither human attention nor informed consent.

## Authority and acknowledgment

No response bytes are read before the entire packet is written and flushed.
The reviewer can explicitly approve, reject or revoke that one request. No
request substitution, batch, source update, scope expansion, automatic permit,
model decision or dispatch command exists in the protocol.

Approval samples the trusted host clock AFTER the complete response arrives,
journals the changed observation and checks the original input/control/epoch cut
before invoking FileHumanReviewer::approve. The original gate enforces expiry,
pending state and one-use key semantics. Reject/revoke use the original withdrawal
operations without depending on fresh evidence or a current clock. Withdrawal
does not release automatic reservations or settle an unknown external effect.

`ReviewerProgress::Applied` is delivered exactly once to the HOST after the
original durable transition commits. It carries the opaque FileHumanPermit only
for an approval. The small socket receipt contains correlation, decision and
committed revision, NEVER that permit. The actor-side driver still separately
needs its automatic reservation and fresh dispatch observations.

Receipt transmission is a later operation. WouldBlock/Interrupted retain the
write offset; failure does not undo the approval, rerun it or regenerate a key.
`committed()` retains the historical operation even if the socket later fails.
A peer that loses its connection without a receipt has an UNKNOWN response
outcome, not an implicit denial or permission to reapprove. An approved status
is never a key-recovery API. A storage error returns no key or success receipt;
recovery uses the original fence/revocation/cancellation rules.

The connection processes only its first fixed-size decision frame. Additional
stream bytes do not select a second operation and are not read. Offline packet
decoding rejects trailing bytes. There is no promise to detect arbitrary bytes
queued behind a complete valid first frame.

## Bounds and source evidence

Offers are capped at 16 MiB before body allocation; responses are 73 bytes and
receipts are 81 bytes. A step performs at most one 4-KiB read or write and one
final flush. Partial/blocked/interrupted I/O never resets offsets. Original view
and action validation still apply. These are logical byte/work bounds, not total
RSS, wall-clock or filesystem-sync bounds. Caller scheduling must stop abandoned
channels; no hidden runtime, polling thread or deadline extension exists.

Ten new Unix-socket integration tests cover exact presentation and two-key
publication, malformed/wrong bindings, a final response byte arriving at expiry,
changed complete input, rejection/revocation despite unavailable evidence,
acknowledgment loss, incomplete-response EOF, actual staging failure followed by
recovery, foreign/reopened owners, and declared-length/truncation bounds. A
compile-fail example checks that the connection has no dispatch API.

Verification attempted during this edit:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-07 test --locked -p fa-reference --test file_reviewer_transport
bash: rch: command not found
exit: 127
```

The new Rust, socket tests, doctest, formatting and Clippy are UNEXECUTED. No local
compiler fallback was used. No Beads packet, foundational admission or production
gate is closed; this is an implementation-status increment, not execution
qualification. Existing human, journal, endpoint and actor/helper tests and
reducers are unchanged. Plaintext trusted storage, cooperative locking, host
clock correctness, lack of independent anti-rollback protection and operator
responsibility for actual human authentication remain unchanged.
