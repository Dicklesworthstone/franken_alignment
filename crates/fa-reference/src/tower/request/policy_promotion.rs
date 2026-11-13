//! Exact data for one independently approved original policy campaign.
//! An address or decoded predecessor can never manufacture a promotion key.

use super::{
    DecodeResult, TowerDecodingError, decimal, field, fields, object, object_json, quote, text,
};
use crate::action::consequence::delivery::persistent::requests::actor::FilePolicyPromotionBasis;
use crate::action::{ElapsedTick, Purpose, Scope};
use crate::strict_json::Json;

pub(super) fn decode_basis(value: &Json) -> DecodeResult<FilePolicyPromotionBasis> {
    let value = object(value)?;
    fields(
        value,
        &[
            "scope",
            "campaign",
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "policy_generation",
            "recorded_at",
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
    let basis = FilePolicyPromotionBasis {
        scope: Scope {
            tenant: decimal(scope, "tenant")?,
            principal: decimal(scope, "principal")?,
            run: decimal(scope, "run")?,
            branch: decimal(scope, "branch")?,
            authority: decimal(scope, "authority")?,
            purpose: Purpose::Effect,
        },
        campaign: decimal(value, "campaign")?,
        journal_revision: decimal(value, "journal_revision")?,
        control_sequence: decimal(value, "control_sequence")?,
        authority_epoch: decimal(value, "authority_epoch")?,
        policy_generation: decimal(value, "policy_generation")?,
        recorded_at: match field(value, "recorded_at")? {
            Json::Null => None,
            _ => Some(ElapsedTick(decimal(value, "recorded_at")?)),
        },
    };
    validate_expected(basis)?;
    Ok(basis)
}

pub(crate) fn validate_expected(basis: FilePolicyPromotionBasis) -> DecodeResult<()> {
    if basis.scope.purpose != Purpose::Effect
        || [
            basis.scope.tenant,
            basis.scope.principal,
            basis.scope.run,
            basis.scope.branch,
            basis.scope.authority,
            basis.campaign,
            basis.policy_generation,
        ]
        .contains(&0)
    {
        return Err(TowerDecodingError::MalformedRequest);
    }
    Ok(())
}

pub(crate) fn encode_basis(basis: FilePolicyPromotionBasis) -> String {
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
        ("campaign", number(basis.campaign)),
        ("journal_revision", number(basis.journal_revision)),
        ("control_sequence", number(basis.control_sequence)),
        ("authority_epoch", number(basis.authority_epoch)),
        ("policy_generation", number(basis.policy_generation)),
        (
            "recorded_at",
            basis
                .recorded_at
                .map_or_else(|| "null".to_owned(), |tick| number(tick.0)),
        ),
    ])
}

fn number(value: u64) -> String {
    quote(&value.to_string())
}
