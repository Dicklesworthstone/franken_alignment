# Actor wire reference profile

Consumer: an existing agent tool dispatcher implemented outside Rust, connected by a host to one `ActorPort`. This extends the actor gateway for plan sections 9.3 and 17.1, FA-107 / FI-A15 / FI-A16. It does not introduce a production `fa` command, an authenticated transport, a second runtime, or a new authority ledger.

## Bounded exact messages

`ActorWire::exchange` accepts one JSON document using the existing strict parser. The version-1 operations are the existing port's `submit`, `poll`, and `cancel`. Unknown or duplicate fields, scope/permit/verdict injection, unsupported operations and malformed binary encodings refuse before the port is touched. The maximum document is 132,096 bytes, nesting is bounded to four, and the parser permits at most 64 items. Payloads are lowercase hexadecimal with a 65,536-byte decoded limit. All u64 fields use canonical unsigned decimal strings, including request IDs, target components, resource limits and deadlines; JSON numbers must not silently round IDs in a JavaScript client. Only `version` is a JSON number. Explicit initial policy epoch `"0"` is valid; request and target IDs, units and deadline must be positive.

```json
{"version":1,"operation":"submit","request":"42","target":{"adapter":"1","object":"1","contract_version":"1","expected_version":"1","generation":"1"},"payload_hex":"7075626c697368","units":"16","deadline":"100","expected_policy_epoch":"0"}
```

Polling and cancellation use exactly `version`, `operation` and `request`. A successful response contains the existing `Knowledge<ActorOutcome>` projection, never a permit or a bare safety Boolean. Cancellation acceptance is not confirmation of cancellation-before-dispatch. Errors have fixed actor-safe reason codes and never interpolate submitted strings or privileged policy diagnostics. Replies contain no payload, committee membership, votes, secret questions, control sequence or effect credentials. Reply encoding is bounded to 512 bytes.

The host must bind a connection to the correct preprovisioned port. No request selects a tenant, principal, run, branch, purpose or port. The wire handler retains at most 128 actual branded tickets. Unknown ticket IDs are uniformly withheld. Retain the handler across reconnects to retain ticket visibility; constructing a fresh handler on the same port requires exact proposal resubmission to recover its ticket. The underlying port performs lifetime deduplication, so reconnecting cannot make an existing request new. Dropping the entire supervisor/port remains outside any durable deduplication claim.

## Implemented verification source, not execution evidence

Public tests compare manual request/response goldens, all 256 byte values, maximum payload size and full-width u64 values; reject ambiguous and privilege-bearing schemas; exercise exact retries and reconnect ticket recovery; and drive the actual reference congress, permit, dispatch and endpoint to one publication. They check that a review/permit remains Pending, dispatch becomes Unknown, a real endpoint receipt establishes Executed, and forbidden private strings never enter wire replies. A compile-fail example excludes broker access through the wire handle.

September 11, 2026: source added, with no new dependency or crate. Rust, rustfmt, Cargo and RCH are absent from this editing environment. These tests have not been compiled or executed, and no production or bead qualification follows. Historical execution receipts remain historical. This profile is an explicit reference protocol, not an implementation of all production wire/address/Knowledge requirements in FA-133/FA-134.
