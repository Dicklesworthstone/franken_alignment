//! Explicit live governor custody for one newly created learned sequence.
//! Selection never reconstructs a role, enables a late guard or approves a
//! campaign. The original native constructor and independent peer own those.
use super::super::{fields, operator};
use super::{Config, Entry, LearnedTextRelease, PeerProfile};
pub(super) use crate::workflow::actor_service::governor::Settings;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileCampaignRequirement;
use std::{collections::BTreeSet, path::PathBuf};

const INVALID: &str = "learned governor requires a new nonpredictive, nonmediated resident sequence, --campaign-profile and paired --operator-profile/--tower-generation";

/// Own the whole selected grammar before another option stripper can erase a
/// recovery or maintenance mode. Without this selection, preserve the old argv.
pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    if !args.iter().any(|arg| arg == "--governor-profile") {
        return Ok((args.to_vec(), None));
    }
    if args.len() < 12
        || args.first().map(String::as_str) != Some("serve-learned-generated")
        || args[1..4]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(INVALID.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut selected = None;
    let mut seen = BTreeSet::new();
    let mut at = 4;
    while at < args.len() {
        let flag = args[at].as_str();
        if !matches!(
            flag,
            "--governor-profile"
                | "--operator-profile"
                | "--tower-generation"
                | "--campaign-profile"
                | "--retain-history-anchor"
                | "--checkpoint-before-generation"
        ) || !seen.insert(flag)
        {
            return Err(INVALID.into());
        }
        let value = args.get(at + 1).ok_or(INVALID)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(INVALID.into());
        }
        if flag == "--governor-profile" {
            selected = Some(fields::path(value)?);
        } else {
            remaining.extend_from_slice(&args[at..at + 2]);
        }
        at += 2;
    }
    if ![
        "--operator-profile",
        "--tower-generation",
        "--campaign-profile",
    ]
    .iter()
    .all(|flag| seen.contains(flag))
    {
        return Err(INVALID.into());
    }
    Ok((remaining, selected))
}

/// Read-only profile equality before any model/helper asset is ingested.
pub(super) fn check_selection(
    governor: Option<&Settings>,
    selected_operator: Option<&operator::Selection>,
    campaigns: Option<FileCampaignRequirement>,
) -> Result<(), String> {
    if governor.is_some()
        != selected_operator.is_some_and(|selected| selected.settings.policy_promotion.is_some())
    {
        return Err("learned policy promotion requires both an independent governor and an explicit numerical operator allowance".into());
    }
    if let Some(governor) = governor {
        if campaigns != Some(governor.campaign_requirement()) {
            return Err(
                "learned governor and original creation campaign limits must match exactly".into(),
            );
        }
    }
    Ok(())
}

/// Every selected identity and destination is admitted against the complete
/// sequence. Later actors cannot inherit, alias or replace the governor role.
pub(super) fn check(
    settings: &Settings,
    selected_operator: &operator::Selection,
    config: &Config,
    entries: &[Entry],
    peers: &PeerProfile,
    continuing: bool,
) -> Result<(), String> {
    if continuing
        || entries
            .iter()
            .filter(|entry| entry.release == LearnedTextRelease::Message)
            .count()
            < 2
    {
        return Err(INVALID.into());
    }
    let expected = settings.campaign_requirement();
    if entries[0].loaded.creation_campaigns != Some(expected) {
        return Err("learned governor requires its original first-image campaign guard".into());
    }
    let requests = entries
        .iter()
        .map(|entry| entry.loaded.request)
        .collect::<Vec<_>>();
    let socket = fields::resolve_root(&settings.socket)?;
    if socket == fields::resolve_root(config.source.path())? {
        return Err("learned governor socket conflicts with the original policy source".into());
    }
    for entry in entries {
        let loaded = &entry.loaded;
        if loaded.topology.is_some()
            || loaded.recovery_anchor.is_some()
            || loaded.generation.required_pre_output_forecast().is_some()
            || loaded
                .generation
                .required_owned_pre_output_forecast()
                .is_some()
            || loaded.generation.required_held_out_joint().is_some()
        {
            return Err(INVALID.into());
        }
        if loaded.expected_campaigns != Some(expected) {
            return Err(
                "every learned entry must retain the exact original campaign limits".into(),
            );
        }
        settings.check_learned_host(
            config,
            &entry.actor,
            peers,
            &selected_operator.settings,
            &requests,
        )?;
        if let Some(path) = loaded.anchor_output.as_deref() {
            if fields::resolve_root(path)? == socket {
                return Err(
                    "learned governor socket conflicts with retained history custody".into(),
                );
            }
        }
        if let Some(precheck) = entry.precheck.as_ref() {
            if fields::resolve_root(precheck.socket_path())? == socket {
                return Err(
                    "learned governor socket conflicts with selected disclosure custody".into(),
                );
            }
        }
    }
    Ok(())
}
