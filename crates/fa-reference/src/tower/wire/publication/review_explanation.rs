//! Original completed review, reducer trace and actual application/effect cut.
//! The native replay validates history. This module only projects its private,
//! bound observation; it never evaluates policy or tallies votes a second time.

mod effect;

use super::policy_replay::{bounded_array, evaluation, target};
use super::{Encoder, TowerEncodingError, object_json, quote};
use crate::Error;
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileReviewExplanation, FileReviewExplanationBasis,
};
use crate::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use crate::action::consequence::{Consequence, Restriction, Rule};
use crate::address_resolver::{Address, Noun};
use crate::reducer::Outcome;
use crate::round::Verdict;
use crate::tower::publication::LinkedReviewExplanation;
use crate::tower::request::policy;
use std::collections::BTreeMap;

pub(super) fn encode(
    linked: &LinkedReviewExplanation,
    address: &Address,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    if address != linked.registration() || linked.round() == 0 {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
    let (field, value) = match linked.native() {
        Ok(Knowledge::Known { value, basis }) => (
            "knowledge",
            known(value, *basis, address, linked.round(), encoder)?,
        ),
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        }) => ("knowledge", unknown("controller_unavailable")),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_review_explanation",
        }) => (
            "knowledge",
            object_json(&[
                ("state", quote("withheld")),
                ("authority_required", quote("supervisor_review_explanation")),
            ]),
        ),
        Err(ActorError::Capacity) => ("error", object_json(&[("code", quote("capacity"))])),
        Err(ActorError::Unavailable) => ("error", object_json(&[("code", quote("unavailable"))])),
        Ok(_) | Err(_) => return Err(TowerEncodingError::InvalidKnowledge),
    };
    encoder.bounded(object_json(&[
        ("schema", quote("fa.publication-review-explanation/1")),
        ("operation", quote("explain")),
        ("scope", quote("historical_completed_review")),
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
        ("current_hold_reason", unknown("not_observed")),
        (field, value),
    ]))
}

fn known(
    value: &FileReviewExplanation,
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
        || value.effect().attempt() != basis.attempt
        || value.effect().stage_at_review() != value.completed_state()
        || value.effect().stage_at_end() != value.ending_state()
    {
        return Err(TowerEncodingError::InvalidKnowledge);
    }
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
        ("congress", congress(value, encoder)?),
        ("decision", decision(value, encoder)?),
        ("application", application(value)),
        ("effect", effect::encode(value.effect(), encoder)?),
        ("input_revision", number(value.input_revision())),
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
    ]))?;
    encoder.bounded(object_json(&[
        ("state", quote("known")),
        ("basis", basis_json(basis)),
        ("value", body),
    ]))
}

fn basis_json(basis: FileReviewExplanationBasis) -> String {
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
        ("attempt", number(basis.attempt)),
        ("round", number(basis.round)),
        ("begin_revision", number(basis.begin_revision)),
        (
            "begin_control_sequence",
            number(basis.begin_control_sequence),
        ),
        ("started_at", number(basis.started_at.0)),
        ("completed_revision", number(basis.completed_revision)),
        (
            "completed_control_sequence",
            number(basis.completed_control_sequence),
        ),
        (
            "completed_at",
            optional(basis.completed_at.map(|tick| tick.0)),
        ),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        (
            "recorded_at",
            optional(basis.recorded_at.map(|tick| tick.0)),
        ),
    ])
}

