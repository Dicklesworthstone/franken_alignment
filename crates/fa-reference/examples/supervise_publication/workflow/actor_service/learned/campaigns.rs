//! Explicit first-image campaign governance, separate from recovery expectations.
//! This profile installs the native guard; it grants no review or promotion key.
use super::{Config, Loaded, debug, fields};
use crate::config::read_regular;
use fa_reference::action::consequence::{
    delivery::persistent::observed::guarded::FileCampaignRequirement,
    oversight::policy_governance::MAX_POLICY_CAMPAIGNS,
    policy_campaign::{MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_PROFILE_BYTES: usize = 4096;

pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    let mut rest = Vec::new();
    let mut selected = None;
    let mut at = 0;
    while at < args.len() {
        if args[at] == "--campaign-profile" {
            if selected.is_some() {
                return Err("duplicate --campaign-profile".into());
            }
            let value = args
                .get(at + 1)
                .ok_or("--campaign-profile requires a path")?;
            selected = Some(fields::path(value)?);
            at += 2;
        } else {
            rest.push(args[at].clone());
            at += 1;
        }
    }
    Ok((rest, selected))
}

pub(super) fn decode(bytes: &[u8]) -> Result<FileCampaignRequirement, String> {
    let mut fields = fields::Fields::parse(bytes, MAX_PROFILE_BYTES)?;
    if fields.text("schema")? != "fa.policy-campaign-profile/1" {
        return Err("unsupported policy campaign profile".into());
    }
    let limits = ReplayLimits {
        cases: fields.size("cases")?,
        input_bytes: fields.size("input_bytes")?,
    };
    let max_campaigns = fields.size("max_campaigns")?;
    fields.end()?;
    if limits.cases == 0
        || limits.cases > MAX_REPLAY_CASES
        || limits.input_bytes == 0
        || limits.input_bytes > MAX_REPLAY_INPUT_BYTES
        || max_campaigns == 0
        || max_campaigns > MAX_POLICY_CAMPAIGNS
    {
        return Err("policy campaign profile exceeds the native nonzero limits".into());
    }
    Ok(FileCampaignRequirement {
        limits,
        max_campaigns,
    })
}

/// Freeze one independently selected guard before any owner or source is opened.
/// Recovery continues to use the exact recovery-policy envelope, not this flag.
pub(super) fn admit(
    config: &Config,
    loaded: &mut Loaded,
    path: Option<&Path>,
    recovering: bool,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if recovering {
        return Err("--campaign-profile is only for new learned histories".into());
    }
    admit_selected(loaded, read(config, path)?)
}

pub(super) fn read(config: &Config, path: &Path) -> Result<FileCampaignRequirement, String> {
    let canonical = fs::canonicalize(path).map_err(debug)?;
    if canonical.starts_with(fields::resolve_root(&config.store)?) {
        return Err(
            "campaign creation profile must be retained outside the authority store".into(),
        );
    }
    decode(&read_regular(path, MAX_PROFILE_BYTES)?)
}

pub(super) fn admit_selected(
    loaded: &mut Loaded,
    selected: FileCampaignRequirement,
) -> Result<(), String> {
    if loaded
        .expected_campaigns
        .is_some_and(|expected| expected != selected)
    {
        return Err(
            "campaign creation profile differs from the independent recovery inventory".into(),
        );
    }
    loaded.expected_campaigns = Some(selected);
    loaded.creation_campaigns = Some(selected);
    Ok(())
}
