//! Exact native predecessor for one independently selected Andon budget.
//! A budget assertion must match the independent grant; it cannot select a new
//! window, threshold, price or accounting owner.

use super::{
    DecodeResult, TowerDecodingError, decimal, field, fields, object, object_json, quote, text,
};
use crate::action::consequence::delivery::persistent::requests::actor::FileAndonWindowBasis;
use crate::action::{ElapsedTick, Purpose, Scope};
use crate::round::review::andon::{AndonBudget, LedgerIdentity};
use crate::strict_json::Json;

pub(super) fn decode_basis(value: &Json) -> DecodeResult<FileAndonWindowBasis> {
    let value = object(value)?;
    fields(
        value,
        &[
            "scope",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "evaluation_revision",
            "congress_generation",
            "policy_generation",
            "recorded_at",
            "accounting",
        ],
    )?;
    let scope = object(field(value, "scope")?)?;
    fields(
        scope,
        &[
            "tenant",
            "principal",
            "run",
            "branch",
            "authority",
            "purpose",
        ],
    )?;
    if text(scope, "purpose")? != "effect" {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let accounting = object(field(value, "accounting")?)?;
    fields(accounting, &["deployment", "evaluator"])?;
    let basis = FileAndonWindowBasis {
        scope: Scope {
            tenant: decimal(scope, "tenant")?,
            principal: decimal(scope, "principal")?,
            run: decimal(scope, "run")?,
            branch: decimal(scope, "branch")?,
            authority: decimal(scope, "authority")?,
            purpose: Purpose::Effect,
        },
        journal_revision: decimal(value, "journal_revision")?,
        control_sequence: decimal(value, "control_sequence")?,
        authority_epoch: decimal(value, "authority_epoch")?,
        evaluation_revision: decimal(value, "evaluation_revision")?,
        congress_generation: decimal(value, "congress_generation")?,
        policy_generation: decimal(value, "policy_generation")?,
        recorded_at: match field(value, "recorded_at")? {
            Json::Null => None,
            _ => Some(ElapsedTick(decimal(value, "recorded_at")?)),
        },
        accounting: LedgerIdentity {
            deployment: decimal(accounting, "deployment")?,
            evaluator: decimal(accounting, "evaluator")?,
        },
    };
    validate_expected(basis)?;
    Ok(basis)
}

pub(crate) fn validate_expected(basis: FileAndonWindowBasis) -> DecodeResult<()> {
    if basis.scope.purpose != Purpose::Effect
        || [
            basis.scope.tenant,
            basis.scope.principal,
            basis.scope.run,
            basis.scope.branch,
            basis.scope.authority,
            basis.congress_generation,
            basis.policy_generation,
            basis.accounting.deployment,
            basis.accounting.evaluator,
        ]
        .contains(&0)
    {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

pub(super) fn decode_budget(value: &Json) -> DecodeResult<AndonBudget> {
    let value = object(value)?;
    fields(
        value,
        &[
            "window",
            "start",
            "end",
            "false_stops_allowed",
            "cost_per_stop",
        ],
    )?;
    let budget = AndonBudget {
        window: decimal(value, "window")?,
        start: decimal(value, "start")?,
        end: decimal(value, "end")?,
        false_stops_allowed: decimal(value, "false_stops_allowed")?,
        cost_per_stop: decimal(value, "cost_per_stop")?,
    };
    validate_budget(budget)?;
    Ok(budget)
}

pub(crate) fn validate_budget(budget: AndonBudget) -> DecodeResult<()> {
    if budget.window == 0 || budget.start >= budget.end {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

pub(crate) fn encode_basis(basis: FileAndonWindowBasis) -> String {
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
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("authority_epoch", number(basis.authority_epoch)),
        ("evaluation_revision", number(basis.evaluation_revision)),
        ("congress_generation", number(basis.congress_generation)),
        ("policy_generation", number(basis.policy_generation)),
        (
            "recorded_at",
            basis
                .recorded_at
                .map_or_else(|| "null".to_owned(), |tick| number(tick.0)),
        ),
        (
            "accounting",
            object_json(&[
                ("deployment", number(basis.accounting.deployment)),
                ("evaluator", number(basis.accounting.evaluator)),
            ]),
        ),
    ])
}

pub(crate) fn encode_budget(budget: AndonBudget) -> String {
    object_json(&[
        ("window", number(budget.window)),
        ("start", number(budget.start)),
        ("end", number(budget.end)),
        ("false_stops_allowed", number(budget.false_stops_allowed)),
        ("cost_per_stop", number(budget.cost_per_stop)),
    ])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
