# Complete Knowledge JSON for reference agent surfaces

The JSON encoder in `knowledge_surface` serves typed reference agent APIs.
It preserves epistemic evidence; it does not authenticate that evidence or turn
a declared proof into a production guarantee.

## APIs and compatibility

`encode_knowledge_json<T: SafetyRelevant>(&Knowledge<T>)` returns a fallible
fieldless JSON wrapper. `AgentSurfaceSerializer::try_serialize_safety_field`
also includes an escaped field name. Existing
`AgentSurfaceSerializer::serialize_safety_field` remains available with the
same signature. If encoding fails, its result is explicitly Unknown with
ConflictingEvidence and an IntegrityFailure serialization marker.

The new default `SafetyRelevant::canonical_payload_json` safely quotes the
existing textual representation, so existing implementations remain compatible.
A domain type may override it to emit structured JSON. Bare primitive types
still do not implement SafetyRelevant. Custom payloads undergo strict bounded
parsing and deterministic re-encoding before embedding; duplicate keys,
trailing fragments and invalid JSON are refused.

`SerializerRefusal` adds InvalidPayloadJson and EncodingLimitExceeded.
Downstream exhaustive matches must handle these variants.

## Evidence retained

| Variant | Retained data |
|---|---|
| Known | Typed value, all nine BasisIdentity fields, claim class, all four proof states |
| Pending | Complete source/projection keys, stages, through-sequences, closure generations and expected cost |
| Unknown | Exact reason; no fabricated value |
| Withheld | Required authority; no payload encoding or inspection |
| Stale | Complete previous basis and current generation, without restoring a value |
| Absent | Domain/projection/epoch, revision, control cut, semantic epoch, required frontier and exact closing marker |

Known also retains the existing top-level claim_class, generation and control_seq
compatibility fields. Stale retains its existing generation fields. Every
Declared, Gapped, Unsupported and Expired proof state keeps its own data.

## Bounds and wire checks

Complete wrappers are limited to 1 MiB, depth 64, 32,768 items and 256 KiB decoded
strings. Custom payloads are limited to depth 56 and 30,000 items, reserving room
for the complete wrapper. Type and field names have a 1 KiB bound. Pending is
limited to the existing 16-frontier contract even after direct public-enum
construction. Limits cover parsing and embedding the returned payload; arbitrary
work performed by a trusted Rust payload implementation is outside that contract.

Strings escape quotes, backslashes and JSON control characters. Numbers retain
their exact JSON lexeme, including u64 values; this is deterministic encoding,
not a claim that different numeric spellings receive JCS normalization.

`validate_wire_field_is_wrapped` is a bounded syntax and top-level-tag guard.
It rejects malformed JSON, nested fake tags, missing type tags, duplicate keys
and unrecognized states. It does not check every variant's semantic obligations
and must never be used as an evidence decoder or authority admission boundary.

The legacy binary `encode_knowledge_verdict_v1` format is unchanged.

## Regression scope

`crates/fa-reference/tests/knowledge_surface_wire.rs` supplies twelve public
regression functions with permitted and causal-negative cases. Independent source
review and qualified formatting completed; focused executable verification is
recorded separately in implementation status when its terminal result is known.
