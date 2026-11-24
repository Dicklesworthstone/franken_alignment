//! Explicit independent evaluation on the original generated publication owner.
//! Creation and anchored recovery select the whole accounting contract before
//! source or token work. Recovery never restores old evaluator or human brands.
#[path = "evaluation/recovery.rs"]
pub(super) mod recovery;
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

const USAGE: &str = "native evaluation requires serve-create, serve-open or their -checked forms with --native-text RECIPE, --andon ANDON_PROFILE, --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; open additionally requires --history-anchor INPUT and --evaluation-recovery REQUIREMENTS and may select --after-message PREVIOUS_REQUEST or --finish-after-message PREVIOUS_REQUEST; optional precheck and paired operator/generation retain their original contracts";

pub(super) struct Options {
    accounting: PathBuf,
    peer: PathBuf,
    recovery: Option<PathBuf>,
}

/// Own the entire opted-in grammar before other parsers can hide a conflicting
/// mode. Recovery and resident paths cannot silently discard evaluation.
pub(super) fn take_options(args: &[String]) -> Result<(Vec<String>, Option<Options>), String> {
    if !args.iter().any(|arg| matches!(
        arg.as_str(), "--andon" | "--evaluator-profile" | "--evaluation-recovery"
    )) {
        return Ok((args.to_vec(), None));
    }
    let (positional, open) = match args.first().map(String::as_str) {
        Some("serve-create") => (4, false),
        Some("serve-open") => (4, true),
        Some("serve-create-checked") => (5, false),
        Some("serve-open-checked") => (5, true),
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
    let mut recovery = None;
    let mut seen = BTreeSet::new();
    let mut continuing = false;
    let mut remaining = args[..positional].to_vec();
    for pair in args[positional..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--andon" => accounting = Some(PathBuf::from(&pair[1])),
            "--evaluator-profile" => peer = Some(PathBuf::from(&pair[1])),
            "--evaluation-recovery" if open => recovery = Some(PathBuf::from(&pair[1])),
            "--history-anchor" if open => remaining.extend_from_slice(pair),
            "--after-message" | "--finish-after-message" if open => {
                if std::mem::replace(&mut continuing, true) {
                    return Err(USAGE.into());
                }
                let previous = pair[1].parse::<u64>().map_err(|_| USAGE)?;
                if previous == 0 || previous.to_string() != pair[1] {
                    return Err(USAGE.into());
                }
                remaining.extend_from_slice(pair);
            }
            "--native-text" | "--retain-history-anchor" | "--precheck-profile"
            | "--operator-profile" | "--tower-generation" => remaining.extend_from_slice(pair),
            _ => return Err(USAGE.into()),
        }
    }
    if !seen.contains("--native-text")
        || !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
        || open != seen.contains("--history-anchor")
        || open != recovery.is_some()
    {
        return Err(USAGE.into());
    }
    Ok((remaining, Some(Options {
        accounting: accounting.ok_or(USAGE)?,
        peer: peer.ok_or(USAGE)?,
        recovery,
    })))
}

pub(super) struct Selection {
    settings: andon_settings::Settings,
    pub(super) peer: evaluator::Settings,
    inputs: Vec<PathBuf>,
    recovery: Option<recovery::Requirements>,
}

impl Options {
    pub(super) fn read(
        self,
        peers: (&Config, &Profile, &PeerProfile),
        services: (Option<&operator::Selection>, Option<&exact_precheck::Selection>, Option<&anchors::Selection>),
    ) -> Result<Selection, String> {
        let peer = evaluator::Settings::read(&self.peer)?;
        let mut inputs = vec![self.accounting.clone(), self.peer];
        inputs.extend(self.recovery.iter().cloned());
        check_contract_paths(peers, services, &peer, &inputs)?;
        let settings = andon_settings::Settings::read(&self.accounting)?;
        if self.recovery.is_none() {
            settings.creation()?;
        }
        let recovery = self.recovery.as_deref().map(recovery::Requirements::read).transpose()?;
        Ok(Selection { settings, peer, inputs, recovery })
    }
}

