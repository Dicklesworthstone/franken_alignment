//! Explicit joint held-out policy around one unchanged learned recipe.
use super::{Fields, MAX_DATA_BYTES, MAX_RECIPE_BYTES, debug};
use fa_reference::strict_json::{self, Json, Limits};
pub(in super::super) use fa_reference::action::consequence::delivery::persistent::observed::credibility::held_out_joint::HeldOutJointPolicy;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::held_out_joint::HeldOutJointBudget;

const SCHEMA: &str = "fa.learned-joint-publication/1";

/// The wrapper adds only one level of depth, within the original aggregate
/// byte/item/string bounds. Legacy documents retain their exact original depth
/// limit. No asset, capsule, journal or live input is read during this parse.
pub(in super::super) fn decode(bytes: &[u8]) -> Result<(Fields, Option<HeldOutJointPolicy>), String> {
    let json = strict_json::parse(bytes, Limits {
        max_bytes: MAX_RECIPE_BYTES, max_depth: 11,
        max_items: 262_144, max_string_bytes: MAX_DATA_BYTES,
    }).map_err(debug)?;
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        return Ok((Fields::parse(bytes, MAX_RECIPE_BYTES)?, None));
    }
    unwrap(json)
}

/// Used only by the canonical outer computed-identity envelope. Its bounded
/// parser already accounted for the one extra wrapper level.
pub(in super::super) fn unwrap(json: Json) -> Result<(Fields, Option<HeldOutJointPolicy>), String> {
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        return Ok((Fields::new(json)?, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let mut joint = wrapper.object("joint")?;
    let policy = HeldOutJointPolicy::new(
        joint.number("id")?, joint.number("generation")?,
        joint.number("minimum_safe_roots")?, joint.number("minimum_violation_roots")?,
        joint.number("maximum_escape_ppm")?, joint.number("maximum_false_stop_ppm")?,
        HeldOutJointBudget { cases: joint.size("max_cases")?,
            member_outcomes: joint.size("max_member_outcomes")? },
    ).map_err(debug)?;
    joint.end()?;
    let recipe = wrapper.take("recipe")?;
    wrapper.end()?;
    if recipe.get("schema").and_then(Json::as_str) == Some(SCHEMA) {
        return Err("joint learned recipe wrappers cannot be nested".into());
    }
    // The existing /1 through /6 grammar still checks every original field.
    Ok((Fields::new(recipe)?, Some(policy)))
}
