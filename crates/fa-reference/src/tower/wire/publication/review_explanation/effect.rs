//! Historical effect linkage from the same single original replay.

use super::{
    Encoder, TowerEncodingError, error_code, number, object_json, optional, quote, unknown,
};
use crate::action::consequence::delivery::persistent::Reconciliation;
use crate::action::consequence::delivery::persistent::observed::replay::ReviewEffectAudit;
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::oversight::human::HumanDisposition;

pub(super) fn encode(
    effect: &ReviewEffectAudit,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let dispatch = match effect.dispatch() {
        None => unknown("not_observed"),
        Some(dispatch) => {
            let authorization = dispatch.authorization();
            let human = dispatch.human();
            if human.disposition != HumanDisposition::Consumed || human.attempt != effect.attempt()
            {
                return Err(TowerEncodingError::InvalidKnowledge);
            }
            known(object_json(&[
                ("journal_revision", number(dispatch.revision())),
                (
                    "selected_review",
                    effect.selected_review_dispatched().to_string(),
                ),
                (
                    "automatic",
                    object_json(&[
                        ("round", number(authorization.round())),
                        ("review_revision", number(authorization.review_revision())),
                        ("authorization_revision", number(authorization.revision())),
                        ("input_revision", number(authorization.input_revision())),
                    ]),
                ),
                (
                    "human",
                    object_json(&[
                        ("request", number(human.request)),
                        ("reviewer", number(human.reviewer)),
                        ("disposition", quote("consumed")),
                        ("created_at", number(human.created_at.0)),
                        ("expires_at", number(human.expires_at.0)),
                        ("issued_at", optional(human.issued_at.map(|tick| tick.0))),
                        (
                            "finished_at",
                            optional(human.finished_at.map(|tick| tick.0)),
                        ),
                    ]),
                ),
            ]))
        }
    };
    let outcome = match effect.outcome() {
        None => unknown("not_observed"),
        Some(observed) => known(object_json(&[
            ("journal_revision", number(observed.revision())),
            ("outcome", outcome_json(observed.outcome())),
        ])),
    };
    let reconciliation = match effect.reconciliation() {
        None => unknown("not_observed"),
        Some(observed) => {
            let result = match observed.result() {
                Ok(Reconciliation::Resolved(value)) => object_json(&[
                    ("state", quote("resolved")),
                    ("outcome", outcome_json(value)),
                ]),
                Ok(Reconciliation::AwaitingResolution) => {
                    object_json(&[("state", quote("awaiting_resolution"))])
                }
                Ok(Reconciliation::RetentionExpired) => {
                    object_json(&[("state", quote("retention_expired"))])
                }
                Err(error) => object_json(&[
                    ("state", quote("refused")),
                    ("code", quote(error_code(error))),
                ]),
            };
            known(object_json(&[
                ("journal_revision", number(observed.revision())),
                ("result", result),
            ]))
        }
    };
    encoder.bounded(object_json(&[
        ("scope", quote("original_reference_publication_sink")),
        (
            "stage_at_review",
            quote(super::super::inspection::stage(effect.stage_at_review())),
        ),
        (
            "stage_at_end",
            quote(super::super::inspection::stage(effect.stage_at_end())),
        ),
        ("dispatch", dispatch),
        ("outcome", outcome),
        ("reconciliation", reconciliation),
    ]))
}

fn known(value: String) -> String {
    object_json(&[("state", quote("known")), ("value", value)])
}

fn outcome_json(value: EndpointOutcome) -> String {
    match value {
        EndpointOutcome::Executed { resulting_version } => object_json(&[
            ("kind", quote("executed")),
            ("resulting_version", number(resulting_version)),
        ]),
        EndpointOutcome::NotExecuted { reason } => object_json(&[
            ("kind", quote("not_executed")),
            (
                "reason",
                quote(match reason {
                    NonExecutionReason::Sealed => "sealed",
                    NonExecutionReason::VersionConflict => "version_conflict",
                    NonExecutionReason::DeadlineElapsed => "deadline_elapsed",
                    NonExecutionReason::StreamRejected => "stream_rejected",
                }),
            ),
        ]),
    }
}
