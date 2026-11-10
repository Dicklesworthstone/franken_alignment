//! Original completed evidence refinement, never an attempted application.
use super::{
    CongressView, Encoder, TowerEncodingError, basis_json, congress, decision, effect, evaluation,
    hex, number, object_json, policy, quote, target,
};
use crate::action::Purpose;
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileRefinementExplanation, FileReviewExplanationBasis,
};
use crate::action::consequence::oversight::actor::Knowledge;
use crate::address_resolver::{Address, Noun};
use crate::tower::publication::LinkedRefinementExplanation;

pub(in crate::tower::wire::publication) fn encode(
    linked: &LinkedRefinementExplanation,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    if address != linked.registration() || linked.round() == 0 {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    // Unknown completion kinds keep the existing review response. Only the
    // original sealed Known refinement can select this stronger result kind.
    let Ok(Knowledge::Known { value, basis }) = linked.native() else {
        return Err(TowerEncodingError::InvalidKnowledge);
    };
    let knowledge = known(value, *basis, address, linked.round(), encoder)?;
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-refinement-explanation/1")),
        ("operation", quote("explain")),
        ("scope", quote("historical_completed_refinement")),
        ("round", number(linked.round())),
        ("historical", "true".to_owned()),
        ("helpers_rerun", "false".to_owned()),
        ("live_evidence_read", "false".to_owned()),
        ("effect_authority", "false".to_owned()),
        ("payload_disclosed", "false".to_owned()),
        ("witness_values_disclosed", "false".to_owned()),
        ("helper_inputs_disclosed", "false".to_owned()),
        ("commitment_salts_disclosed", "false".to_owned()),
        ("current_eligibility", quote("not_observed")),
        ("current_hold_reason", super::unknown("not_observed")),
        ("knowledge", knowledge),
    ]))
}

fn known(
    value: &FileRefinementExplanation,
    basis: FileReviewExplanationBasis,
    address: &Address,
    round: u64,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let scope = basis.scope;
    let subject = format!(
        "request-{}-{}-{}-{}-{}",
        scope.run, scope.principal, scope.branch, scope.authority, basis.request
    );
    if basis != value.basis()
        || basis.round != round
        || basis.request == 0
        || basis.attempt == 0
        || address.kind != Noun::Attempt
        || address.tenant != scope.tenant.to_string()
        || address.id != subject
        || scope.purpose != Purpose::Effect
        || basis.begin_revision >= basis.completed_revision
        || basis.completed_revision > basis.journal_revision
        || basis.begin_control_sequence > basis.completed_control_sequence
        || basis.completed_control_sequence > basis.control_sequence
        || value.refined_input_revision() <= value.reviewed_input_revision()
        || value.input_revision() != value.reviewed_input_revision()
        || value.effect().attempt() != basis.attempt
        || value.effect().stage_at_review() != value.completed_state()
        || value.effect().stage_at_end() != value.ending_state()
        || value.effect().selected_review_dispatched()
        || value.effect().dispatch().is_some_and(|dispatch| {
            dispatch.authorization().round() == round
                || dispatch.authorization().input_revision() < value.refined_input_revision()
        })
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let group = value.group();
    let body = encoder.bounded(object_json(&[
        (
            "action",
            object_json(&[
                ("policy_epoch", number(value.proposal_policy_epoch())),
                ("semantic_epoch", number(value.semantic_epoch())),
                (
                    "target",
                    value.target().map_or_else(|| "null".to_owned(), target),
                ),
                ("units", number(value.units())),
                ("payload_bytes", number(value.payload_bytes())),
            ]),
        ),
        ("policy", encoder.bounded(policy::encode(value.policy()))?),
        (
            "evaluation",
            evaluation(value.evaluation(), value.policy(), encoder)?,
        ),
        (
            "congress",
            congress(
                CongressView {
                    policy: value.congress(),
                    tally: value.tally(),
                    commits: value.commits(),
                    commit_times: value.commit_times(),
                    reveals: value.reveals(),
                    missing: value.missing(),
                    abstained: value.abstained(),
                },
                encoder,
            )?,
        ),
        ("decision", decision(value.decision(), encoder)?),
        (
            "application",
            object_json(&[("state", quote("not_attempted"))]),
        ),
        ("effect", effect::encode(value.effect(), encoder)?),
        ("input_revision", number(value.reviewed_input_revision())),
        ("evidence_root", hex(&value.evidence_root())),
        (
            "timing",
            object_json(&[
                ("started_at", number(basis.started_at.0)),
                ("commit_by", number(value.window().commit_by.0)),
                ("reveal_by", number(value.window().reveal_by.0)),
                ("reveals_opened_at", number(value.reveals_opened_at().0)),
                ("completed_at", number(value.review_completed_at().0)),
            ]),
        ),
        (
            "refinement",
            object_json(&[
                ("kind", quote("kv_exact_residual")),
                (
                    "group",
                    object_json(&[
                        ("layer", number(group.row.layer)),
                        (
                            "side",
                            quote(match group.row.side {
                                KvSide::Key => "key",
                                KvSide::Value => "value",
                            }),
                        ),
                        ("position", number(group.row.position)),
                        ("head", number(group.head)),
                    ]),
                ),
                (
                    "reviewed_input_revision",
                    number(value.reviewed_input_revision()),
                ),
                (
                    "refined_input_revision",
                    number(value.refined_input_revision()),
                ),
                ("requires_fresh_review", "true".to_owned()),
            ]),
        ),
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", body),
    ]))
}
