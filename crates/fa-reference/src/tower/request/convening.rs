//! Complete original held-review predecessor; data alone conveys no grant.
use super::{
    DecodeResult, TowerDecodingError, decimal, field, fields, object, object_json, quote, text,
};
use crate::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use crate::action::{ElapsedTick, Purpose, Scope};
use crate::strict_json::Json;

pub(super) fn decode_basis(value: &Json) -> DecodeResult<FileConveneBasis> {
    let value = object(value)?;
    fields(
        value,
        &[
            "scope",
            "request",
            "request_generation",
            "attempt",
            "held_round",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "input_revision",
            "recorded_at",
            "deadline",
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
    let recorded_at = match field(value, "recorded_at")? {
        Json::Null => None,
        _ => Some(ElapsedTick(decimal(value, "recorded_at")?)),
    };
    let basis = FileConveneBasis {
        scope: Scope {
            tenant: decimal(scope, "tenant")?,
            principal: decimal(scope, "principal")?,
            run: decimal(scope, "run")?,
            branch: decimal(scope, "branch")?,
            authority: decimal(scope, "authority")?,
            purpose: Purpose::Effect,
        },
        request: decimal(value, "request")?,
        request_generation: decimal(value, "request_generation")?,
        attempt: decimal(value, "attempt")?,
        held_round: decimal(value, "held_round")?,
        journal_revision: decimal(value, "journal_revision")?,
        control_sequence: decimal(value, "control_sequence")?,
        authority_epoch: decimal(value, "authority_epoch")?,
        input_revision: decimal(value, "input_revision")?,
        recorded_at,
        deadline: ElapsedTick(decimal(value, "deadline")?),
    };
    validate_expected(basis)?;
    Ok(basis)
}

pub(super) fn validate_expected(basis: FileConveneBasis) -> DecodeResult<()> {
    if basis.scope.purpose != Purpose::Effect
        || [
            basis.scope.tenant,
            basis.scope.principal,
            basis.scope.run,
            basis.scope.branch,
            basis.scope.authority,
            basis.request,
            basis.attempt,
            basis.held_round,
        ]
        .contains(&0)
    {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

pub(crate) fn encode_basis(basis: FileConveneBasis) -> String {
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
        ("request", number(basis.request)),
        ("request_generation", number(basis.request_generation)),
        ("attempt", number(basis.attempt)),
        ("held_round", number(basis.held_round)),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("authority_epoch", number(basis.authority_epoch)),
        ("input_revision", number(basis.input_revision)),
        (
            "recorded_at",
            basis
                .recorded_at
                .map_or_else(|| "null".into(), |at| number(at.0)),
        ),
        ("deadline", number(basis.deadline.0)),
    ])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