/// Reserve all input names against the future service endpoints before reading
/// recovery requirements. Canonical parent resolution also covers absent sockets.
fn check_contract_paths(
    peers: (&Config, &Profile, &PeerProfile),
    services: (Option<&operator::Selection>, Option<&exact_precheck::Selection>, Option<&anchors::Selection>),
    evaluator: &evaluator::Settings,
    inputs: &[PathBuf],
) -> Result<(), String> {
    let (config, actor, reviewer) = peers;
    let (operator, precheck, history) = services;
    let store = operator::resolve(&config.store)?;
    let source = operator::resolve(config.source.path())?;
    let mut endpoints = vec![
        actor.socket.clone(), reviewer.socket(actor.request),
        crate::workflow::control::socket_path(reviewer, actor.request),
        evaluator.socket_path().to_path_buf(),
    ];
    if let Some(operator) = operator { endpoints.push(operator.settings.socket.clone()); }
    if let Some(precheck) = precheck { endpoints.push(precheck.settings.socket_path().to_path_buf()); }
    let endpoints = endpoints.iter().map(|path| operator::resolve(path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut admitted = BTreeSet::new();
    for input in inputs {
        let path = operator::resolve(input)?;
        if path.starts_with(&store) || path == source || endpoints.contains(&path)
            || !admitted.insert(path)
        {
            return Err("native evaluation inputs require distinct custody outside the authority store, evidence source and service endpoints".into());
        }
    }
    if let Some(history) = history { history.check_files(inputs)?; }
    Ok(())
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
        if open != self.recovery.is_some() || credibility.is_some()
            || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
        {
            return Err(USAGE.into());
        }
        if !open { self.settings.creation()?; }
        // Stronger witness/held-out publication profiles need their separately
        // composed workflow. A request for one is never reduced to simple mode.
        publication.map(PublicationProfile::simple_limits).transpose()?;
        let (operator, precheck, history) = services;
        let history = history.ok_or(USAGE)?;
        if history.output().is_none() || history.input().is_some() != open {
            return Err(USAGE.into());
        }
        if operator.is_some_and(operator::Selection::is_governed) {
            return Err(USAGE.into());
        }
        check_contract_paths((config, actor, reviewer), services, &self.peer, &self.inputs)?;
        self.peer.check_host(
            config, actor, reviewer, operator.map(|selected| &selected.settings),
            &[actor.request], self.settings.config.identity(),
        )?;
        let mut custody = vec![config.source.path()];
        custody.extend(self.inputs.iter().map(PathBuf::as_path));
        if let Some(precheck) = precheck { custody.push(precheck.settings.socket_path()); }
        if let Some(publication) = publication {
            custody.extend(publication.input_paths().iter().map(PathBuf::as_path));
            self.check_inputs(&publication.input_paths().iter().map(PathBuf::as_path).collect::<Vec<_>>())?;
        }
        custody.push(history.output().expect("required above"));
        self.peer.check_custody_paths(&custody)?;
        let mut paths = self.inputs.clone();
        paths.push(self.peer.socket_path().to_path_buf());
        history.check_files(&paths)
    }

    pub(super) fn check_inputs(&self, paths: &[&Path]) -> Result<(), String> {
        self.peer.check_custody_paths(paths)?;
        let contracts = self.inputs.iter().map(|path| operator::resolve(path))
            .collect::<Result<Vec<_>, _>>()?;
        for path in paths {
            if contracts.contains(&operator::resolve(path)?) {
                return Err("native evaluation contracts must not alias workflow or numerical inputs".into());
            }
        }
        Ok(())
    }

    pub(super) fn create(
        &self,
        config: &Config,
        inputs: &Inputs,
        selected: GeneratedSourceProfile,
    ) -> Result<(FileOversight, FileHumanReviewer, FileIndependentEvaluator), String> {
        if self.recovery.is_some() { return Err(USAGE.into()); }
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
