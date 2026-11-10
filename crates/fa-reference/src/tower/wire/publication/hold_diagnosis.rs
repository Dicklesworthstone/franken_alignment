//! Preserve a native diagnosis as partial recorded conditions, never permission.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileHoldCondition, FileHoldDiagnosis, FileHoldDiagnosisBasis, FileHoldPredicate,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedHoldDiagnosis, request_address_for};

const CONDITIONS: [FileHoldCondition; 10] = [
    FileHoldCondition::RecordedAdmissionOpen,
    FileHoldCondition::RecordedNotSuspended,
    FileHoldCondition::RecordedPolicyEpochMatches,
    FileHoldCondition::RecordedDeadlineOpen,
    FileHoldCondition::RecordedStageAuthorized,
    FileHoldCondition::DispatchNotRecorded,
    FileHoldCondition::CurrentSourceEvidence,
    FileHoldCondition::CurrentApprovals,
    FileHoldCondition::CurrentClock,
    FileHoldCondition::FinalPublicationEligibility,
];

pub(super) fn encode(
    linked: &LinkedHoldDiagnosis,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let registration = linked.registration();
    if address != linked.query_address()
        || address.kind != Noun::Attempt
        || address.tenant != registration.tenant
        || address
            .generation
            .is_some_and(|generation| Some(generation) != registration.generation)
        || registration.kind != Noun::Run
        || registration.generation.is_none_or(|generation| generation == 0)
        || linked.request() == 0
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, value) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge",
            known(value, *basis, linked, encoder)?,
        ),
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        }) => ("knowledge", unknown()),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_hold_diagnosis",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_hold_diagnosis")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-hold-diagnosis/1")),
        ("operation", quote("why-held")),
        ("scope", quote("recorded_publication_state")),
        ("request", number(linked.request())),
        ("registration", quote(&registration.canonical_string())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("current_eligibility", quote("not_observed")),
        ("blocker_completeness", quote("recorded_conditions_only")),
        (field, value),
    ]))
}

fn known(
    diagnosis: &FileHoldDiagnosis,
    basis: FileHoldDiagnosisBasis,
    linked: &LinkedHoldDiagnosis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let expected = request_address_for(basis.scope, linked.registration(), basis.request)
        .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    if basis != diagnosis.basis()
        || basis.request != linked.request()
        || basis.request == 0
        || basis.attempt == 0
        || basis.scope.purpose != Purpose::Effect
        || expected.without_generation() != linked.query_address().without_generation()
        || diagnosis.predicates().len() != CONDITIONS.len()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let predicates = diagnosis
        .predicates()
        .iter()
        .zip(CONDITIONS)
        .map(|(predicate, condition)| {
            if predicate.condition() != condition {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            predicate_json(predicate, basis, encoder)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = encoder.bounded(object_json(&[
        ("stage", quote(super::inspection::stage(diagnosis.stage()))),
        ("proposal_policy_epoch", number(diagnosis.proposal_policy_epoch())),
        ("current_policy_epoch", number(diagnosis.current_policy_epoch())),
        ("deadline", number(diagnosis.deadline().0)),
        ("predicates", format!("[{}]", predicates.join(","))),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", value),
    ]))
}

fn predicate_json(
    predicate: &FileHoldPredicate,
    basis: FileHoldDiagnosisBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let condition = predicate.condition();
    let observed = match condition {
        FileHoldCondition::RecordedDeadlineOpen => basis.recorded_at.is_some(),
        FileHoldCondition::CurrentSourceEvidence
        | FileHoldCondition::CurrentApprovals
        | FileHoldCondition::CurrentClock
        | FileHoldCondition::FinalPublicationEligibility => false,
        _ => true,
    };
    let satisfied = match (predicate.satisfied(), observed) {
        (Knowledge::Known { value, basis: native }, true) if *native == basis => object_json(&[
            ("state", quote("known")),
            ("basis", basis_json(*native)),
            ("value", value.to_string()),
        ]),
        (
            Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable,
            },
            false,
        ) => unknown(),
        _ => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("condition", quote(condition_name(condition))),
        ("satisfied", satisfied),
    ]))
}

fn condition_name(condition: FileHoldCondition) -> &'static str {
    match condition {
        FileHoldCondition::RecordedAdmissionOpen => "recorded_admission_open",
        FileHoldCondition::RecordedNotSuspended => "recorded_not_suspended",
        FileHoldCondition::RecordedPolicyEpochMatches => "recorded_policy_epoch_matches",
        FileHoldCondition::RecordedDeadlineOpen => "recorded_deadline_open",
        FileHoldCondition::RecordedStageAuthorized => "recorded_stage_authorized",
        FileHoldCondition::DispatchNotRecorded => "dispatch_not_recorded",
        FileHoldCondition::CurrentSourceEvidence => "current_source_evidence",
        FileHoldCondition::CurrentApprovals => "current_approvals",
        FileHoldCondition::CurrentClock => "current_clock",
        FileHoldCondition::FinalPublicationEligibility => "final_publication_eligibility",
    }
}

fn basis_json(basis: FileHoldDiagnosisBasis) -> String {
    let scope = basis.scope;
    object_json(&[
        (
            "scope",
            object_json(&[
                ("tenant", number(scope.tenant)),
                ("principal", number(scope.principal)),
                ("run", number(scope.run)),
                ("branch", number(scope.branch)),
                ("authority", number(scope.authority)),
                ("purpose", quote("effect")),
            ]),
        ),
        ("origin", quote("acknowledged_owner")),
        ("request", number(basis.request)),
        ("request_generation", number(basis.request_generation)),
        ("attempt", number(basis.attempt)),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        (
            "recorded_at",
            basis.recorded_at.map_or_else(|| "null".to_owned(), |tick| number(tick.0)),
        ),
    ])
}

fn unknown() -> String {
    object_json(&[
        ("state", quote("unknown")),
        ("reason", quote("controller_unavailable")),
    ])
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}
