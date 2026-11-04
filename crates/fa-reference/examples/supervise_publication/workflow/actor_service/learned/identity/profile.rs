//! An explicit computed-identity registration around one unchanged recipe.
//! Per-attempt measurement inputs are separate from the exact native contract.
use super::Selection;
use super::super::{fields::{Assets, Fields, MAX_DATA_BYTES}, recipe::joint, debug};
use fa_reference::action::consequence::activation::{
    identity::wire, tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS},
};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileIdentityRequirement;
use fa_reference::action::consequence::oversight::identity::{IdentityPolicy, MAX_IDENTITY_CHECKS};
use fa_reference::strict_json::{self, Json, Limits};
use std::path::PathBuf;

const SCHEMA: &str = "fa.learned-identity-publication/1";
const MAX_RECIPE_BYTES: usize = 64 * 1024;

pub(in super::super) struct Profile {
    passport: PathBuf,
    policy: IdentityPolicy,
    pub selection: Selection,
}
impl Profile {
    pub(in super::super) fn registration(&self, assets: &mut Assets)
        -> Result<FileIdentityRequirement, String>
    {
        let passport = wire::decode_passport(&assets.read(&self.passport, wire::MAX_PASSPORT_BYTES)?)
            .map_err(debug)?;
        Ok(FileIdentityRequirement { passport, policy: self.policy })
    }
}

/// Canonical composition is identity(joint(legacy)) or identity(legacy).
/// Legacy and joint-only recipes retain their existing original parse budgets.
pub(in super::super) fn decode(bytes: &[u8])
    -> Result<(Fields, Option<joint::HeldOutJointPolicy>, Option<Profile>), String>
{
    let json = strict_json::parse(bytes, Limits {
        max_bytes: MAX_RECIPE_BYTES, max_depth: 12,
        max_items: 262_144, max_string_bytes: MAX_DATA_BYTES,
    }).map_err(debug)?;
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        let (recipe, joint) = joint::decode(bytes)?;
        return Ok((recipe, joint, None));
    }
    unwrap(json)
}

/// The canonical checked-publication envelope has already accounted for its
/// additional depth. Inner identity/joint ordering remains exactly the same.
pub(in super::super) fn unwrap(json: Json)
    -> Result<(Fields, Option<joint::HeldOutJointPolicy>, Option<Profile>), String>
{
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        let (recipe, joint) = joint::unwrap(json)?;
        return Ok((recipe, joint, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let mut identity = wrapper.object("identity")?;
    let passport = identity.path("passport")?;
    let mut policy = identity.object("policy")?;
    let policy_value = IdentityPolicy {
        observer_id: policy.number("observer_id")?,
        timeout_ticks: policy.number("timeout_ticks")?,
        validity_ticks: policy.number("validity_ticks")?,
        max_checks: policy.size("max_checks")?,
    };
    policy.end()?;
    if policy_value.observer_id == 0 || policy_value.timeout_ticks == 0
        || policy_value.timeout_ticks > policy_value.validity_ticks
        || !(1..=MAX_IDENTITY_CHECKS).contains(&policy_value.max_checks) {
        return Err("computed learned identity requires a nonzero bounded native policy".into());
    }
    let check = identity.number("check")?;
    let measurement_sequence = identity.number("measurement_sequence")?;
    let scalar_products = identity.number("scalar_products")?;
    let observed_manifest = identity.path("observed_manifest")?;
    identity.end()?;
    if check == 0 || measurement_sequence == 0 || scalar_products == 0
        || scalar_products > MAX_DECODER_PRODUCTS {
        return Err("computed learned identity requires fresh nonzero IDs and finite native work".into());
    }
    let recipe = wrapper.take("recipe")?;
    wrapper.end()?;
    let (recipe, joint) = joint::unwrap(recipe)?;
    Ok((recipe, joint, Some(Profile { passport, policy: policy_value,
        selection: Selection { check, measurement_sequence,
            budget: DecoderBudget { scalar_products }, observed_manifest } })))
}
