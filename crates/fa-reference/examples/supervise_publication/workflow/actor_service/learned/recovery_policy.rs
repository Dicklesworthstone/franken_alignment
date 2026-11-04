//! An independently retained full policy for original guarded learned recovery.
//! This expectation changes neither bootstrap configuration nor numerical
//! provenance, and supplies no policy update, campaign approval or live key.
use super::{MAX_RECIPE_BYTES, Policy, joint::HeldOutJointPolicy};
use super::super::{credential, fields::{Fields, MAX_DATA_BYTES}, identity};
use crate::config::{debug, policy_from_json};
use fa_reference::action::consequence::{
    delivery::persistent::observed::guarded::FileCampaignRequirement,
    oversight::policy_governance::MAX_POLICY_CAMPAIGNS,
    policy_campaign::{ReplayLimits, MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::path::PathBuf;

const SCHEMA: &str = "fa.learned-recovery-policy/1";
const CAMPAIGN_SCHEMA: &str = "fa.learned-recovery-policy/2";

pub(super) struct Expectation {
    pub policy: Policy,
    pub campaigns: Option<FileCampaignRequirement>,
}

type Decoded = (Fields, Option<HeldOutJointPolicy>, Option<identity::profile::Profile>,
    Option<PathBuf>, Option<PathBuf>, Option<Expectation>);

/// The single optional outer envelope is recovery_policy(credential(checked(
/// identity(joint(legacy))))). Each existing inner envelope remains optional;
/// repeated or reordered wrappers still fail the original closed grammar.
pub(super) fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let json = strict_json::parse(bytes, Limits {
        max_bytes: MAX_RECIPE_BYTES, max_depth: 15,
        max_items: 262_144, max_string_bytes: MAX_DATA_BYTES,
    }).map_err(debug)?;
    let schema = json.get("schema").and_then(Json::as_str);
    let with_campaigns = schema == Some(CAMPAIGN_SCHEMA);
    if schema != Some(SCHEMA) && !with_campaigns {
        // Legacy input retains every original parser and depth bound.
        let (recipe, joint, identity, publication, credential) = credential::decode(bytes)?;
        return Ok((recipe, joint, identity, publication, credential, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let policy = policy_from_json(wrapper.take("effective_policy")?)?;
    let campaigns = if with_campaigns {
        Some(campaign_requirement(wrapper.object("campaigns")?)?)
    } else { None };
    let recipe = wrapper.take("recipe")?;
    wrapper.end()?;
    let (recipe, joint, identity, publication, credential) = credential::unwrap(recipe)?;
    Ok((recipe, joint, identity, publication, credential,
        Some(Expectation { policy, campaigns })))
}

/// These are independently retained guard settings, not a campaign request or
/// approval. Bound them before any numerical asset or journal is opened; the
/// original native guard still checks exact equality after complete replay.
fn campaign_requirement(mut fields: Fields) -> Result<FileCampaignRequirement, String> {
    let limits = ReplayLimits {
        cases: fields.size("cases")?,
        input_bytes: fields.size("input_bytes")?,
    };
    let max_campaigns = fields.size("max_campaigns")?;
    fields.end()?;
    if limits.cases == 0 || limits.input_bytes == 0 || max_campaigns == 0 {
        return Err("learned recovery campaign limits must be nonzero".into());
    }
    if limits.cases > MAX_REPLAY_CASES || limits.input_bytes > MAX_REPLAY_INPUT_BYTES
        || max_campaigns > MAX_POLICY_CAMPAIGNS {
        return Err("learned recovery campaign limits exceed the native bounds".into());
    }
    Ok(FileCampaignRequirement { limits, max_campaigns })
}
