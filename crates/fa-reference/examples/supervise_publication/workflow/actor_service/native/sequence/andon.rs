//! Explicit creation or accounted recovery for one original native generated stream.
//! Configuration and evaluator custody are selected before numerical assets,
//! listeners or the first image; none is inferred from a later journal.
use super::super::{Config, PeerProfile, Profile, PublicationProfile, exact_precheck, operator};
use super::{Mode, anchors, evaluation};
use crate::workflow::actor_service::{andon_settings, evaluator};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub(super) const INVALID: &str = "native live evaluation requires serve-native-generated with paired --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; explicit --continue-generation or --after-message suffix recovery additionally requires --history-anchor INPUT and --evaluation-recovery REQUIREMENTS; governance, topology, joint promotion and activation require their own composition";

pub(super) struct Paths {
    pub settings: PathBuf,
    pub evaluator: PathBuf,
    pub recovery: Option<PathBuf>,
    pub mode: Mode,
}

#[derive(Clone, Copy)]
pub(super) struct Selection<'a> {
    pub settings: &'a andon_settings::Settings,
    pub evaluator: &'a evaluator::Settings,
    pub recovery: Option<&'a evaluation::Recovery>,
    pub paths: Option<&'a Paths>,
}

/// Own the complete selected grammar before another parser removes a mode.
/// The unselected grammar is unchanged, including its exact refusal behavior.
pub(super) fn take_options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<Paths>), String> {
    if !args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--andon" | "--evaluator-profile" | "--evaluation-recovery"
        )
    }) {
        return Ok((args.to_vec(), None));
    }
    if credibility.is_some()
        || args.len() < 10
        || args.len() > 22
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4].iter().any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(INVALID.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut settings = None;
    let mut evaluator = None;
    let mut recovery = None;
    let mut mode = None;
    let mut seen = BTreeSet::new();
    let mut index = 4;
    while index < args.len() {
        let flag = args[index].as_str();
        if !seen.insert(flag) {
            return Err(INVALID.into());
        }
        if flag == "--continue-generation" {
            if mode.replace(Mode::ContinueGeneration).is_some() {
                return Err(INVALID.into());
            }
            index += 1;
            continue;
        }
        let value = args.get(index + 1).ok_or(INVALID)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(INVALID.into());
        }
        let pair = &args[index..index + 2];
        match flag {
            "--andon" => settings = Some(PathBuf::from(value)),
            "--evaluator-profile" => evaluator = Some(PathBuf::from(value)),
            "--evaluation-recovery" => recovery = Some(PathBuf::from(value)),
            "--after-message" => {
                if value.len() > 20
                    || value.starts_with('0')
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(INVALID.into());
                }
                let after = value.parse::<u64>().map_err(|_| INVALID)?;
                if mode.replace(Mode::AfterMessage(after)).is_some() {
                    return Err(INVALID.into());
                }
                remaining.extend_from_slice(pair);
            }
            "--retain-history-anchor"
            | "--history-anchor"
            | "--publication-profile"
            | "--operator-profile"
            | "--tower-generation" => remaining.extend_from_slice(pair),
            _ => return Err(INVALID.into()),
        }
        index += 2;
    }
    let mode = mode.unwrap_or(Mode::Create);
    if !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
        || mode.is_recovery() != seen.contains("--history-anchor")
        || mode.is_recovery() != recovery.is_some()
    {
        return Err(INVALID.into());
    }
    Ok((
        remaining,
        Some(Paths {
            settings: settings.ok_or(INVALID)?,
            evaluator: evaluator.ok_or(INVALID)?,
            recovery,
            mode,
        }),
    ))
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

impl Paths {
    fn files(&self) -> Vec<&Path> {
        let mut files = vec![self.settings.as_path(), self.evaluator.as_path()];
        files.extend(self.recovery.as_deref());
        files
    }
}

/// All future listeners reserve their complete path, including directories
/// required by delayed inputs. Reading an offline witness later cannot repair a
/// namespace already consumed by a socket earlier in the resident schedule.
pub(super) fn check_endpoint_files(endpoints: &[PathBuf], files: &[&Path]) -> Result<(), String> {
    for file in files {
        let canonical = operator::resolve(file)?;
        if endpoints
            .iter()
            .any(|endpoint| overlaps(endpoint, &canonical))
        {
            return Err("native evaluation future endpoints overlap a selected input".into());
        }
    }
    Ok(())
}

