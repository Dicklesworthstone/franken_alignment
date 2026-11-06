# Bounded JSON requests through the original Tower owners

An integrating host can now accept one `fa.tower_request/1` JSON document through
`ReferenceTower::exchange` or, on Unix, `TowerRouter::exchange`. The request codec
also exposes `decode_request` and `encode_request` for integrations that need the
existing typed interface. Responses use the original sixteen-field envelope and
the selected backend's original result encoder.

The consumer is an agent connection whose host already owns a configured Tower
and authenticates its caller. The host supplies `TowerAccess` separately on each
call. No request field can supply an operator role, purpose grant, disclosure,
experiment context, owner or permission. This is a document interface; the host
still owns transport framing, authentication, scheduling and elapsed deadlines.

## Request schema

The root has exactly six required fields. `arguments` and `idempotency_key` use
explicit JSON `null` when absent; missing, duplicate or extra fields are rejected.
Field order and JSON whitespace do not matter.

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-1",
  "address": "fa://7/run/8@3",
  "verb": "get",
  "arguments": null,
  "idempotency_key": null
}
```

`operation_id` and a non-null key are nonempty UTF-8 strings of at most 128 bytes.
The address must use the existing canonical `fa://` spelling and fit in 256
bytes; display aliases and alternate generation spellings are rejected. Every
integer argument is an unsigned canonical decimal **string**, including target
fields, epochs, elapsed ticks and collection limits. This retains all `u64`
bits in clients whose JSON number representation cannot do so. Signs, leading
zeros, fractional/exponent forms, numeric JSON values and overflow are rejected.

| Verb | `arguments` |
|---|---|
| `get`, `situation`, `explain`, `why-held`, `replay-plan` | `null` |
| `related` | `null` or `{"relation":null}`; a relation string selects one existing relation kind |
| `tail` | `null` or `{"since_seq":"0","max_events":"64"}` |
| `next` | `null` or `{"max_cost":"8"}` |
| `replay` | `null` or `{"interventions":[...]}`; requires a nonempty idempotency key |
| `propose` | `{"request":"1","proposal":{...}}`; requires the same canonical key and an explicit nonzero address generation |
| `precheck` | `{"query":"1","proposal":{...}}` |
| `capabilities` | `{"query":"1","expected_policy_epoch":"0"}`; discovery support depends on the selected native profile |
| Other registered verbs | `null`; the original dispatcher returns `CapabilityUnsupported` |

The six relation strings are `closure_strong_ref`, `closure_weak_ref`,
`provenance`, `thought_graph_edge`, `support_set_member` and `annotation_target`.
`tail` accepts 1 through 128 events at the syntax boundary; the host's actual
page limit can be smaller. Omitted optional read arguments retain their original
typed defaults. An unknown or retired verb is not silently treated as a read.

### Native proposals

The proposal object preserves the original native actor schema. All five target
fields, units and deadline must satisfy `ActorProposal::validate`. Epoch zero is
syntactically valid; only the original owner can decide whether an epoch is
current. Payloads use even-length lowercase hexadecimal, retaining arbitrary
binary bytes without Unicode conversion.

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "submit-1",
  "address": "fa://7/run/8@3",
  "verb": "propose",
  "arguments": {
    "request": "1",
    "proposal": {
      "target": {
        "adapter": "1",
        "object": "9",
        "contract_version": "1",
        "expected_version": "1",
        "generation": "1"
      },
      "payload_hex": "68656c6c6f",
      "units": "5",
      "deadline": "100",
      "expected_policy_epoch": "0"
    }
  },
  "idempotency_key": "1"
}
```

These identifiers illustrate syntax; the host must provision the actual mount,
target, policy and grants. Parsing never creates a native admission snapshot or
publication key. Valid `propose` and `precheck` requests enter exactly the original
native paths described in [TOWER_PUBLICATION.md](TOWER_PUBLICATION.md). An exact
submission retry retains original content binding, tickets and durable one-shot
behavior. A repeated precheck label spends another original disclosure query.

### Branch replay edits

The closed intervention forms use the original edit tags:

```json
[
  {"kind":"payload","payload_hex":"6e6577"},
  {"kind":"target","target":{"adapter":"1","object":"9","contract_version":"1","expected_version":"1","generation":"1"}},
  {"kind":"units","units":"2"},
  {"kind":"key","key":"5","expected_hex":null,"replacement_hex":"76616c7565"}
]
```

A key's `null` expected value names observed absence; it is not a wildcard.
`null` replacement removes that observed value. Empty strings represent empty
bytes, separately from absence. At most 64 edits and 65,536 combined decoded edit
bytes are accepted, using the original replay resource validator. The native
experiment still decides whether edits are registered, nonconflicting and valid
for its actual baseline. A syntactically valid but semantically rejected edit
retains the original admitted-work accounting.

Replay requires the existing host-owned branch context, complete access and an
exact retry key. It cannot route a branch to a production publication owner.
Successful replay records the original branch outcome and never grants effect
authority. See [TOWER_REPLAY.md](TOWER_REPLAY.md).

## Admission, uncertainty and resource limits

The entire document is parsed and validated before route lookup or backend work.
The strict parser rejects malformed UTF-8, invalid escapes and unpaired surrogates,
duplicate keys at every depth, concatenated documents and trailing non-whitespace
bytes. Closed nested objects reject hidden authority fields as well as unknown
proposal, target or intervention fields.

The input ceiling is 256 KiB, the JSON depth ceiling is eight, and decoded
strings are bounded to 128 KiB. A 2,048-item ceiling bounds structural
allocation. Original proposal, address, label, replay and host-specific limits
remain in force. The encoder checks typed sizes before copying or hex-encoding
payloads and emits the same exact numeric and byte conventions.

Malformed input produces a bounded ordinary Tower error envelope with
`<invalid-request>` and `fa://invalid/run/request`. No partial operation ID,
address, payload or key is echoed. Capacity refusals use `BudgetExhausted`,
unsupported schemas and unknown verbs use `CapabilityUnsupported`, and other
syntax failures use `IntegrityFailure`. Their retry flag is false; no read unit,
disclosure query, ticket, branch event or native journal operation was spent.
The router uses the same response whether a publication mount exists or not.

After decoding, the original typed dispatch checks caller scope, requested
generation, purpose, current owner and resource allowances. The fixed routing
rules still prevent a native refusal or lost owner from falling back to an
imported snapshot. Global Tower clocks remain distinct from native request,
disclosure and elapsed coordinates. A failed response delivery does not undo an
admitted effect or justify changing its idempotency key.

## Integration

```rust
use fa_reference::tower::{ReferenceTower, TowerAccess, TowerEncodingError};

fn exchange(
    tower: &mut ReferenceTower,
    authenticated_access: &TowerAccess,
    document: &[u8],
) -> Result<String, TowerEncodingError> {
    tower.exchange(authenticated_access, document).to_json()
}
```

`TowerRouter::exchange` has the same arguments and returns `RoutedResponse`.
Callers that already construct typed requests can continue using `dispatch`, or
use `encode_request` to produce a bounded request for another integrating host.
The typed and byte interfaces share the original dispatcher and native owners;
there is no alternate backend, result cache or dependency.

The codec and real-owner integration regressions accompany this source change.
Execution and the full dispatcher/deployment qualifications remain open until
the required revision-bound RCH checks run. Request decoding does not implement
the remaining registered verbs, a `fa` executable, a network service, live global
situation reconstruction or signed receipt verification.
