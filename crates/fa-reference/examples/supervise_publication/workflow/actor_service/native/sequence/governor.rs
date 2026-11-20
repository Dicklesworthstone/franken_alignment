//! Explicit governor custody for a NEW source-bound native sequence. Recovery
//! needs a separately selected complete guard/policy contract and is not
//! inferred from an archive or an operator profile.
use super::super::{Config, PeerProfile, Profile, PublicationProfile, exact_precheck, operator};
use super::anchors;
pub(super) use crate::workflow::actor_service::governor::Settings;
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
        if publication.requires_joint() {
            return Err(
                "native governance cannot activate a held-out joint profile during creation".into(),
            );
        }
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
    check_selection(governor, operator, after, credibility)?;
    let Some(governor) = governor else {
        return Ok(());
    };
    let operator = operator.ok_or(INVALID)?;
    let requests = entries
        .iter()
        .map(|(actor, _)| actor.request)
        .collect::<Vec<_>>();
    governor.check_custody_paths(&[config.source.path()])?;
    for (actor, precheck) in entries {
        governor.check_generated_host(config, actor, peers, &operator.settings, &requests)?;
        if let Some(precheck) = precheck {
            governor.check_custody_paths(&[precheck.settings.socket_path()])?;
        }
    }
    if let Some(history) = history {
        history.check_endpoint(&governor.socket)?;
    }
    Ok(())
}
