//! Explicit operator composition; no actor verb, inferred label or new gate.
use super::{Fields, Json, Limits, PROFILE_BYTES, PublicationProfile, debug, strict_json};
pub(super) use fa_reference::action::consequence::delivery::persistent::observed::credibility::held_out_joint::{
    HeldOutJointBudget, HeldOutJointPolicy,
};
use fa_reference::action::consequence::delivery::persistent::observed::credibility::held_out_joint::publication::{JointPublicationProfile, JointPublicationFeed};

const SCHEMA: &str = "fa.supervised-joint-publication/1";

/// One envelope around one existing witness/whole-input profile. The same total
/// byte/item/string bounds apply; the only extra depth is this outer object.
/// Legacy documents are re-parsed under their ORIGINAL depth limit, so the new
/// schema does not expand their accepted syntax. No source file is read here.
pub(super) fn decode(bytes: &[u8]) -> Result<(Json, Option<HeldOutJointPolicy>), String> {
    let limits = Limits { max_bytes: PROFILE_BYTES, max_depth: 5, max_items: 2048, max_string_bytes: 4096 };
    let json = strict_json::parse(bytes, Limits { max_depth: 6, ..limits }).map_err(debug)?;
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        return Ok((strict_json::parse(bytes, limits).map_err(debug)?, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let mut fields = Fields::new(wrapper.take("joint")?)?;
    let policy = HeldOutJointPolicy::new(
        fields.number("id")?, fields.number("generation")?,
        fields.number("minimum_safe_roots")?, fields.number("minimum_violation_roots")?,
        fields.number("maximum_escape_ppm")?, fields.number("maximum_false_stop_ppm")?,
        HeldOutJointBudget {
            cases: usize::try_from(fields.number("max_cases")?).map_err(debug)?,
            member_outcomes: usize::try_from(fields.number("max_member_outcomes")?).map_err(debug)?,
        },
    ).map_err(debug)?;
    fields.end()?;
    let publication = wrapper.take("publication")?;
    wrapper.end()?;
    if publication.get("schema").and_then(Json::as_str) == Some(SCHEMA) {
        return Err("joint publication wrappers cannot be nested".into());
    }
    // The unchanged profile grammar below still checks every original field.
    Ok((publication, Some(policy)))
}

impl PublicationProfile {
    pub(super) fn joint_policy(&self) -> Option<JointPublicationProfile> {
        self.joint.map(|joint| JointPublicationProfile {
            joint, validation: Some(self.limits),
            feed: self.feed.as_ref().map(|feed| JointPublicationFeed {
                changes: feed.changes, freshness: feed.freshness,
                snapshot_fallback: self.snapshot_fallback,
            }),
        })
    }
}
