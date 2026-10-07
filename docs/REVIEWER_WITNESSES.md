# Native read witnesses in independent human review

## Consumer and defect

`ReviewerConnection` and `ReviewerClient` now transport policy-derived action
read witnesses through the original `ReviewPacket`. The old offer codec refused
every action with a nonempty `required_witnesses` list, even though the original
policy compiler, human request and two-key dispatch support those dependencies.
This blocked external review for the existing exact-value policy fixture and for
real policies depending on values, absent keys or empty ranges.

The fix is a lossless presentation extension, not removal of dependencies, a new
approval role, a reconstructed judgment or an alternate publication path. Native
human approval and automatic authorization still consume the original request and
action; native dispatch still checks the current snapshot and both original keys.
The transport never turns absent evidence into an absence witness.

## Wire compatibility

Witness-free offers retain the exact `FAHRVW\0\x01` format and bytes. A nonempty
list uses `FAHRVW\0\x02`. After action units and before helper views, v2 adds a
big-endian u32 witness count followed by the ordered records:

| Tag | Record following the one-byte tag | Meaning |
| --- | --- | --- |
| 0 | u64 key | The exact key was absent. |
| 1 | u64 key, u32 byte length, exact value bytes | The key had this value; an empty value is not absence. |
| 2 | u64 start, u64 end | The half-open range `[start, end)` was empty. |

The existing bounded journal reader/writer primitives frame all fields. The
native limits apply independently of the 16 MiB offer ceiling: at most 64
witnesses and 65,536 aggregate value bytes. The decoder checks count and remaining
value budget before allocating records/values. Empty or reversed ranges, unknown
tags, oversized lengths, truncation and trailing bytes refuse. Records are not
sorted or deduplicated; exact frozen-action equality includes their order.
Canonical re-encoding rejects a v2 empty-list alias. Old clients reject v2 at its
header instead of silently reading an action without dependencies. Decision and
receipt frames are unchanged; session uniqueness and original request/reviewer/
attempt/revision binding remain mandatory.

## Disclosure and authority

Read-witness values are private action data. They go only to the independently
provisioned human-review channel authorized for this request. Linux peer checking
still precedes transmission; generic streams still require the trusted host to
supply an authenticated, isolated channel. This is not a cryptographic protocol,
remote authentication scheme or assertion of human understanding. Nothing adds
these values to actor channels or projected helper inputs.

The decoded packet is read-only presentation. It neither mints a human permit nor
replaces the original request. An explicit reviewer choice, an acknowledged native
reviewer operation, a separately earned automatic key, and current native
execution checks are still required. A changed exact value, insertion into a
negative key/range, different semantic epoch or incomplete snapshot remains a
refusal at dispatch. An unrelated snapshot change can still pass the original
witness check.

## Implementation and verification status

Added five bounded-codec unit tests and six native integration tests. Positive
coverage includes original policy compilation and request capture, exact full
action/view round trips, byte-stable legacy offers, fragmented socket delivery,
explicit human decision, original two-key dispatch, publication and reconciliation.
Negative cases include all witness variants and limits, invalid framing, version
alias/downgrade, partial presentation, and six current-snapshot invalidations.
Existing Linux peer and subprocess tests exercise the same updated offer path
without replacing their required-witness policy or relaxing their assertions.

The required command was attempted in this environment:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

It could not start: `rch: command not found` (exit 127). Compilation, Rust test
execution, rustfmt and Clippy for this change remain unexecuted. No production
gate, deployment assurance or bead closure is claimed. This extends the existing
L2 evidence presentation used by the L4 human-review boundary; it does not change
the plan's authority semantics, dependency universe or native journal format.