impl Selection<'_> {
    pub(super) fn check_mode(self, mode: Mode) -> Result<(), String> {
        if mode.after() == Some(0)
            || mode.is_recovery() != self.recovery.is_some()
            || self.paths.is_some_and(|paths| paths.mode != mode)
        {
            return Err(INVALID.into());
        }
        Ok(())
    }

    pub(super) fn check_configuration(
        self,
        config: &Config,
        publication: Option<&PublicationProfile>,
    ) -> Result<(), String> {
        if self.recovery.is_none() {
            self.settings.creation()?;
        }
        if self.settings.joint.is_some()
            || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
        {
            return Err(INVALID.into());
        }
        if let Some(publication) = publication {
            publication.simple_limits()?;
        }
        if let Some(recovery) = self.recovery {
            recovery.check_configuration(config)?;
        }
        if let Some(paths) = self.paths {
            let store = operator::resolve(&config.store)?;
            let source = operator::resolve(config.source.path())?;
            let mut admitted: Vec<PathBuf> = Vec::new();
            for path in paths.files() {
                let canonical = operator::resolve(path)?;
                if overlaps(&canonical, &store)
                    || overlaps(&canonical, &source)
                    || admitted.iter().any(|other| overlaps(&canonical, other))
                {
                    return Err("native evaluation contracts require independent distinct custody outside the source and authority store".into());
                }
                admitted.push(canonical);
            }
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
        if self.recovery.is_none() {
            self.settings.creation()?;
        }
        if self.settings.joint.is_some()
            || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
            || selected_operator.is_some_and(operator::Selection::is_governed)
        {
            return Err(INVALID.into());
        }
        if let Some(recovery) = self.recovery {
            recovery.check_configuration(config)?;
            recovery.check_actors(actors, peers, selected_operator, self.evaluator)?;
        }
        let endpoints = self.endpoints(actors, peers, selected_operator)?;
        check_endpoint_files(&endpoints, &[config.source.path()])?;
        if let Some(paths) = self.paths {
            check_endpoint_files(&endpoints, &paths.files())?;
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
        if history.input().is_some() != self.recovery.is_some() || history.output_path().is_none() {
            return Err(INVALID.into());
        }
        history.check_endpoint(self.evaluator.socket_path())?;
        if let Some(recovery) = self.recovery {
            recovery.check_history(history)?;
        }
        if let Some(paths) = self.paths {
            history.check_files(&paths.files())?;
        }
        self.evaluator
            .check_custody_paths(&[history.output_path().expect("required output")])
    }

    pub(super) fn check_files(self, paths: &[&Path]) -> Result<(), String> {
        if let Some(recovery) = self.recovery {
            recovery.check_files(paths)?;
        }
        let mut reserved = vec![operator::resolve(self.evaluator.socket_path())?];
        if let Some(contracts) = self.paths {
            for path in contracts.files() {
                reserved.push(operator::resolve(path)?);
            }
        }
        check_endpoint_files(&reserved, paths)?;
        self.evaluator.check_custody_paths(paths)
    }

    pub(super) fn endpoints(
        self,
        actors: &[&Profile],
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
    ) -> Result<Vec<PathBuf>, String> {
        let mut selected = vec![self.evaluator.socket_path().to_path_buf()];
        for actor in actors {
            selected.extend([
                actor.socket.clone(),
                reviewer.socket(actor.request),
                crate::workflow::control::socket_path(reviewer, actor.request),
            ]);
        }
        if let Some(operator) = selected_operator {
            selected.push(operator.settings.socket.clone());
        }
        let mut endpoints: Vec<PathBuf> = Vec::new();
        for selected in selected {
            let canonical = operator::resolve(&selected)?;
            if endpoints.iter().any(|other| overlaps(&canonical, other)) {
                return Err(
                    "native evaluation requires disjoint future service endpoint paths".into(),
                );
            }
            endpoints.push(canonical);
        }
        Ok(endpoints)
    }

    /// Extend the same finite service namespace with every selected actor
    /// disclosure. The actor credentials already checked against the evaluator
    /// also own these sidecars; a disclosure never receives evaluator custody.
    /// Validate future sockets against the complete accounting and recovery
    /// inputs even if a selected input has since been removed from disk.
    pub(super) fn endpoints_with_prechecks<'a>(
        self,
        config: &Config,
        actors: &[&Profile],
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
        prechecks: impl IntoIterator<Item = &'a exact_precheck::Selection>,
    ) -> Result<Vec<PathBuf>, String> {
        let mut endpoints = self.endpoints(actors, reviewer, selected_operator)?;
        let store = operator::resolve(&config.store)?;
        for selected in prechecks {
            let socket = operator::resolve(selected.settings.socket_path())?;
            if overlaps(&socket, &store)
                || endpoints.iter().any(|other| overlaps(&socket, other))
            {
                return Err("native evaluation precheck endpoints must be disjoint from the authority store and every scheduled service endpoint".into());
            }
            self.check_files(&[socket.as_path()])?;
            endpoints.push(socket);
        }
        check_endpoint_files(&endpoints, &[config.source.path()])?;
        Ok(endpoints)
    }
}
