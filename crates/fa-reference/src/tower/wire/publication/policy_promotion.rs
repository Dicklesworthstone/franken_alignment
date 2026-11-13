//! Historical promotion plans and acknowledged original policy transitions.
//! Encodings contain no governor key, policy literals or effect authority.

use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::action::MAX_ATTEMPTS;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyPromotionBasis, FilePolicyPromotionBlockReason, FilePolicyPromotionError,
    FilePolicyPromotionPlan, FilePolicyPromotionReceipt,
};
use crate::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::{LinkedPolicyPromotion, LinkedPolicyPromotionNext};
use crate::tower::request::policy_promotion::{encode_basis, validate_expected};

pub(super) fn encode_next(
    linked: &LinkedPolicyPromotionNext,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    if linked.campaign() == 0
        || linked.operation() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != linked.query_idempotency_key()
        || idempotency_key.is_some_and(|key| key != linked.operation().to_string())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", next_known(linked, value, *basis, encoder)?)
        }
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-policy-promotion-next/1")),
        ("operation", quote("next")),
        ("scope", quote("original_approved_policy_campaign")),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("campaign", number(linked.campaign())),
        ("promotion_operation", number(linked.operation())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("policy_disclosed", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("idempotency_scope", quote("live_policy_promotion_grant")),
        (
            "action_completeness",
            quote("single_independently_approved_policy_campaign"),
        ),
        (field, result),
    ]))
}

fn next_known(
    linked: &LinkedPolicyPromotionNext,
    plan: &FilePolicyPromotionPlan,
    basis: FilePolicyPromotionBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    validate_basis(basis, linked.registration())?;
    if plan.basis() != basis
        || basis.campaign != linked.campaign()
        || plan.operation() != linked.operation()
        || plan.candidate_policy_generation() == 0
        || plan.can_promote() != plan.request().is_some()
        || plan.can_promote() != plan.block_reason().is_none()
        || (plan.can_promote() && plan.candidate_policy_generation() <= basis.policy_generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let action = linked
        .action()
        .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
    let action = match action {
        Some(request) => crate::tower::encode_request(&request)
            .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
        None => "null".to_owned(),
    };
    let value = encoder.bounded(object_json(&[
        (
            "candidate_policy_generation",
            number(plan.candidate_policy_generation()),
        ),
        ("can_promote", plan.can_promote().to_string()),
        (
            "block_reason",
            plan.block_reason()
                .map_or_else(|| "null".to_owned(), |reason| quote(block_reason(reason))),
        ),
        ("action", action),
        ("cost_estimate", unknown()),
        ("expected_information_gain", unknown()),
        ("ranking", unknown()),
        ("current_eligibility", unknown()),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", encode_basis(basis)),
        ("value", value),
    ]))
}

pub(super) fn encode_promotion(
    linked: &LinkedPolicyPromotion,
    address: &Address,
    operation_id: &str,
    idempotency_key: Option<&str>,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    registration(linked.registration(), linked.query_address(), address)?;
    let attempted = linked.attempted();
    validate_basis(attempted.expected(), linked.registration())?;
    if address != linked.registration()
        || attempted.operation() == 0
        || operation_id != linked.query_operation_id()
        || idempotency_key != Some(attempted.operation().to_string().as_str())
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, result) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => {
            ("knowledge", promoted_known(linked, value, *basis, encoder)?)
        }
        Ok(value) => ("knowledge", unavailable(value)?),
        Err(error) => ("error", error_json(*error)),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-policy-promotion/1")),
        ("operation", quote("promote")),
        ("scope", quote("original_approved_policy_campaign")),
        (
            "registration",
            quote(&linked.registration().canonical_string()),
        ),
        ("campaign", number(attempted.expected().campaign)),
        ("promotion_operation", number(attempted.operation())),
        ("expected", encode_basis(attempted.expected())),
        ("historical", "true".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("clock_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("policy_disclosed", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("may_write_journal", "true".to_owned()),
        ("publication_attempted", "false".to_owned()),
        ("automatic_retry", "false".to_owned()),
        ("idempotency_scope", quote("live_policy_promotion_grant")),
        (field, result),
    ]))
}

