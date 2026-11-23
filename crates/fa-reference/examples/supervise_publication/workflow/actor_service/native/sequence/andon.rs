//! Explicit creation-only accounting for one original native generated stream.
//! Configuration and evaluator custody are selected before numerical assets,
//! listeners or the first image; none is inferred from a later journal.
use super::super::{Config, PeerProfile, Profile, PublicationProfile, operator};
use super::anchors;
use crate::workflow::actor_service::{andon_settings, evaluator};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub(super) const INVALID: &str = "native live evaluation requires new serve-native-generated creation with paired --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; recovery, governance, topology, joint promotion and precheck schedules require their own composition";

pub(super) struct Paths {
    pub settings: PathBuf,
    pub evaluator: PathBuf,
}

#[derive(Clone, Copy)]
pub(super) struct Selection<'a> {
    pub settings: &'a andon_settings::Settings,
    pub evaluator: &'a evaluator::Settings,
}

/// Own the complete selected grammar before another parser removes a mode.
/// The unselected grammar is unchanged, including its exact refusal behavior.
pub(super) fn take_options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<Paths>), String> {
    if !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--andon" | "--evaluator-profile"))
    {
        return Ok((args.to_vec(), None));
    }
    if credibility.is_some()
        || args.len() < 10
        || args.len() > 16
        || args.len() % 2 != 0
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(INVALID.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut settings = None;
    let mut evaluator = None;
    let mut seen = BTreeSet::new();
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(INVALID.into());
        }
        match pair[0].as_str() {
            "--andon" => settings = Some(PathBuf::from(&pair[1])),
            "--evaluator-profile" => evaluator = Some(PathBuf::from(&pair[1])),
            "--retain-history-anchor"
            | "--publication-profile"
            | "--operator-profile"
            | "--tower-generation" => remaining.extend_from_slice(pair),
            _ => return Err(INVALID.into()),
        }
    }
    if !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
    {
        return Err(INVALID.into());
    }
    Ok((
        remaining,
        Some(Paths {
            settings: settings.ok_or(INVALID)?,
            evaluator: evaluator.ok_or(INVALID)?,
        }),
    ))
}

impl Selection<'_> {
    pub(super) fn check_configuration(
        self,
        config: &Config,
        publication: Option<&PublicationProfile>,
    ) -> Result<(), String> {
        self.settings.creation()?;
        if self.settings.joint.is_some()
            || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
        {
            return Err(INVALID.into());
        }
        if let Some(publication) = publication {
            publication.simple_limits()?;
        }
        self.evaluator.check_audience(
            config.profile.delivery.scope,
            self.settings.config.identity(),
        )
    }

    /// Validate the complete future schedule before model loading or startup.
    /// Every actor credential and every future reviewer/Stop endpoint belongs
    /// to the original selection, even when its listener does not yet exist.
    pub(super) fn check_actors(
        self,
        config: &Config,
        actors: &[&Profile],
        peers: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
    ) -> Result<(), String> {
        self.settings.creation()?;
        if self.settings.joint.is_some()
            || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
            || selected_operator.is_some_and(operator::Selection::is_governed)
        {
            return Err(INVALID.into());
        }
        let requests: Vec<_> = actors.iter().map(|actor| actor.request).collect();
        for actor in actors {
            self.evaluator.check_host(
                config,
                actor,
                peers,
                selected_operator.map(|selected| &selected.settings),
                &requests,
                self.settings.config.identity(),
            )?;
        }
        self.evaluator.check_custody_paths(&[config.source.path()])
    }

    pub(super) fn check_history(self, history: &anchors::Selection) -> Result<(), String> {
        if history.input().is_some() || history.output_path().is_none() {
            return Err(INVALID.into());
        }
        history.check_endpoint(self.evaluator.socket_path())?;
        self.evaluator
            .check_custody_paths(&[history.output_path().expect("required output")])
    }

    pub(super) fn check_files(self, paths: &[&Path]) -> Result<(), String> {
        self.evaluator.check_custody_paths(paths)
    }
}
