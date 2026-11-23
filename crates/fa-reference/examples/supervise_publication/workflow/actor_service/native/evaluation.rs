//! Explicit independent evaluation on the original generated publication owner.
//! Creation selects the whole accounting contract before any source or token work.
use super::{
    Config, FileHumanReviewer, FileOversight, Inputs, PeerProfile, Profile,
    PublicationProfile, anchors, debug, exact_precheck, operator,
};
use super::super::{andon_settings, evaluator};
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::FileIndependentEvaluator,
    stream::generated::checked::source::{
        GeneratedSourceProfile, evaluation::FileGeneratedEvaluationConfig,
    },
};
use std::{collections::BTreeSet, path::{Path, PathBuf}};

const USAGE: &str = "native evaluation requires serve-create or serve-create-checked with --native-text RECIPE, --andon ANDON_PROFILE, --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; optional precheck and paired operator/generation retain their original contracts";

pub(super) struct Options {
    accounting: PathBuf,
    peer: PathBuf,
}

/// Own the entire opted-in grammar before other parsers can hide a conflicting
/// mode. Unsupported recovery and resident paths cannot discard evaluation.
pub(super) fn take_options(args: &[String]) -> Result<(Vec<String>, Option<Options>), String> {
    if !args.iter().any(|arg| matches!(arg.as_str(), "--andon" | "--evaluator-profile")) {
        return Ok((args.to_vec(), None));
    }
    let positional = match args.first().map(String::as_str) {
        Some("serve-create") => 4,
        Some("serve-create-checked") => 5,
        _ => return Err(USAGE.into()),
    };
    if args.len() < positional + 8
        || (args.len() - positional) % 2 != 0
        || args[1..positional].iter().any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut accounting = None;
    let mut peer = None;
    let mut seen = BTreeSet::new();
    let mut remaining = args[..positional].to_vec();
    for pair in args[positional..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--andon" => accounting = Some(PathBuf::from(&pair[1])),
            "--evaluator-profile" => peer = Some(PathBuf::from(&pair[1])),
            "--native-text" | "--retain-history-anchor" | "--precheck-profile"
            | "--operator-profile" | "--tower-generation" => remaining.extend_from_slice(pair),
            _ => return Err(USAGE.into()),
        }
    }
    if !seen.contains("--native-text")
        || !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
    {
        return Err(USAGE.into());
    }
    Ok((remaining, Some(Options {
        accounting: accounting.ok_or(USAGE)?,
        peer: peer.ok_or(USAGE)?,
    })))
}

pub(super) struct Selection {
    settings: andon_settings::Settings,
    pub(super) peer: evaluator::Settings,
    inputs: [PathBuf; 2],
}

impl Options {
    pub(super) fn read(self) -> Result<Selection, String> {
        let settings = andon_settings::Settings::read(&self.accounting)?;
        settings.creation()?;
        let peer = evaluator::Settings::read(&self.peer)?;
        Ok(Selection { settings, peer, inputs: [self.accounting, self.peer] })
    }
}

impl Selection {
    /// All audiences, credentials, paths and declared modes are checked before
    /// a clock, exclusive listener, store creation or native numerical operation.
    pub(super) fn check(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        services: (Option<&operator::Selection>, Option<&exact_precheck::Selection>, Option<&anchors::Selection>),
        deployment: (bool, Option<&PublicationProfile>, Option<&Path>),
    ) -> Result<(), String> {
        let (open, publication, credibility) = deployment;
        if open || credibility.is_some() || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN {
            return Err(USAGE.into());
        }
        self.settings.creation()?;
        // Stronger witness/held-out publication profiles need their separately
        // composed workflow. A request for one is never reduced to simple mode.
        publication.map(PublicationProfile::simple_limits).transpose()?;
        let (operator, precheck, history) = services;
        let history = history.ok_or(USAGE)?;
        if history.output().is_none() || history.input().is_some() {
            return Err(USAGE.into());
        }
        if operator.is_some_and(operator::Selection::is_governed) {
            return Err(USAGE.into());
        }
        self.peer.check_host(
            config, actor, reviewer, operator.map(|selected| &selected.settings),
            &[actor.request], self.settings.config.identity(),
        )?;
        let mut custody = vec![config.source.path(), self.inputs[0].as_path(), self.inputs[1].as_path()];
        if let Some(precheck) = precheck { custody.push(precheck.settings.socket_path()); }
        if let Some(publication) = publication {
            custody.extend(publication.input_paths().iter().map(PathBuf::as_path));
        }
        custody.push(history.output().expect("required above"));
        self.peer.check_custody_paths(&custody)?;
        let mut paths = self.inputs.to_vec();
        paths.push(self.peer.socket_path().to_path_buf());
        history.check_files(&paths)
    }

    pub(super) fn check_inputs(&self, paths: &[&Path]) -> Result<(), String> {
        self.peer.check_custody_paths(paths)
    }

    pub(super) fn create(
        &self,
        config: &Config,
        inputs: &Inputs,
        selected: GeneratedSourceProfile,
    ) -> Result<(FileOversight, FileHumanReviewer, FileIndependentEvaluator), String> {
        self.settings.creation()?;
        let mut evaluation = FileGeneratedEvaluationConfig::new(self.settings.protocol.clone());
        if let Some(joint) = self.settings.joint {
            evaluation = evaluation.with_joint(joint).map_err(debug)?;
        }
        evaluation = evaluation.with_andon(self.settings.config.clone()).map_err(debug)?;
        let (host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
            &config.store, config.profile.clone(), inputs.decoder.clone(),
            inputs.tokenizer.clone(), selected, evaluation,
        ).map_err(debug)?;
        Ok((host, roles.oversight.human, roles.evaluator))
    }
}

