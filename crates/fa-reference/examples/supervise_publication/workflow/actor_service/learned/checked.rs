//! Canonical final-publication selection around the original learned recipe.
//! Admission reads only the independently selected profile. Actual witness and
//! producer files are opened per admitted request, before its original review.
use super::{fields::{Assets, Fields, MAX_DATA_BYTES}, identity, recipe::joint, Config, debug};
use crate::publication::{PublicationProfile, PROFILE_BYTES};
use fa_reference::strict_json::{self, Json, Limits};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "fa.learned-checked-publication/1";
const MAX_RECIPE_BYTES: usize = 64 * 1024;

type Decoded = (Fields, Option<joint::HeldOutJointPolicy>, Option<identity::profile::Profile>, Option<PathBuf>);

pub(super) fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let json = strict_json::parse(bytes, Limits {
        max_bytes: MAX_RECIPE_BYTES, max_depth: 13,
        max_items: 262_144, max_string_bytes: MAX_DATA_BYTES,
    }).map_err(debug)?;
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        let (recipe, joint, identity) = identity::profile::decode(bytes)?;
        return Ok((recipe, joint, identity, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let publication = wrapper.path("publication")?;
    let recipe = wrapper.take("recipe")?;
    wrapper.end()?;
    let (recipe, joint, identity) = identity::profile::unwrap(recipe)?;
    Ok((recipe, joint, identity, Some(publication)))
}

pub(super) fn load(path: &Path, config: &Config, assets: &mut Assets)
    -> Result<PublicationProfile, String>
{
    let selected = PublicationProfile::decode(&assets.read(path, PROFILE_BYTES)?)?;
    // Reject any unsupported stronger selection before model loading, store
    // access, sockets or clocks. No joint or waiting requirement is discarded.
    selected.learned_profile(&config.profile)?;
    Ok(selected)
}

/// The private runtime selection must agree with the independently retained
/// native recipe even when a caller constructs a resident entry in memory.
/// This check performs no source, clock, socket or journal operation.
pub(super) fn check_selection(config: &Config, loaded: &super::Loaded) -> Result<(), String> {
    let selected = loaded.publication.as_ref().map(|profile| profile.learned_profile(&config.profile)).transpose()?;
    if loaded.generation.required_publication_validation() != selected {
        return Err("learned publication profile differs from the exact numerical recipe".into());
    }
    Ok(())
}
