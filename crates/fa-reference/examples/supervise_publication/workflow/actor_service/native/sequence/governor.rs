//! Explicit governor custody for one source-bound native sequence. Recovery
//! requires its separately selected complete guard/policy contract and anchor;
//! neither an archive nor an operator profile supplies missing expectations.
#[path = "governor/recovery.rs"]
pub(super) mod recovery;
#[path = "governor/requirements.rs"]
pub(super) mod requirements;
use super::super::{Config, PeerProfile, Profile, PublicationProfile, exact_precheck, operator};
use super::{Inputs, Mode, anchors};
pub(super) use crate::workflow::actor_service::governor::Settings;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileRecoveryRequirements;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const INVALID: &str = "native governance requires new serve-native-generated creation with paired --operator-profile/--tower-generation and independent --governor-profile; recovery and credibility activation need their own governed recovery contract";

/// Validate the WHOLE selected grammar before another parser can consume a
/// conflicting recovery mode. The original unselected grammar is unchanged.
pub(super) fn take_option(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<PathBuf>), String> {
    if !args.iter().any(|arg| arg == "--governor-profile") {
        return Ok((args.to_vec(), None));
    }
    if credibility.is_some()
        || args.len() < 10
        || args.len() > 14
        || args.len() % 2 != 0
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(INVALID.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut selected = None;
    let mut seen = BTreeSet::new();
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(INVALID.into());
        }
        match pair[0].as_str() {
            "--governor-profile" => selected = Some(PathBuf::from(&pair[1])),
            "--operator-profile"
            | "--tower-generation"
            | "--publication-profile"
            | "--retain-history-anchor" => remaining.extend_from_slice(pair),
            _ => return Err(INVALID.into()),
        }
    }
    if !seen.contains("--operator-profile") || !seen.contains("--tower-generation") {
        return Err(INVALID.into());
    }
    Ok((remaining, selected))
}

/// Governed recovery is an independently selected grammar. The creation parser
/// remains unchanged, and no missing recovery expectation can fall back to it.
pub(super) fn take_recovery_options(
    args: &[String],
    credibility: Option<&Path>,
    continuing: bool,
) -> Result<(Vec<String>, Option<PathBuf>, Option<PathBuf>), String> {
    if !args.iter().any(|arg| arg == "--recovery-requirements") {
        if continuing && args.iter().any(|arg| arg == "--governor-profile") {
            return Err("native governed continuation requires complete independent recovery requirements".into());
        }
        let (remaining, governor) = take_option(args, credibility)?;
        return Ok((remaining, governor, None));
    }
    const RECOVERY: &str = "native governed recovery requires --governor-profile, paired --operator-profile/--tower-generation, --recovery-requirements, --history-anchor and exactly one of --after-message or --continue-generation; continuation also requires a new --retain-history-anchor";
    let minimum = if continuing { 14 } else { 16 };
    let maximum = if continuing { 18 } else { 20 };
    if args.len() < minimum
        || args.len() > maximum
        || (continuing && credibility.is_some())
        || args.len() % 2 != 0
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(RECOVERY.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut governor = None;
    let mut requirements = None;
    let mut seen = BTreeSet::new();
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(RECOVERY.into());
        }
        match pair[0].as_str() {
            "--governor-profile" => governor = Some(PathBuf::from(&pair[1])),
            "--recovery-requirements" => requirements = Some(PathBuf::from(&pair[1])),
            "--operator-profile"
            | "--tower-generation"
            | "--after-message"
            | "--history-anchor"
            | "--publication-profile"
            | "--retain-history-anchor" => {
                remaining.extend_from_slice(pair);
            }
            _ => return Err(RECOVERY.into()),
        }
    }
    for required in [
        "--governor-profile",
        "--operator-profile",
        "--tower-generation",
        "--recovery-requirements",
        "--history-anchor",
    ] {
        if !seen.contains(required) {
            return Err(RECOVERY.into());
        }
    }
    if continuing {
        if seen.contains("--after-message") || !seen.contains("--retain-history-anchor") {
            return Err(RECOVERY.into());
        }
    } else if !seen.contains("--after-message") {
        return Err(RECOVERY.into());
    }
    Ok((remaining, governor, requirements))
}

/// Direct callers must select the same complete mode before opening listeners.
pub(super) fn check_selected(
    governor: Option<&Settings>,
    operator: Option<&operator::Selection>,
    mode: Mode,
    credibility: Option<&Path>,
    recovery: bool,
) -> Result<(), String> {
    if !recovery {
        if mode == Mode::ContinueGeneration && governor.is_some() {
            return Err("native governed continuation requires complete independent recovery requirements".into());
        }
        return check_selection(governor, operator, mode.after(), credibility);
    }
    if governor.is_none()
        || !operator.is_some_and(operator::Selection::is_governed)
        || !mode.is_recovery()
        || mode.after() == Some(0)
        || (mode == Mode::ContinueGeneration && credibility.is_some())
    {
        return Err("native governance recovery requires independent governor/operator custody and an explicit original recovery mode".into());
    }
    Ok(())
}

