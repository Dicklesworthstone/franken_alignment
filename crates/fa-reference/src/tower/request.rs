//! Closed, bounded request syntax for the existing Tower operations.
//!
//! Decoding constructs data only. Caller grants, original owners, disclosure
//! ports and experiment contexts are provisioned by the host, never by a wire
//! field. Native admission, charging and retry semantics remain in dispatch.

use std::collections::BTreeMap;
use std::fmt::{self, Write};

#[cfg(unix)]
pub(super) mod convening;
#[cfg(unix)]
pub(super) mod fence;
#[cfg(unix)]
pub(super) mod narrowing;
pub(super) mod policy;
#[cfg(unix)]
pub(super) mod policy_promotion;
#[cfg(unix)]
pub(super) mod resolution;

use super::{ReadArguments, TowerRequest, Verb, replay, validate_address};
use crate::action::consequence::experiment::{Intervention, MAX_INTERVENTION_BYTES};
use crate::action::consequence::oversight::actor::{ActorError, ActorProposal};
use crate::action::{ElapsedTick, MAX_PAYLOAD_BYTES, ResolvedTarget};
use crate::address_resolver::{Address, RelationKind};
use crate::strict_json::{self, ErrorKind, Json, Limits};

pub const REQUEST_SCHEMA: &str = "fa.tower_request/1";
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const MAX_REQUEST_DEPTH: usize = 8;
// Includes both object keys and values, with room for all 64 target edits.
pub const MAX_REQUEST_ITEMS: usize = 2048;
/// The live wire accepts a bounded canonical restriction, independently of the
/// wider trusted native API. This does not enlarge any owner or grant limit.
pub const MAX_NARROWING_TARGETS: usize = 64;
pub const MAX_REQUEST_STRING_BYTES: usize = MAX_PAYLOAD_BYTES * 2;

/// Finite public errors deliberately contain no input, field names, offsets or
/// native diagnostics. Even a partially decoded request has no echo on failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TowerDecodingError {
    MalformedRequest,
    UnsupportedVersion,
    UnknownVerb,
    Capacity,
}

impl fmt::Display for TowerDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MalformedRequest => "malformed Tower request",
            Self::UnsupportedVersion => "unsupported Tower request version",
            Self::UnknownVerb => "unknown Tower verb",
            Self::Capacity => "Tower request capacity exceeded",
        })
    }
}

impl std::error::Error for TowerDecodingError {}

type Object = BTreeMap<String, Json>;
type DecodeResult<T> = Result<T, TowerDecodingError>;