fn congress(
    value: &FileReviewExplanation,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let policy = value.congress();
    let tally = value.tally();
    // Index original accepted events once; no per-member scan or re-reduction.
    let commits: BTreeMap<_, _> = value
        .commits()
        .iter()
        .zip(value.commit_times())
        .map(|(record, at)| (record.member.as_str(), *at))
        .collect();
    let reveals: BTreeMap<_, _> = value
        .reveals()
        .map(|(member, verdict, at)| (member, (verdict, at)))
        .collect();
    let members = bounded_array(
        policy.members.iter().map(|(member, profile)| {
            let revealed = reveals.get(member.as_str());
            let outcome = revealed.map_or("missing", |(verdict, _)| match verdict {
                Verdict::Allow => "allow",
                Verdict::Hold => "hold",
                Verdict::Deny => "deny",
                Verdict::Abstain => "abstained",
            });
            Ok(object_json(&[
                ("member", quote(member)),
                ("cohort", quote(&profile.cohort)),
                ("declared_weight", number(profile.weight)),
                (
                    "admitted_weight",
                    tally
                        .admitted_weights
                        .get(member)
                        .map_or_else(|| "null".to_owned(), number),
                ),
                ("outcome", quote(outcome)),
                (
                    "committed_at",
                    optional(commits.get(member.as_str()).map(|at| at.0)),
                ),
                ("revealed_at", optional(revealed.map(|(_, at)| at.0))),
            ]))
        }),
        encoder,
    )?;
    let cohorts = bounded_array(
        tally
            .admitted_cohort_weights
            .iter()
            .map(|(cohort, weight)| {
                Ok(object_json(&[
                    ("cohort", quote(cohort)),
                    ("admitted_weight", number(weight)),
                ]))
            }),
        encoder,
    )?;
    encoder.bounded(object_json(&[
        ("generation", number(policy.generation)),
        ("member_count", number(policy.members.len())),
        ("commit_count", number(value.commits().len())),
        ("reveal_count", number(reveals.len())),
        ("members", members),
        ("cohorts", cohorts),
        (
            "caps",
            object_json(&[
                ("per_member", number(policy.caps.per_member)),
                ("per_cohort", number(policy.caps.per_cohort)),
            ]),
        ),
        (
            "thresholds",
            object_json(&[
                ("continue_minimum", number(policy.continue_minimum)),
                (
                    "continue_hold_maximum",
                    number(policy.continue_hold_maximum),
                ),
                ("narrow_at", number(policy.narrow_at)),
                ("suspend_at", number(policy.suspend_at)),
                ("minimum_members", number(policy.minimum_members)),
                ("minimum_cohorts", number(policy.minimum_cohorts)),
            ]),
        ),
        ("permit_weight", number(tally.permit_weight)),
        ("hold_weight", number(tally.hold_weight)),
        (
            "tally_kind",
            quote(match tally.outcome {
                Outcome::Empirical => "empirical",
                Outcome::Disqualified => "disqualified",
            }),
        ),
        (
            "missing",
            bounded_array(value.missing().iter().map(|name| Ok(quote(name))), encoder)?,
        ),
        (
            "abstained",
            bounded_array(
                value.abstained().iter().map(|name| Ok(quote(name))),
                encoder,
            )?,
        ),
    ]))
}

fn decision(
    value: &FileReviewExplanation,
    encoder: &Encoder,
) -> Result<String, TowerEncodingError> {
    let decision = value.decision();
    let requires = decision.consequence.requires();
    encoder.bounded(object_json(&[
        ("consequence", quote(consequence(decision.consequence))),
        (
            "authority_floor",
            quote(match requires.authority_floor {
                Restriction::Continue => "continue",
                Restriction::HoldEffect => "hold_effect",
                Restriction::Deny => "deny",
                Restriction::NarrowAuthority => "narrow_authority",
                Restriction::SuspendRun => "suspend_run",
            }),
        ),
        (
            "functional_restart_required",
            requires.functional_restart_required.to_string(),
        ),
        (
            "rules",
            bounded_array(
                decision.rules.iter().map(|rule| {
                    Ok(quote(match rule {
                        Rule::EmpiricalRecommendation => "empirical_recommendation",
                        Rule::ExactDisqualifier => "exact_disqualifier",
                        Rule::MandatoryAbsent => "mandatory_absent",
                        Rule::Contradiction => "contradiction",
                    }))
                }),
                encoder,
            )?,
        ),
    ]))
}

fn application(value: &FileReviewExplanation) -> String {
    match value.application() {
        Ok(applied) => object_json(&[
            ("state", quote("applied")),
            ("control_sequence", number(applied.control_sequence)),
            ("input_revision", number(applied.input_revision)),
            ("consequence", quote(consequence(applied.consequence))),
        ]),
        Err(error) => object_json(&[
            ("state", quote("refused")),
            ("code", quote(error_code(error))),
        ]),
    }
}

fn consequence(value: Consequence) -> &'static str {
    match value {
        Consequence::Continue => "continue",
        Consequence::HoldEffect => "hold_effect",
        Consequence::Deny => "deny",
        Consequence::NarrowAuthority => "narrow_authority",
        Consequence::SuspendRun => "suspend_run",
        Consequence::ResetToCheckpoint => "reset_to_checkpoint",
    }
}

fn error_code(value: Error) -> &'static str {
    match value {
        Error::InvalidInput => "invalid_input",
        Error::Incomplete => "incomplete",
        Error::Limit => "limit",
        Error::Overflow => "overflow",
        Error::Duplicate => "duplicate",
        Error::Missing => "missing",
        Error::WrongState => "wrong_state",
        Error::Stale => "stale",
        Error::Binding => "binding",
    }
}

fn unknown(reason: &str) -> String {
    object_json(&[("state", quote("unknown")), ("reason", quote(reason))])
}

fn number(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), number)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("writing to a String");
    }
    quote(&out)
}
