//! The full original acknowledged cut carried by a native reconciliation request.
//! Parsing is data validation only; this object never confers an owner or grant.

use super::{DecodeResult, TowerDecodingError, decimal, field, fields, object, object_json, quote, text};
use crate::action::consequence::delivery::persistent::requests::actor::FileHoldDiagnosisBasis;
use crate::action::{ElapsedTick, Purpose, Scope};
use crate::strict_json::Json;

pub(super) fn decode_basis(value: &Json) -> DecodeResult<FileHoldDiagnosisBasis> {
    let value = object(value)?;
    fields(value, &[
        "scope", "origin", "request", "request_generation", "attempt",
        "journal_revision", "control_sequence", "recorded_at",
    ])?;
    if text(value, "origin")? != "acknowledged_owner" {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let scope = object(field(value, "scope")?)?;
    fields(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])?;
    if text(scope, "purpose")? != "effect" {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let recorded_at = match field(value, "recorded_at")? {
        Json::Null => None,
        Json::String(_) => Some(ElapsedTick(decimal(value, "recorded_at")?)),
        _ => return Err(TowerDecodingError::MalformedRequest),
    };
    let basis = FileHoldDiagnosisBasis {
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
        journal_revision: decimal(value, "journal_revision")?,
        control_sequence: decimal(value, "control_sequence")?,
        recorded_at,
    };
    validate_expected(basis)?;
    Ok(basis)
}

pub(super) fn validate_expected(basis: FileHoldDiagnosisBasis) -> DecodeResult<()> {
    if basis.scope.purpose != Purpose::Effect || basis.request == 0 || basis.attempt == 0 {
        return Err(TowerDecodingError::MalformedRequest);
    }
    // The request's initial generation and control sequence may both be zero.
    // Native comparison, not arithmetic here, establishes the current cut.
    Ok(())
}

pub(crate) fn encode_basis(basis: FileHoldDiagnosisBasis) -> String {
    let scope = basis.scope;
    object_json(&[
        ("scope", object_json(&[
            ("tenant", number(scope.tenant)),
            ("principal", number(scope.principal)),
            ("run", number(scope.run)),
            ("branch", number(scope.branch)),
            ("authority", number(scope.authority)),
            ("purpose", quote("effect")),
        ])),
        ("origin", quote("acknowledged_owner")),
        ("request", number(basis.request)),
        ("request_generation", number(basis.request_generation)),
        ("attempt", number(basis.attempt)),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("recorded_at", basis.recorded_at.map_or_else(|| "null".to_owned(), |tick| number(tick.0))),
    ])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