/// Decode exactly one complete JSON request before any Tower operation runs.
/// All integer arguments are canonical decimal strings, preserving every u64
/// bit through clients that cannot represent full-width JSON numbers exactly.
pub fn decode_request(document: &[u8]) -> DecodeResult<TowerRequest> {
    let parsed = strict_json::parse(
        document,
        Limits {
            max_bytes: MAX_REQUEST_BYTES,
            max_depth: MAX_REQUEST_DEPTH,
            max_items: MAX_REQUEST_ITEMS,
            max_string_bytes: MAX_REQUEST_STRING_BYTES,
        },
    )
    .map_err(|error| match error.kind {
        ErrorKind::SizeLimit
        | ErrorKind::DepthLimit
        | ErrorKind::ItemLimit
        | ErrorKind::StringLimit => TowerDecodingError::Capacity,
        _ => TowerDecodingError::MalformedRequest,
    })?;
    let root = object(&parsed)?;
    fields(
        root,
        &[
            "schema_version",
            "operation_id",
            "address",
            "verb",
            "arguments",
            "idempotency_key",
        ],
    )?;
    if text(root, "schema_version")? != REQUEST_SCHEMA {
        return Err(TowerDecodingError::UnsupportedVersion);
    }
    let operation_id = text(root, "operation_id")?;
    bounded_label(operation_id)?;
    let address_text = text(root, "address")?;
    if address_text.len() > 256 {
        return Err(TowerDecodingError::Capacity);
    }
    let address = Address::parse(address_text).map_err(|_| TowerDecodingError::MalformedRequest)?;
    // Address::parse also accepts leading zeros in a generation. The wire's
    // identity is the canonical address, not an alternate spelling of it.
    if address.canonical_string() != address_text {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let verb = text(root, "verb")?
        .parse::<Verb>()
        .map_err(|_| TowerDecodingError::UnknownVerb)?;
    let idempotency_key = match field(root, "idempotency_key")? {
        Json::Null => None,
        Json::String(value) => {
            bounded_label(value)?;
            Some(value.clone())
        }
        _ => return Err(TowerDecodingError::MalformedRequest),
    };
    let arguments = decode_arguments(verb, field(root, "arguments")?)?;
    let request = TowerRequest {
        operation_id: operation_id.to_owned(),
        address,
        verb,
        arguments,
        idempotency_key,
    };
    validate_request(&request)?;
    Ok(request)
}

/// Encode a fully validated request deterministically. This performs no read,
/// reserves no work and never turns a typed proposal into a permission.
pub fn encode_request(request: &TowerRequest) -> DecodeResult<String> {
    // Check all typed collection lengths and native proposal bounds before any
    // payload is hex encoded or a caller's unbounded vector could be copied.
    validate_request(request)?;
    let encoded = object_json(&[
        ("schema_version", quote(REQUEST_SCHEMA)),
        ("operation_id", quote(&request.operation_id)),
        ("address", quote(&request.address.canonical_string())),
        ("verb", quote(request.verb.as_str())),
        ("arguments", encode_arguments(&request.arguments)),
        (
            "idempotency_key",
            request
                .idempotency_key
                .as_deref()
                .map_or_else(|| "null".to_owned(), quote),
        ),
    ]);
    if encoded.len() > MAX_REQUEST_BYTES {
        return Err(TowerDecodingError::Capacity);
    }
    Ok(encoded)
}

fn decode_arguments(verb: Verb, value: &Json) -> DecodeResult<ReadArguments> {
    if value.is_null() {
        return Ok(ReadArguments::None);
    }
    let args = object(value)?;
    match verb {
        Verb::Related => {
            fields(args, &["relation"])?;
            let relation = match field(args, "relation")? {
                Json::Null => None,
                Json::String(value) => Some(decode_relation(value)?),
                _ => return Err(TowerDecodingError::MalformedRequest),
            };
            Ok(ReadArguments::Related { relation })
        }
        Verb::Tail => {
            if args.contains_key("after_revision") {
                fields(args, &["after_revision"])?;
                return Ok(ReadArguments::JournalTail {
                    after_revision: decimal(args, "after_revision")?,
                });
            }
            if args.contains_key("cursor") {
                fields(args, &["cursor"])?;
                return Ok(ReadArguments::JournalTailPage {
                    cursor: decimal(args, "cursor")?,
                });
            }
            fields(args, &["since_seq", "max_events"])?;
            Ok(ReadArguments::Tail {
                since_seq: decimal(args, "since_seq")?,
                max_events: usize::try_from(decimal(args, "max_events")?)
                    .map_err(|_| TowerDecodingError::Capacity)?,
            })
        }
        Verb::Situation => {
            if args.contains_key("request_links") {
                if field(args, "request_links")?.as_bool() != Some(true) {
                    return Err(TowerDecodingError::MalformedRequest);
                }
                let cursor = if args.contains_key("cursor") {
                    fields(args, &["request_links", "cursor"])?;
                    Some(decimal(args, "cursor")?)
                } else {
                    fields(args, &["request_links"])?;
                    None
                };
                return Ok(ReadArguments::SituationLinks { cursor });
            }
            fields(args, &["cursor"])?;
            Ok(ReadArguments::SituationPage {
                cursor: decimal(args, "cursor")?,
            })
        }
        Verb::Explain => {
            fields(args, &["round"])?;
            Ok(ReadArguments::ReviewExplanation {
                round: decimal(args, "round")?,
            })
        }
        #[cfg(unix)]
        Verb::Next if args.contains_key("round") => {
            fields(args, &["action", "round"])?;
            if text(args, "action")? != "convene_held" {
                return Err(TowerDecodingError::MalformedRequest);
            }
            Ok(ReadArguments::ConveneNext {
                round: decimal(args, "round")?,
            })
        }
        #[cfg(unix)]
        Verb::Convene => {
            fields(args, &["round", "expected"])?;
            Ok(ReadArguments::Convene {
                round: decimal(args, "round")?,
                expected: convening::decode_basis(field(args, "expected")?)?,
            })
        }
        #[cfg(unix)]
        Verb::Next if args.contains_key("operation") => {
            if args.contains_key("action") {
                if text(args, "action")? == "promote_policy" {
                    fields(args, &["action", "campaign", "operation"])?;
                    return Ok(ReadArguments::PolicyPromotionNext {
                        campaign: decimal(args, "campaign")?,
                        operation: decimal(args, "operation")?,
                    });
                }
                if text(args, "action")? == "narrow_authority" {
                    fields(args, &["action", "operation", "retained_targets"])?;
                    return Ok(ReadArguments::NarrowingNext {
                        operation: decimal(args, "operation")?,
                        retained_targets: narrowing::decode_targets(field(
                            args,
                            "retained_targets",
                        )?)?,
                    });
                }
                fields(args, &["action", "operation"])?;
                let operation = decimal(args, "operation")?;
                return match text(args, "action")? {
                    "cancel_undispatched" => Ok(ReadArguments::CancellationNext { operation }),
                    "abandon_unknown" => Ok(ReadArguments::AbandonmentNext { operation }),
                    _ => Err(TowerDecodingError::MalformedRequest),
                };
            }
            fields(args, &["operation"])?;
            Ok(ReadArguments::ResolutionNext {
                operation: decimal(args, "operation")?,
            })
        }
        Verb::Next => {
            fields(args, &["max_cost"])?;
            Ok(ReadArguments::Next {
                max_cost: decimal(args, "max_cost")?,
            })
        }
        #[cfg(unix)]
        Verb::Resolve => {
            if args.contains_key("action") {
                fields(args, &["action", "operation", "expected"])?;
                let operation = decimal(args, "operation")?;
                let expected = resolution::decode_basis(field(args, "expected")?)?;
                return match text(args, "action")? {
                    "cancel_undispatched" => Ok(ReadArguments::CancelUndispatched {
                        operation,
                        expected,
                    }),
                    "abandon_unknown" => Ok(ReadArguments::AbandonUnknown {
                        operation,
                        expected,
                    }),
                    _ => Err(TowerDecodingError::MalformedRequest),
                };
            }
            fields(args, &["operation", "expected"])?;
            Ok(ReadArguments::Resolve {
                operation: decimal(args, "operation")?,
                expected: resolution::decode_basis(field(args, "expected")?)?,
            })
        }
        #[cfg(unix)]
        Verb::Narrow => {
            fields(args, &["operation", "expected", "retained_targets"])?;
            Ok(ReadArguments::Narrow {
                operation: decimal(args, "operation")?,
                expected: narrowing::decode_basis(field(args, "expected")?)?,
                retained_targets: narrowing::decode_targets(field(args, "retained_targets")?)?,
            })
        }
        #[cfg(unix)]
        Verb::Promote => {
            fields(args, &["operation", "expected"])?;
            Ok(ReadArguments::Promote {
                operation: decimal(args, "operation")?,
                expected: policy_promotion::decode_basis(field(args, "expected")?)?,
            })
        }
        #[cfg(unix)]
        Verb::Fence => {
            fields(args, &["operation", "expected"])?;
            Ok(ReadArguments::Fence {
                operation: decimal(args, "operation")?,
                expected: fence::decode_basis(field(args, "expected")?)?,
            })
        }
        Verb::Replay => {
            fields(args, &["interventions"])?;
            let edits = field(args, "interventions")?
                .as_array()
                .ok_or(TowerDecodingError::MalformedRequest)?;
            if edits.len() > crate::action::consequence::experiment::MAX_INTERVENTIONS {
                return Err(TowerDecodingError::Capacity);
            }
            let mut interventions = Vec::with_capacity(edits.len());
            let mut remaining_bytes = MAX_INTERVENTION_BYTES;
            for edit in edits {
                interventions.push(decode_intervention(edit, &mut remaining_bytes)?);
            }
            Ok(ReadArguments::Replay { interventions })
        }
        Verb::Propose => {
            fields(args, &["request", "proposal"])?;
            Ok(ReadArguments::Propose {
                request: decimal(args, "request")?,
                proposal: decode_proposal(field(args, "proposal")?)?,
            })
        }
        Verb::Precheck => {
            fields(args, &["query", "proposal"])?;
            Ok(ReadArguments::Precheck {
                query: decimal(args, "query")?,
                proposal: decode_proposal(field(args, "proposal")?)?,
            })
        }
        Verb::Capabilities => {
            fields(args, &["query", "expected_policy_epoch"])?;
            Ok(ReadArguments::Capabilities {
                query: decimal(args, "query")?,
                expected_policy_epoch: decimal(args, "expected_policy_epoch")?,
            })
        }
        Verb::PolicyReplay => {
            if args.contains_key("cursor") {
                fields(args, &["campaign", "cursor"])?;
                return Ok(ReadArguments::PolicyReplayContinue {
                    campaign: decimal(args, "campaign")?,
                    cursor: decimal(args, "cursor")?,
                });
            }
            if args.contains_key("paged") {
                fields(
                    args,
                    &[
                        "campaign",
                        "expected_policy_generation",
                        "candidate",
                        "paged",
                    ],
                )?;
                if field(args, "paged")?.as_bool() != Some(true) {
                    return Err(TowerDecodingError::MalformedRequest);
                }
                return Ok(ReadArguments::PolicyReplayPaged {
                    campaign: decimal(args, "campaign")?,
                    expected_policy_generation: decimal(args, "expected_policy_generation")?,
                    candidate: policy::decode(field(args, "candidate")?)?,
                });
            }
            fields(
                args,
                &["campaign", "expected_policy_generation", "candidate"],
            )?;
            Ok(ReadArguments::PolicyReplay {
                campaign: decimal(args, "campaign")?,
                expected_policy_generation: decimal(args, "expected_policy_generation")?,
                candidate: policy::decode(field(args, "candidate")?)?,
            })
        }
        _ => Err(TowerDecodingError::MalformedRequest),
    }
}

fn validate_request(request: &TowerRequest) -> DecodeResult<()> {
    bounded_label(&request.operation_id)?;
    if let Some(key) = &request.idempotency_key {
        bounded_label(key)?;
    }
    validate_address(&request.address).map_err(|_| TowerDecodingError::MalformedRequest)?;
    match (&request.verb, &request.arguments) {
        (
            Verb::Propose,
            ReadArguments::Propose {
                request: key,
                proposal,
            },
        ) => {
            if *key == 0
                || request.idempotency_key.as_deref() != Some(key.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
            validate_proposal(proposal)?;
        }
        #[cfg(unix)]
        (
            Verb::Next,
            ReadArguments::ResolutionNext { operation }
            | ReadArguments::CancellationNext { operation }
            | ReadArguments::AbandonmentNext { operation },
        ) => {
            if *operation == 0
                || request
                    .idempotency_key
                    .as_ref()
                    .is_some_and(|key| key != &operation.to_string())
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (Verb::Next, ReadArguments::ConveneNext { round }) => {
            if *round == 0
                || request
                    .idempotency_key
                    .as_ref()
                    .is_some_and(|key| key != &round.to_string())
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (Verb::Convene, ReadArguments::Convene { round, expected }) => {
            convening::validate_expected(*expected)?;
            if *round == 0
                || request.idempotency_key.as_deref() != Some(round.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (
            Verb::Resolve,
            ReadArguments::Resolve {
                operation,
                expected,
            }
            | ReadArguments::CancelUndispatched {
                operation,
                expected,
            }
            | ReadArguments::AbandonUnknown {
                operation,
                expected,
            },
        ) => {
            resolution::validate_expected(*expected)?;
            if *operation == 0
                || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (
            Verb::Next,
            ReadArguments::NarrowingNext {
                operation,
                retained_targets,
            },
        ) => {
            narrowing::validate_targets(retained_targets)?;
            if *operation == 0
                || request.address.kind != crate::address_resolver::Noun::Run
                || request
                    .idempotency_key
                    .as_ref()
                    .is_some_and(|key| key != &operation.to_string())
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (
            Verb::Next,
            ReadArguments::PolicyPromotionNext {
                campaign,
                operation,
            },
        ) => {
            if *campaign == 0
                || *operation == 0
                || request
                    .idempotency_key
                    .as_ref()
                    .is_some_and(|key| key != &operation.to_string())
                || request.address.kind != crate::address_resolver::Noun::Run
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (
            Verb::Promote,
            ReadArguments::Promote {
                operation,
                expected,
            },
        ) => {
            policy_promotion::validate_expected(*expected)?;
            if *operation == 0
                || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
                || request.address.kind != crate::address_resolver::Noun::Run
                || request.address.tenant != expected.scope.tenant.to_string()
                || request.address.id != expected.scope.run.to_string()
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (
            Verb::Narrow,
            ReadArguments::Narrow {
                operation,
                expected,
                retained_targets,
            },
        ) => {
            narrowing::validate_expected(*expected)?;
            narrowing::validate_targets(retained_targets)?;
            if *operation == 0
                || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
                || request.address.kind != crate::address_resolver::Noun::Run
                || request.address.tenant != expected.scope.tenant.to_string()
                || request.address.id != expected.scope.run.to_string()
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        #[cfg(unix)]
        (
            Verb::Fence,
            ReadArguments::Fence {
                operation,
                expected,
            },
        ) => {
            fence::validate_expected(*expected)?;
            if *operation == 0
                || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
                || request.address.kind != crate::address_resolver::Noun::Run
                || request.address.tenant != expected.scope.tenant.to_string()
                || request.address.id != expected.scope.run.to_string()
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (Verb::Precheck, ReadArguments::Precheck { query, proposal }) => {
            if *query == 0 {
                return Err(TowerDecodingError::MalformedRequest);
            }
            validate_proposal(proposal)?;
        }
        (Verb::Capabilities, ReadArguments::Capabilities { query, .. }) => {
            if *query == 0 {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (
            Verb::PolicyReplay,
            ReadArguments::PolicyReplay {
                campaign,
                expected_policy_generation,
                candidate,
            }
            | ReadArguments::PolicyReplayPaged {
                campaign,
                expected_policy_generation,
                candidate,
            },
        ) => {
            if *campaign == 0
                || *expected_policy_generation == 0
                || request.idempotency_key.as_deref() != Some(campaign.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
            if matches!(request.arguments, ReadArguments::PolicyReplayPaged { .. }) {
                policy::validate_paged_wire(candidate)?;
            } else {
                policy::validate_wire(candidate)?;
            }
        }
        (Verb::PolicyReplay, ReadArguments::PolicyReplayContinue { campaign, cursor }) => {
            if *campaign == 0
                || *cursor == 0
                || request.idempotency_key.as_deref() != Some(campaign.to_string().as_str())
                || request
                    .address
                    .generation
                    .is_none_or(|generation| generation == 0)
            {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (Verb::Explain, ReadArguments::ReviewExplanation { round }) => {
            if *round == 0 {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (Verb::Situation, ReadArguments::SituationPage { cursor }) => {
            if *cursor == 0 {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (Verb::Situation, ReadArguments::SituationLinks { cursor }) => {
            if cursor.is_some_and(|value| value == 0) {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (Verb::Replay, ReadArguments::Replay { interventions }) => {
            replay::validate_edits(interventions).map_err(|_| TowerDecodingError::Capacity)?;
        }
        (Verb::Related, ReadArguments::Related { .. })
        | (Verb::Next, ReadArguments::Next { .. }) => {}
        (Verb::Tail, ReadArguments::Tail { max_events, .. }) => {
            // The host may impose a smaller page bound. No Tower admits more
            // than this existing maximum configurable page size.
            if *max_events == 0 || *max_events > 128 {
                return Err(TowerDecodingError::Capacity);
            }
        }
        (Verb::Tail, ReadArguments::JournalTail { .. }) => {}
        (Verb::Tail, ReadArguments::JournalTailPage { cursor }) => {
            if *cursor == 0 {
                return Err(TowerDecodingError::MalformedRequest);
            }
        }
        (
            Verb::Propose | Verb::Precheck | Verb::Capabilities | Verb::PolicyReplay,
            ReadArguments::None,
        ) => {
            return Err(TowerDecodingError::MalformedRequest);
        }
        #[cfg(unix)]
        (
            Verb::Resolve | Verb::Fence | Verb::Narrow | Verb::Convene | Verb::Promote,
            ReadArguments::None,
        ) => {
            return Err(TowerDecodingError::MalformedRequest);
        }
        (_, ReadArguments::None) => {}
        _ => return Err(TowerDecodingError::MalformedRequest),
    }
    if request.verb == Verb::Replay && request.idempotency_key.is_none() {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

fn decode_proposal(value: &Json) -> DecodeResult<ActorProposal> {
    let value = object(value)?;
    fields(
        value,
        &[
            "target",
            "payload_hex",
            "units",
            "deadline",
            "expected_policy_epoch",
        ],
    )?;
    let proposal = ActorProposal {
        target: decode_target(field(value, "target")?)?,
        payload: unhex(text(value, "payload_hex")?, MAX_PAYLOAD_BYTES)?,
        units: decimal(value, "units")?,
        deadline: ElapsedTick(decimal(value, "deadline")?),
        expected_policy_epoch: decimal(value, "expected_policy_epoch")?,
    };
    validate_proposal(&proposal)?;
    Ok(proposal)
}

fn validate_proposal(proposal: &ActorProposal) -> DecodeResult<()> {
    proposal.validate().map_err(|error| match error {
        ActorError::Capacity => TowerDecodingError::Capacity,
        _ => TowerDecodingError::MalformedRequest,
    })
}

fn decode_target(value: &Json) -> DecodeResult<ResolvedTarget> {
    let value = object(value)?;
    fields(
        value,
        &[
            "adapter",
            "object",
            "contract_version",
            "expected_version",
            "generation",
        ],
    )?;
    Ok(ResolvedTarget {
        adapter: decimal(value, "adapter")?,
        object: decimal(value, "object")?,
        contract_version: decimal(value, "contract_version")?,
        expected_version: decimal(value, "expected_version")?,
        generation: decimal(value, "generation")?,
    })
}

fn decode_intervention(value: &Json, remaining_bytes: &mut usize) -> DecodeResult<Intervention> {
    let value = object(value)?;
    match text(value, "kind")? {
        "payload" => {
            fields(value, &["kind", "payload_hex"])?;
            Ok(Intervention::Payload(charged_hex(
                text(value, "payload_hex")?,
                remaining_bytes,
            )?))
        }
        "target" => {
            fields(value, &["kind", "target"])?;
            Ok(Intervention::Target(decode_target(field(
                value, "target",
            )?)?))
        }
        "units" => {
            fields(value, &["kind", "units"])?;
            Ok(Intervention::Units(decimal(value, "units")?))
        }
        "key" => {
            fields(value, &["kind", "key", "expected_hex", "replacement_hex"])?;
            Ok(Intervention::Key {
                key: decimal(value, "key")?,
                expected: optional_hex(field(value, "expected_hex")?, remaining_bytes)?,
                replacement: optional_hex(field(value, "replacement_hex")?, remaining_bytes)?,
            })
        }
        _ => Err(TowerDecodingError::MalformedRequest),
    }
}

fn charged_hex(value: &str, remaining: &mut usize) -> DecodeResult<Vec<u8>> {
    let bytes = unhex(value, *remaining)?;
    *remaining -= bytes.len();
    Ok(bytes)
}

fn optional_hex(value: &Json, remaining: &mut usize) -> DecodeResult<Option<Vec<u8>>> {
    match value {
        Json::Null => Ok(None),
        Json::String(value) => charged_hex(value, remaining).map(Some),
        _ => Err(TowerDecodingError::MalformedRequest),
    }
}

fn object(value: &Json) -> DecodeResult<&Object> {
    value
        .as_object()
        .ok_or(TowerDecodingError::MalformedRequest)
}

fn fields(value: &Object, expected: &[&str]) -> DecodeResult<()> {
    if value.len() != expected.len() || expected.iter().any(|key| !value.contains_key(*key)) {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

fn field<'a>(value: &'a Object, key: &str) -> DecodeResult<&'a Json> {
    value.get(key).ok_or(TowerDecodingError::MalformedRequest)
}

fn text<'a>(value: &'a Object, key: &str) -> DecodeResult<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or(TowerDecodingError::MalformedRequest)
}

fn decimal(value: &Object, key: &str) -> DecodeResult<u64> {
    decimal_text(text(value, key)?)
}

fn decimal_text(value: &str) -> DecodeResult<u64> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(TowerDecodingError::MalformedRequest);
    }
    value
        .parse()
        .map_err(|_| TowerDecodingError::MalformedRequest)
}

fn bounded_label(value: &str) -> DecodeResult<()> {
    if value.len() > 128 {
        Err(TowerDecodingError::Capacity)
    } else if value.is_empty() {
        Err(TowerDecodingError::MalformedRequest)
    } else {
        Ok(())
    }
}

fn decode_relation(value: &str) -> DecodeResult<RelationKind> {
    match value {
        "closure_strong_ref" => Ok(RelationKind::ClosureStrongRef),
        "closure_weak_ref" => Ok(RelationKind::ClosureWeakRef),
        "provenance" => Ok(RelationKind::Provenance),
        "thought_graph_edge" => Ok(RelationKind::ThoughtGraphEdge),
        "support_set_member" => Ok(RelationKind::SupportSetMember),
        "annotation_target" => Ok(RelationKind::AnnotationTarget),
        _ => Err(TowerDecodingError::MalformedRequest),
    }
}

fn unhex(value: &str, limit: usize) -> DecodeResult<Vec<u8>> {
    if value.len() / 2 > limit {
        return Err(TowerDecodingError::Capacity);
    }
    if !value.len().is_multiple_of(2) {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(TowerDecodingError::MalformedRequest),
    };
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().as_chunks::<2>().0 {
        bytes.push((digit(pair[0])? << 4) | digit(pair[1])?);
    }
    Ok(bytes)
}

fn encode_arguments(arguments: &ReadArguments) -> String {
    match arguments {
        ReadArguments::None => "null".to_owned(),
        ReadArguments::Related { relation } => object_json(&[(
            "relation",
            relation.map_or_else(
                || "null".to_owned(),
                |relation| {
                    quote(match relation {
                        RelationKind::ClosureStrongRef => "closure_strong_ref",
                        RelationKind::ClosureWeakRef => "closure_weak_ref",
                        RelationKind::Provenance => "provenance",
                        RelationKind::ThoughtGraphEdge => "thought_graph_edge",
                        RelationKind::SupportSetMember => "support_set_member",
                        RelationKind::AnnotationTarget => "annotation_target",
                    })
                },
            ),
        )]),
        ReadArguments::Tail {
            since_seq,
            max_events,
        } => object_json(&[
            ("since_seq", quote(&since_seq.to_string())),
            ("max_events", quote(&max_events.to_string())),
        ]),
        ReadArguments::JournalTail { after_revision } => {
            object_json(&[("after_revision", quote(&after_revision.to_string()))])
        }
        ReadArguments::JournalTailPage { cursor } => {
            object_json(&[("cursor", quote(&cursor.to_string()))])
        }
        ReadArguments::SituationPage { cursor } => {
            object_json(&[("cursor", quote(&cursor.to_string()))])
        }
        ReadArguments::SituationLinks { cursor } => match cursor {
            Some(cursor) => object_json(&[
                ("request_links", "true".to_owned()),
                ("cursor", quote(&cursor.to_string())),
            ]),
            None => object_json(&[("request_links", "true".to_owned())]),
        },
        ReadArguments::ReviewExplanation { round } => {
            object_json(&[("round", quote(&round.to_string()))])
        }
        ReadArguments::Next { max_cost } => {
            object_json(&[("max_cost", quote(&max_cost.to_string()))])
        }
        #[cfg(unix)]
        ReadArguments::ResolutionNext { operation } => {
            object_json(&[("operation", quote(&operation.to_string()))])
        }
        #[cfg(unix)]
        ReadArguments::CancellationNext { operation } => object_json(&[
            ("action", quote("cancel_undispatched")),
            ("operation", quote(&operation.to_string())),
        ]),
        #[cfg(unix)]
        ReadArguments::ConveneNext { round } => object_json(&[
            ("action", quote("convene_held")),
            ("round", quote(&round.to_string())),
        ]),
        #[cfg(unix)]
        ReadArguments::Convene { round, expected } => object_json(&[
            ("round", quote(&round.to_string())),
            ("expected", convening::encode_basis(*expected)),
        ]),
        #[cfg(unix)]
        ReadArguments::CancelUndispatched {
            operation,
            expected,
        } => object_json(&[
            ("action", quote("cancel_undispatched")),
            ("operation", quote(&operation.to_string())),
            ("expected", resolution::encode_basis(*expected)),
        ]),
        #[cfg(unix)]
        ReadArguments::AbandonmentNext { operation } => object_json(&[
            ("action", quote("abandon_unknown")),
            ("operation", quote(&operation.to_string())),
        ]),
        #[cfg(unix)]
        ReadArguments::AbandonUnknown {
            operation,
            expected,
        } => object_json(&[
            ("action", quote("abandon_unknown")),
            ("operation", quote(&operation.to_string())),
            ("expected", resolution::encode_basis(*expected)),
        ]),
        #[cfg(unix)]
        ReadArguments::Resolve {
            operation,
            expected,
        } => object_json(&[
            ("operation", quote(&operation.to_string())),
            ("expected", resolution::encode_basis(*expected)),
        ]),
        #[cfg(unix)]
        ReadArguments::NarrowingNext {
            operation,
            retained_targets,
        } => object_json(&[
            ("action", quote("narrow_authority")),
            ("operation", quote(&operation.to_string())),
            (
                "retained_targets",
                narrowing::encode_targets(retained_targets),
            ),
        ]),
        #[cfg(unix)]
        ReadArguments::Narrow {
            operation,
            expected,
            retained_targets,
        } => object_json(&[
            ("operation", quote(&operation.to_string())),
            ("expected", narrowing::encode_basis(*expected)),
            (
                "retained_targets",
                narrowing::encode_targets(retained_targets),
            ),
        ]),
        #[cfg(unix)]
        ReadArguments::PolicyPromotionNext {
            campaign,
            operation,
        } => object_json(&[
            ("action", quote("promote_policy")),
            ("campaign", quote(&campaign.to_string())),
            ("operation", quote(&operation.to_string())),
        ]),
        #[cfg(unix)]
        ReadArguments::Promote {
            operation,
            expected,
        } => object_json(&[
            ("operation", quote(&operation.to_string())),
            ("expected", policy_promotion::encode_basis(*expected)),
        ]),
        #[cfg(unix)]
        ReadArguments::Fence {
            operation,
            expected,
        } => object_json(&[
            ("operation", quote(&operation.to_string())),
            ("expected", fence::encode_basis(*expected)),
        ]),
        ReadArguments::Replay { interventions } => object_json(&[(
            "interventions",
            array_json(interventions.iter().map(encode_intervention)),
        )]),
        ReadArguments::Propose { request, proposal } => object_json(&[
            ("request", quote(&request.to_string())),
            ("proposal", encode_proposal(proposal)),
        ]),
        ReadArguments::Precheck { query, proposal } => object_json(&[
            ("query", quote(&query.to_string())),
            ("proposal", encode_proposal(proposal)),
        ]),
        ReadArguments::Capabilities {
            query,
            expected_policy_epoch,
        } => object_json(&[
            ("query", quote(&query.to_string())),
            (
                "expected_policy_epoch",
                quote(&expected_policy_epoch.to_string()),
            ),
        ]),
        ReadArguments::PolicyReplay {
            campaign,
            expected_policy_generation,
            candidate,
        } => object_json(&[
            ("campaign", quote(&campaign.to_string())),
            (
                "expected_policy_generation",
                quote(&expected_policy_generation.to_string()),
            ),
            ("candidate", policy::encode(candidate)),
        ]),
        ReadArguments::PolicyReplayPaged {
            campaign,
            expected_policy_generation,
            candidate,
        } => object_json(&[
            ("campaign", quote(&campaign.to_string())),
            (
                "expected_policy_generation",
                quote(&expected_policy_generation.to_string()),
            ),
            ("candidate", policy::encode(candidate)),
            ("paged", "true".to_owned()),
        ]),
        ReadArguments::PolicyReplayContinue { campaign, cursor } => object_json(&[
            ("campaign", quote(&campaign.to_string())),
            ("cursor", quote(&cursor.to_string())),
        ]),
    }
}

fn encode_proposal(proposal: &ActorProposal) -> String {
    object_json(&[
        ("target", encode_target(&proposal.target)),
        ("payload_hex", quote(&hex(&proposal.payload))),
        ("units", quote(&proposal.units.to_string())),
        ("deadline", quote(&proposal.deadline.0.to_string())),
        (
            "expected_policy_epoch",
            quote(&proposal.expected_policy_epoch.to_string()),
        ),
    ])
}

fn encode_target(target: &ResolvedTarget) -> String {
    object_json(&[
        ("adapter", quote(&target.adapter.to_string())),
        ("object", quote(&target.object.to_string())),
        (
            "contract_version",
            quote(&target.contract_version.to_string()),
        ),
        (
            "expected_version",
            quote(&target.expected_version.to_string()),
        ),
        ("generation", quote(&target.generation.to_string())),
    ])
}

fn encode_intervention(edit: &Intervention) -> String {
    match edit {
        Intervention::Payload(bytes) => object_json(&[
            ("kind", quote("payload")),
            ("payload_hex", quote(&hex(bytes))),
        ]),
        Intervention::Target(target) => {
            object_json(&[("kind", quote("target")), ("target", encode_target(target))])
        }
        Intervention::Units(units) => object_json(&[
            ("kind", quote("units")),
            ("units", quote(&units.to_string())),
        ]),
        Intervention::Key {
            key,
            expected,
            replacement,
        } => object_json(&[
            ("kind", quote("key")),
            ("key", quote(&key.to_string())),
            ("expected_hex", encode_optional_hex(expected.as_deref())),
            (
                "replacement_hex",
                encode_optional_hex(replacement.as_deref()),
            ),
        ]),
    }
}

fn encode_optional_hex(value: Option<&[u8]>) -> String {
    value.map_or_else(|| "null".to_owned(), |bytes| quote(&hex(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    encoded
}

fn quote(value: &str) -> String {
    let mut encoded = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\u{0000}'..='\u{001f}' => {
                write!(encoded, "\\u{:04x}", u32::from(character)).expect("write to String");
            }
            _ => encoded.push(character),
        }
    }
    encoded.push('"');
    encoded
}

fn object_json(fields: &[(&str, String)]) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn array_json(values: impl Iterator<Item = String>) -> String {
    format!("[{}]", values.collect::<Vec<_>>().join(","))
}

#[cfg(test)]
mod tests;
