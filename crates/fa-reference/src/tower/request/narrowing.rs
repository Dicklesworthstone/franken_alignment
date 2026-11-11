//! Closed original-run narrowing data. Wire restrictions have one canonical
//! ordering; numeric scope, targets and predecessors never supply authority.

use super::{
    DecodeResult, MAX_NARROWING_TARGETS, TowerDecodingError, array_json, decimal,
    decode_target, encode_target, field, fields, object, object_json, quote, text,
};
use crate::action::consequence::delivery::persistent::requests::actor::FileNarrowBasis;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::strict_json::Json;

pub(super) fn decode_basis(value: &Json) -> DecodeResult<FileNarrowBasis> {
    let value = object(value)?;
    fields(value, &[
        "scope", "journal_revision", "control_sequence", "authority_epoch", "recorded_at",
    ])?;
    let scope = object(field(value, "scope")?)?;
    fields(scope, &["tenant", "principal", "run", "branch", "authority", "purpose"])?;
    if text(scope, "purpose")? != "effect" {
        return Err(TowerDecodingError::MalformedRequest);
    }
    let basis = FileNarrowBasis {
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
        recorded_at: match field(value, "recorded_at")? {
            Json::Null => None,
            _ => Some(ElapsedTick(decimal(value, "recorded_at")?)),
        },
    };
    validate_expected(basis)?;
    Ok(basis)
}

pub(crate) fn validate_expected(basis: FileNarrowBasis) -> DecodeResult<()> {
    if basis.scope.purpose != Purpose::Effect
        || [basis.scope.tenant, basis.scope.principal, basis.scope.run,
            basis.scope.branch, basis.scope.authority].contains(&0)
    {
        return Err(TowerDecodingError::MalformedRequest);
    }
    // Zero is a legitimate initial predecessor; None is an unobserved clock.
    // Neither is converted into a fresh observation by parsing or encoding.
    Ok(())
}

pub(super) fn decode_targets(value: &Json) -> DecodeResult<Vec<ResolvedTarget>> {
    let values = value.as_array().ok_or(TowerDecodingError::MalformedRequest)?;
    if values.len() > MAX_NARROWING_TARGETS {
        return Err(TowerDecodingError::Capacity);
    }
    let targets = values.iter().map(decode_target).collect::<DecodeResult<Vec<_>>>()?;
    validate_targets(&targets)?;
    Ok(targets)
}

pub(crate) fn validate_targets(targets: &[ResolvedTarget]) -> DecodeResult<()> {
    if targets.len() > MAX_NARROWING_TARGETS {
        return Err(TowerDecodingError::Capacity);
    }
    if !ordered_targets(targets) {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

pub(crate) fn target_key(target: &ResolvedTarget) -> [u64; 5] {
    [target.adapter, target.object, target.contract_version,
        target.expected_version, target.generation]
}

pub(crate) fn ordered_targets(targets: &[ResolvedTarget]) -> bool {
    targets.iter().all(|target| !target_key(target).contains(&0))
        && targets.windows(2).all(|pair| target_key(&pair[0]) < target_key(&pair[1]))
}

pub(crate) fn encode_targets(targets: &[ResolvedTarget]) -> String {
    array_json(targets.iter().map(encode_target))
}

pub(crate) fn encode_basis(basis: FileNarrowBasis) -> String {
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
        ("recorded_at", basis.recorded_at.map_or_else(|| "null".to_owned(), |tick| number(tick.0))),
    ])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
