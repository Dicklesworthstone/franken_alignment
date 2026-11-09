# Bounded JSON requests through the original Tower owners

An integrating host can now accept one `fa.tower_request/1` JSON document through
`ReferenceTower::exchange` or, on Unix, `TowerRouter::exchange` and
`PublicationTower::exchange`. The request codec
also exposes `decode_request` and `encode_request` for integrations that need the
existing typed interface. Responses use the original sixteen-field envelope and
the selected backend's original result encoder.

The consumer is an agent connection whose host already owns a configured Tower
and authenticates its caller. The host supplies `TowerAccess` separately on each
call. No request field can supply an operator role, purpose grant, disclosure,
experiment context, owner or permission. This is a document interface; the host
still owns transport framing, authentication, scheduling and elapsed deadlines.

The [Linux live publication service](TOWER_PEER.md) supplies a concrete consumer:
select `--tower-generation` on an ordinary `supervise_publication` serve mode,
then send explicit request files with `tower-exchange`. It uses the same codec
and native owner with bounded newline framing and kernel-credential admission.

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
| `get`, `why-held`, `replay-plan` | `null` |
| `explain` | `null` for the existing explanation; `{"round":"801"}` selects a separately granted original completed congress at a native request address |
| `situation` | `null` for a fresh capture; `{"cursor":"17"}` for native continuation; opt-in request links use `{"request_links":true}` or `{"request_links":true,"cursor":"17"}` |
| `related` | `null` or `{"relation":null}`; a relation string selects one existing relation kind |
| `tail` | `null` or `{"since_seq":"0","max_events":"64"}` |
| `next` | `null` or `{"max_cost":"8"}` |
| `replay` | `null` or `{"interventions":[...]}`; requires a nonempty idempotency key |
| `propose` | `{"request":"1","proposal":{...}}`; requires the same canonical key and an explicit nonzero address generation |
| `precheck` | `{"query":"1","proposal":{...}}` |
| `capabilities` | `{"query":"1","expected_policy_epoch":"0"}` at the native run mount with an explicitly installed disclosure |
| `policy-replay` | `{"campaign":"1","expected_policy_generation":"1","candidate":{"generation":"2","nodes":[...]}}`, optionally adding `"paged":true`; native page continuation uses exactly `{"campaign":"1","cursor":"17"}`. Requires separate native grants, canonical matching key and explicit nonzero mount generation |
| Other registered verbs | `null`; the original dispatcher returns `CapabilityUnsupported` |

The six relation strings are `closure_strong_ref`, `closure_weak_ref`,
`provenance`, `thought_graph_edge`, `support_set_member` and `annotation_target`.
`tail` accepts 1 through 128 events at the syntax boundary; the host's actual
page limit can be smaller. Omitted optional read arguments retain their original
typed defaults. An unknown or retired verb is not silently treated as a read.

### Native situation pages

A separately granted operator inspection can continue beyond the first 16
attempts by copying the native result's `knowledge.value.continuation` into
`{"cursor":"N"}`. The cursor is a nonzero canonical `u64` string, is the sole
argument in the default profile, and is accepted only by the original native publication facade that
retains the issued cursor. It cannot supply an offset, grant inspection, cross
owners or select an imported reference snapshot. The typed equivalent is
`ReadArguments::SituationPage { cursor }`.