fn promoted_known(
    linked: &LinkedPolicyPromotion,
    receipt: &FilePolicyPromotionReceipt,
    basis: FilePolicyPromotionBasis,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let before = receipt.before();
    let after = receipt.after();
    validate_basis(after, linked.registration())?;
    if before != linked.attempted().expected()
        || after != basis
        || receipt.operation() != linked.attempted().operation()
        || before.scope != after.scope
        || before.campaign != after.campaign
        || before.recorded_at != after.recorded_at
        || before.journal_revision.checked_add(1) != Some(after.journal_revision)
        || before.control_sequence.checked_add(1) != Some(after.control_sequence)
        || before.authority_epoch.checked_add(1) != Some(after.authority_epoch)
        || before.policy_generation >= after.policy_generation
        || after.policy_generation != receipt.candidate_policy_generation()
        || receipt.cancelled().len() > MAX_ATTEMPTS
        || !ordered_ids(receipt.cancelled())
        || (receipt.cancelled().is_empty() && receipt.refunded_units() != 0)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let value = encoder.bounded(object_json(&[
        ("before", encode_basis(before)),
        (
            "candidate_policy_generation",
            number(receipt.candidate_policy_generation()),
        ),
        ("cancelled", ids(receipt.cancelled())),
        ("refunded_units", number(receipt.refunded_units())),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", encode_basis(after)),
        ("value", value),
    ]))
}

fn registration(
    registration: &Address,
    query: &Address,
    outer: &Address,
) -> Result<(), TowerEncodingError> {
    if query != outer
        || outer.kind != Noun::Run
        || registration.kind != Noun::Run
        || outer.without_generation() != registration.without_generation()
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || outer
            .generation
            .is_some_and(|generation| Some(generation) != registration.generation)
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn validate_basis(
    basis: FilePolicyPromotionBasis,
    registration: &Address,
) -> Result<(), TowerEncodingError> {
    if validate_expected(basis).is_err()
        || registration.tenant != basis.scope.tenant.to_string()
        || registration.id != basis.scope.run.to_string()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    Ok(())
}

fn ordered_ids(values: &[u64]) -> bool {
    values.first().is_none_or(|value| *value != 0)
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ids(values: &[u64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| number(*value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn unavailable<T>(
    knowledge: &Knowledge<T, FilePolicyPromotionBasis>,
) -> Result<String, TowerEncodingError> {
    match knowledge {
        Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        } => Ok(unknown()),
        Knowledge::Withheld {
            authority_required: "supervisor_policy_promotion",
        } => Ok(object_json(&[
            ("state", quote("withheld")),
            ("authority_required", quote("supervisor_policy_promotion")),
        ])),
        _ => Err(TowerEncodingError::InvalidKnowledge),
    }
}

fn unknown() -> String {
    object_json(&[
        ("state", quote("unknown")),
        ("reason", quote("controller_unavailable")),
    ])
}

fn error_json(error: FilePolicyPromotionError) -> String {
    let code = match error {
        FilePolicyPromotionError::MalformedRequest => "malformed_request",
        FilePolicyPromotionError::IdempotencyConflict => "idempotency_conflict",
        FilePolicyPromotionError::StalePredecessor => "stale_predecessor",
        FilePolicyPromotionError::Capacity => "capacity",
        FilePolicyPromotionError::Unsupported => "unsupported",
        FilePolicyPromotionError::Unavailable => "unavailable",
    };
    object_json(&[("code", quote(code))])
}

fn block_reason(reason: FilePolicyPromotionBlockReason) -> &'static str {
    match reason {
        FilePolicyPromotionBlockReason::Stopped => "stopped",
        FilePolicyPromotionBlockReason::CampaignNotApproved => "campaign_not_approved",
        FilePolicyPromotionBlockReason::SourceUnavailable => "source_unavailable",
        FilePolicyPromotionBlockReason::StaleCampaign => "stale_campaign",
        FilePolicyPromotionBlockReason::NumericalReplayUnsupported => {
            "numerical_replay_unsupported"
        }
        FilePolicyPromotionBlockReason::NumericalReplayLimit => "numerical_replay_limit",
        FilePolicyPromotionBlockReason::ReplayLimit => "replay_limit",
        FilePolicyPromotionBlockReason::OperationLimit => "operation_limit",
        FilePolicyPromotionBlockReason::OperationAlreadyUsed => "operation_already_used",
    }
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
