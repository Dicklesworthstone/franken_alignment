//! Sealed host-registration metadata beside original grant observations.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::FileOperatorCapabilityObservation;
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedOperatorCapabilities, OperatorCapabilityKind};

pub(super) fn encode(
    linked: &LinkedOperatorCapabilities,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let registration = linked.registration();
    if address != linked.query_address()
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || address.without_generation() != registration.without_generation()
        || registration.kind != Noun::Run
        || registration.generation.is_none_or(|value| value == 0)
        || address
            .generation
            .is_some_and(|value| Some(value) != registration.generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let mut entries = Vec::with_capacity(OperatorCapabilityKind::ALL.len());
    for (entry, kind) in linked.entries().iter().zip(OperatorCapabilityKind::ALL) {
        if entry.kind() != kind {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        let grant = match entry.native() {
            Some(native) => observation(native, kind, registration, encoder)?,
            None => object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote(kind.authority_required())),
            ]),
        };
        entries.push(
            encoder.bounded(object_json(&[
                ("capability", quote(kind.as_str())),
                (
                    "verbs",
                    format!(
                        "[{}]",
                        kind.verbs()
                            .iter()
                            .map(|verb| quote(verb.as_str()))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                ),
                (
                    "required_purpose",
                    kind.required_purpose()
                        .map_or_else(|| "null".to_owned(), |purpose| quote(purpose.as_str())),
                ),
                ("route_connected", entry.route_connected().to_string()),
                ("purpose_allowed", entry.purpose_allowed().to_string()),
                ("installed", entry.configured().to_string()),
                ("grant", grant),
            ]))?,
        );
    }
    let configuration = object_json(&[
        ("state", quote("known")),
        (
            "basis",
            object_json(&[
                ("origin", quote("host_registration")),
                ("registration", quote(&registration.canonical_string())),
            ]),
        ),
        ("value", format!("[{}]", entries.join(","))),
    ]);
    encoder.bounded(object_json(&[
        ("schema", quote("fa.operator-capabilities/1")),
        ("operation", quote("capabilities")),
        ("registration", quote(&registration.canonical_string())),
        ("query_address", quote(&address.canonical_string())),
        ("disclosure", quote("mounted_operator_grants")),
        ("current_eligibility", quote("not_observed")),
        ("unlisted_limits", quote("not_observed")),
        ("signed_profile", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("configuration", configuration),
    ]))
}

fn observation(
    native: &FileOperatorCapabilityObservation,
    kind: OperatorCapabilityKind,
    registration: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let fields = match native {
        Knowledge::Known { value, basis } => {
            if basis.scope.purpose != Purpose::Effect
                || registration.tenant != basis.scope.tenant.to_string()
                || registration.id != basis.scope.run.to_string()
                || value.limits().len() > 8
            {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            let mut limits = Vec::with_capacity(value.limits().len());
            for (&name, &value) in value.limits() {
                if !matches!(
                    name,
                    "remaining_queries"
                        | "remaining_operations"
                        | "replay_event_limit"
                        | "remaining_numerical_replay_events"
                        | "remaining_page_queries"
                        | "events_per_page"
                        | "history_event_limit"
                        | "review_limit"
                        | "case_limit"
                        | "input_byte_limit"
                ) {
                    return Err(TowerEncodingError::InvalidKnowledge);
                }
                limits.push((name, quote(&value.to_string())));
            }
            let scope = basis.scope;
            vec![
                ("state", quote("known")),
                (
                    "basis",
                    object_json(&[
                        (
                            "scope",
                            object_json(&[
                                ("tenant", quote(&scope.tenant.to_string())),
                                ("principal", quote(&scope.principal.to_string())),
                                ("run", quote(&scope.run.to_string())),
                                ("branch", quote(&scope.branch.to_string())),
                                ("authority", quote(&scope.authority.to_string())),
                                ("purpose", quote("effect")),
                            ]),
                        ),
                        (
                            "journal_revision",
                            quote(&basis.journal_revision.to_string()),
                        ),
                        ("origin", quote("original_live_grant")),
                    ]),
                ),
                ("value", object_json(&limits)),
            ]
        }
        Knowledge::Withheld { authority_required }
            if *authority_required == kind.authority_required() =>
        {
            vec![
                ("state", quote("withheld")),
                ("authority_required", quote(authority_required)),
            ]
        }
        Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        } => vec![
            ("state", quote("unknown")),
            ("reason", quote("controller_unavailable")),
        ],
        _ => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&fields))
}
