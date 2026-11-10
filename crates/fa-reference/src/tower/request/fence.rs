//! A complete native authority predecessor, never an authority grant.

use super::{
    DecodeResult, TowerDecodingError, decimal, field, fields, object, object_json, quote, text,
};
use crate::action::consequence::delivery::persistent::requests::actor::FileFenceBasis;
use crate::action::{Purpose, Scope};
use crate::strict_json::Json;

pub(super) fn decode_basis(value: &Json) -> DecodeResult<FileFenceBasis> {
    let value = object(value)?;
    fields(value, &["scope", "journal_revision", "control_sequence", "authority_epoch"])?;
    let scope = object(field(value, "scope")?)?;
    fields(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])?;
    if text(scope, "purpose")? != "effect" {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let basis = FileFenceBasis {
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
    };
    validate_expected(basis)?;
    Ok(basis)
}

pub(super) fn validate_expected(basis: FileFenceBasis) -> DecodeResult<()> {
    if basis.scope.purpose != Purpose::Effect
        || [basis.scope.tenant, basis.scope.principal, basis.scope.run,
            basis.scope.branch, basis.scope.authority].contains(&0)
    {
        return Err(TowerDecodingError::MalformedRequest);
    }
    // Zero is a legitimate initial sequence. Only the original owner's exact
    // comparison establishes a current authority cut.
    Ok(())
}

pub(crate) fn encode_basis(basis: FileFenceBasis) -> String {
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
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("authority_epoch", number(basis.authority_epoch)),
    ])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
