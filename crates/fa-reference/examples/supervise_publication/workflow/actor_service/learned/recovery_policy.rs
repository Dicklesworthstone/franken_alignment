//! An independently retained full policy for original guarded learned recovery.
//! This expectation changes neither bootstrap configuration nor numerical
//! provenance, and supplies no policy update, campaign approval or live key.
use super::{MAX_RECIPE_BYTES, Policy, joint::HeldOutJointPolicy};
use super::super::{credential, fields::{Fields, MAX_DATA_BYTES}, identity};
use crate::config::{debug, policy_from_json};
use fa_reference::strict_json::{self, Json, Limits};
use std::path::PathBuf;

const SCHEMA: &str = "fa.learned-recovery-policy/1";

type Decoded = (Fields, Option<HeldOutJointPolicy>, Option<identity::profile::Profile>,
    Option<PathBuf>, Option<PathBuf>, Option<Policy>);

/// The single optional outer envelope is recovery_policy(credential(checked(
/// identity(joint(legacy))))). Each existing inner envelope remains optional;
/// repeated or reordered wrappers still fail the original closed grammar.
pub(super) fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let json = strict_json::parse(bytes, Limits {
        max_bytes: MAX_RECIPE_BYTES, max_depth: 15,
        max_items: 262_144, max_string_bytes: MAX_DATA_BYTES,
    }).map_err(debug)?;
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        // Legacy input retains every original parser and depth bound.
        let (recipe, joint, identity, publication, credential) = credential::decode(bytes)?;
        return Ok((recipe, joint, identity, publication, credential, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let policy = policy_from_json(wrapper.take("effective_policy")?)?;
    let recipe = wrapper.take("recipe")?;
    wrapper.end()?;
    let (recipe, joint, identity, publication, credential) = credential::unwrap(recipe)?;
    Ok((recipe, joint, identity, publication, credential, Some(policy)))
}