Continued pages retain the original capture's basis, ordering and whole-ledger
summary. Each accepted page spends the original grant's shared query allowance
and consumes its cursor; the returned continuation selects the next page.
`arguments: null` starts a new capture and invalidates prior progress for that
grant. Check `enumeration_complete` independently of continuation: the final
allowed query returns no cursor even if rows remain. These process/service-local
values do not resume across restarts. See the
[native inspection contract](TOWER_PUBLICATION.md#inspect-the-original-live-owners-situation).

Request navigation is an explicit opt-in: use `{"request_links":true}` for the
first page and `{"request_links":true,"cursor":"N"}` for its continuation.
The typed form is `ReadArguments::SituationLinks { cursor: Option<u64> }`.
`request_links` must be the boolean `true`; false, null, string values and extra
fields refuse. The response uses `fa.publication-situation/2` and adds the
original external request key and canonical request address, both nullable,
to each captured attempt. Default requests retain the exact v1 row shape.
The same original page, cursor custody and query allowance apply; changing the
projection does not recapture or grant another read. Imported reference
snapshots reject this native projection without charging their read allowance.
See [captured request links](TOWER_PUBLICATION.md#follow-captured-attempts-to-original-requests).

The bounded operator client exposes the same selection as
`tower-situation OPERATOR_PROFILE RUN_ADDRESS --max-pages N --request-links`.
It validates every paired request identity and the selected v2 schema before
emitting a page or following its original cursor. Default client requests and
v1 validation remain unchanged.

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

### Native candidate-policy replay

An explicitly granted operator can use `policy-replay` at the original native
run mount to compare a candidate with the owner's complete retained
current-policy corpus. The request requires the host's operator role and
`Experiment` purpose, an installed same-owner policy-replay port, and an exact
registration generation. The caller cannot select cases or provide evidence.

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "compare-policy-91",
  "address": "fa://1/run/3@41",
  "verb": "policy-replay",
  "arguments": {
    "campaign": "91",
    "expected_policy_generation": "1",
    "candidate": {
      "generation": "2",
      "nodes": [{"op": "payload_at_most", "maximum": "128"}]
    }
  },
  "idempotency_key": "91"
}
```

The closed argument object has exactly `campaign`,
`expected_policy_generation` and `candidate`. The first two are nonzero decimal
strings, and the key must equal `campaign`. The candidate has exactly
`generation` and `nodes`; all predicate arguments and child indices use the
same canonical string convention. Bytes use lowercase hexadecimal. The original
`Policy::new` constructor validates the bounded, reachable, topologically ordered
graph. Unknown operators, extra fields, duplicate keys and invalid references
refuse before native work. The native port additionally requires candidate
generation to exceed the expected current-policy generation.

The typed form is `ReadArguments::PolicyReplay { campaign,
expected_policy_generation, candidate }`. Exact campaign retries retain the
first result and original owner cut across aliases and later journal changes,
without spending another campaign. Different contents conflict; a fresh
comparison requires a new admitted campaign. This retained cache belongs to the
native grant, and every disclosure still checks original owner health and live
custody. Its identities do not survive owner/grant replacement.

The response uses `fa.publication-policy-replay/1`, preserves every selected case
and exact trace, and omits payload and observed witness values. It supplies no
promotion or publication authority. See the [full policy-replay contract](TOWER_POLICY_REPLAY.md)
for predicate syntax, disclosure, result basis, bounded failures and exact retries.

### Native candidate-policy pages

The complete initial `policy-replay` arguments may additionally contain
`"paged": true`, selecting `ReadArguments::PolicyReplayPaged`. This selector must
be the boolean true; false, null, text and unrelated fields are rejected. A
separately provisioned page allowance is required. The original complete replay
and campaign cache are unchanged; only disclosure is divided into a header and
contiguous groups of at most 16 whole cases.

Follow the returned native cursor using exactly `{"campaign":"91","cursor":"17"}`
and the matching `"91"` idempotency key at the original explicit mount. This is
`ReadArguments::PolicyReplayContinue`; both values must be nonzero canonical
unsigned decimal strings. Candidate, expected generation, paged selector,
caller-supplied offsets and extra fields are forbidden on continuations. The
facade accepts only sealed native cursor handles it has issued, bound to the
original owner, grant, campaign and report.

Responses use `fa.publication-policy-replay/2`. Exact requests retain the
original page and cut without another native campaign or page charge. Check
`page.enumeration_complete` independently of `page.next_cursor`: exhaustion can
return no continuation before every case has been delivered. These identities
are process-local and supply no authority across restart or grant replacement.
See [bounded policy pages](TOWER_POLICY_REPLAY.md#enumerate-a-complete-comparison-in-bounded-pages)
for original custody, summary, output refusal and client validation behavior.
The authenticated `tower-policy-replay OPERATOR_PROFILE TOWER_REQUEST_JSON
--max-pages N` consumer accepts an explicit initial paged document and follows
only validated native continuations within the original connection and budgets.

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
proposal, target, intervention or policy-graph fields.

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
`PublicationTower::exchange` directly returns the original `PublicationResponse`.
Callers that already construct typed requests can continue using `dispatch`, or
use `encode_request` to produce a bounded request for another integrating host.
The typed and byte interfaces share the original dispatcher and native owners.
The codec adds no alternate backend, result cache or dependency; policy campaign
retention belongs to the original explicitly granted native port.

The codec and real-owner integration regressions accompany these interfaces.
The policy-replay contract records its [focused local runtime result](TOWER_POLICY_REPLAY.md#executed-coverage-and-remaining-qualification).
Full dispatcher, repository and deployment qualifications remain open until the
required revision-bound checks run. Request decoding does not implement
the remaining registered verbs, a `fa` executable, a network service, live global
situation reconstruction or signed receipt verification.
