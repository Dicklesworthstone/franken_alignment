# Actor-side protocol consumption

Consumer: an external agent dispatcher using the existing actor submit/poll/cancel
protocol (plan 17.1; FA-107). Responses remain Knowledge observations, never
execution permits. This adds no authority, wire command, parser fallback,
dependency, runtime, automatic approval or remote endpoint.

## Strict responses and one exchange

`decode_response` parses the existing bounded response schema. It preserves all
u64 identifier bits, rejects duplicate/unknown fields, invalid versions and
cross-request bases, and distinguishes remote refusals from invalid responses.
Known execution cannot originate at intake; a Pending response cannot name a
different request. No approved/authorized value is admitted as a terminal outcome.
The existing command encoder and server response encoder remain unchanged.

`ActorExchange` sends exactly one encoded newline-terminated command on an
explicitly supplied stream. Writes and flushes complete before any response read.
Partial writes, blocked flushes and interrupted reads retain their offsets. One
step performs at most one read or write, and a final flush. The full correlated
newline response must parse before any observation is returned. Truncated replies,
unknown variants, wrong IDs and trailing frames fail the connection; none implies
nonexecution. A response is yielded once and retained as historical inspection.

A transport failure closes the stream and records the original request ID, bytes
accepted by Write and whether any write was attempted. This conservative delivery
uncertainty is distinct from the server's effect outcome. Dropping an exchange
cannot cancel a request, obtain a refund or authorize a replacement effect.
Only a complete exchange returns a reusable stream. Preflight failures return the
untouched stream and spend no exchange allowance.

`ClientIoBudget` counts actual bytes, every I/O call and admitted exchanges.
Interrupted, WouldBlock, EOF and flush calls consume the same persistent call
allowance; they cannot form an unbounded hidden retry loop. Work is never refunded.
A generic stream needs an independently enforced nonblocking/bounded I/O contract;
a call quota is not a latency bound. The client does not authenticate the peer.
The trusted caller must bind the connection to its intended authority domain.
Request correlation is not cryptographic replay protection.

## Verification boundary

Four response tests and five exchange tests cover independent literal vectors,
original-encoder roundtrips, all observation/error variants, full-width IDs,
malformed/injected fields, cross-request provenance, partial reads/writes, blocked
flushes, interruption exhaustion, EOF, trailing frames and exact/one-over budgets.
A compile-fail example excludes effect-permit access through the exchange.

Rust compilation, formatting, Clippy, tests and doctests have not run. RCH is
unavailable in this editing environment; source review and publication do not
qualify executable behavior. No historical results, Beads states or production
gates are promoted. Existing authority, journal and actor transport code retains
its original behavior. Authentication, independent watchdogs and hostile-process
containment remain outside this reference client.
