# Sequential actor submissions on one authenticated connection

The existing `actor-submit` command now accepts multiple original submission
files, in the order selected independently by the supervisor's `--requests`:

```text
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --requests 7,8
supervise_publication actor-submit ACTOR_PROFILE first.json second.json
```

The server and client commands run in their separately provisioned processes.
`WITNESS_PROFILE` may be the existing explicit joint-publication wrapper. Each
submission still needs a fresh helper review, independent human approval and
current native publication checks. The client cannot select or change the
server's schedule, open its journal, supply a reviewer role, activate credibility,
change policy or renew an expired qualification.

## Frozen input, bounded work, and original results

A one-document invocation preserves the original behavior. A multi-document
invocation admits 2 through 64 distinct Submit documents, with the first request
matching the actor profile. All documents are read and decoded before connecting;
a malformed later file cannot cause the first request to be submitted. The sum
of original file bytes, including JSON whitespace, is at most 2 MiB, and every
file retains the existing single-frame bound. Parsing uses the original codec.
Targets, expected versions, policy epochs, payloads, units and deadlines are
never edited after reading. A client-generated sequence is not a grant: the
independently selected service still decides which request can be admitted.

One connected server is authenticated before any actor bytes are sent. Every
Submit and Poll then shares one original `ClientIoBudget` and one elapsed-time
limit across the complete sequence; there is no per-document reset. The same
exchange implementation handles single and multiple requests. Pending and
outcome-unknown responses cause bounded polling of the CURRENT request's own
identifier, not resubmission and not polling the first request by mistake.
File reads, socket setup and output are synchronous reference operations; the
elapsed checks do not claim to preempt a blocked operating-system operation.

Each terminal response is the original `WireResponse`, written as one JSON line.
The client advances only after an executed result AND successful output flush.
A refusal, nonexecution, unresolved result, transport/protocol error, timeout or
exhausted work allowance stops the sequence. An output failure can follow real
execution; later documents are not sent and no previous effect is refunded.
There is no automatic reconnect, resend, target/version repair or new request ID.
A deliberately preplanned later document can therefore become stale; it is not
silently repaired. Inspect original results and use the ordinary proposal path
for a genuinely new request.

All-recorded server schedules remain receipt-only on reopening. Supplying the
unchanged original documents can retrieve prior executions with source/helper/
reviewer/capsule access absent; it does not create new effects or restore old
approval keys. A client failure is not proof of nonexecution. Retain original
request bytes for the existing reconciliation path and use the independently
provisioned stop channel when terminating a server still waiting for later work.

## Implementation and validation boundary

This increment reuses the native actor wire, authenticated peer connection,
sequential service and original journal. No new dependency, runtime, authority
ledger, wire operation or source observation is introduced. The consumer is the
runnable actor client for the previously restored bounded service.

Seven new Rust regression tests are authored. Actual temporary journals,
child-helper and independent reviewer sockets exercise two effects over one
connection, history-only retrieval, a shared exchange ceiling, refusal of an
unchanged stale second target, output loss after execution without a later
submission, all-document and aggregate-byte preflight, and connected-server
credential rejection without protocol disclosure. Synthetic helper responses
are not evidence of detector accuracy. Existing assertions remain intact.

The required RCH example-test and xtask commands were attempted and could not
start because `rch` is unavailable (exit 127). Compilation, formatting, Clippy
and Rust tests remain UNEXECUTED. Source/hash/lexical checks do not substitute
for these gates, and no bead or production qualification is closed.