/// Expectations are selected data. Validate the service's entire supported
/// inventory, including absent roles, before time, sockets or numerical replay.
pub(super) fn check_recovery(
    governor: Option<&Settings>,
    expected: Option<&FileRecoveryRequirements>,
    history: Option<&anchors::Selection>,
    config: &Config,
    inputs: &Inputs,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let governor = governor.ok_or("native recovery requirements lack governor custody")?;
    if history.and_then(anchors::Selection::input).is_none() {
        return Err(
            "native governed recovery requires an independently selected history anchor".into(),
        );
    }
    if expected.guards.campaigns != Some(governor.campaign_requirement())
        || expected.guards.source != Some(config.source_policy)
        || expected.guards.stream != Some(inputs.stream)
        || expected.guards.decoder.as_ref() != Some(&inputs.decoder)
        || expected.guards.decoder_stop.is_some()
        || expected.guards.identity.is_some()
        || expected.guards.credential.is_some()
        || expected.credential_epoch.is_some()
    {
        return Err("native governed recovery requires the complete selected campaign, source, stream and decoder contracts and absent unsupported guards".into());
    }
    Ok(())
}

pub(super) fn check_selection(
    governor: Option<&Settings>,
    operator: Option<&operator::Selection>,
    after: Option<u64>,
    credibility: Option<&Path>,
) -> Result<(), String> {
    if governor.is_some() != operator.is_some_and(operator::Selection::is_governed) {
        return Err("native governance needs both independent governor and governed numerical operator custody".into());
    }
    if governor.is_some() && (after.is_some() || credibility.is_some()) {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn check_publication(
    governor: Option<&Settings>,
    publication: Option<&PublicationProfile>,
) -> Result<(), String> {
    if let (Some(governor), Some(publication)) = (governor, publication) {
        // The original atomic constructor installs the COMPLETE joint guard
        // while preserving the baseline committee. A guard is not held-out
        // qualification: fresh activation remains excluded by take_option and
        // check_selection, and recovery still requires its exact original guard.
        governor.check_custody_paths(
            &publication
                .input_paths()
                .iter()
                .map(PathBuf::as_path)
                .collect::<Vec<_>>(),
        )?;
    }
    Ok(())
}

/// All future actor identities and endpoints are admitted before recipe/model
/// loading. Different paths cannot substitute for independent credentials.
pub(super) fn check(
    governor: Option<&Settings>,
    operator: Option<&operator::Selection>,
    config: &Config,
    entries: &[(&Profile, Option<&exact_precheck::Selection>)],
    peers: &PeerProfile,
    after: Option<u64>,
    credibility: Option<&Path>,
    history: Option<&anchors::Selection>,
) -> Result<(), String> {
    check_with_recovery(
        governor,
        operator,
        config,
        entries,
        peers,
        Mode::from_after(after),
        credibility,
        history,
        false,
    )
}

pub(super) fn check_with_recovery(
    governor: Option<&Settings>,
    operator: Option<&operator::Selection>,
    config: &Config,
    entries: &[(&Profile, Option<&exact_precheck::Selection>)],
    peers: &PeerProfile,
    mode: Mode,
    credibility: Option<&Path>,
    history: Option<&anchors::Selection>,
    recovery: bool,
) -> Result<(), String> {
    check_selected(governor, operator, mode, credibility, recovery)?;
    let Some(governor) = governor else {
        return Ok(());
    };
    let operator = operator.ok_or(INVALID)?;
    let requests = entries
        .iter()
        .map(|(actor, _)| actor.request)
        .collect::<Vec<_>>();
    governor.check_custody_paths(&[config.source.path()])?;
    if let Some(credibility) = credibility {
        // Direct callers need the same separation as the command input loader.
        // A future governor listener must not acquire the capsule's pathname.
        governor.check_custody_paths(&[credibility])?;
    }
    for (actor, precheck) in entries {
        if recovery {
            governor.check_generated_recovery_host(
                config,
                actor,
                peers,
                &operator.settings,
                &requests,
            )?;
        } else {
            governor.check_generated_host(config, actor, peers, &operator.settings, &requests)?;
        }
        if let Some(precheck) = precheck {
            governor.check_custody_paths(&[precheck.settings.socket_path()])?;
        }
    }
    if let Some(history) = history {
        history.check_endpoint(&governor.socket)?;
    }
    Ok(())
}
