//! Independent post-review assessment of the same original learned owner.
//! Selection precedes work; the finite evaluator lifetime includes generation
//! and review. A late label never reruns either or supplies an effect key.
#[path = "evaluation/recovery.rs"]
mod recovery;
#[path = "evaluation/history.rs"]
pub(super) mod history;
use super::super::{Profile, andon_settings, evaluator};
use super::{
    Config, Control, Deadline, ElapsedTick, FileHumanReviewer, FileOversight, FileSupervisedDriver,
    Loaded, PeerProfile, RecoveryReserve, debug, fields, ingress, operator, precheck,
    recovery as learned_recovery,
};
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const USAGE: &str = "learned evaluation requires create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE with --actor-profile ACTOR_PROFILE, --andon ANDON_PROFILE, --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; explicit --continue-generation, --after-message PREVIOUS_REQUEST or --finish-after-message PREVIOUS_REQUEST additionally requires --history-anchor INPUT; --evaluate-only requires --history-anchor INPUT and permits only an optional historical --finish-after-message PREVIOUS_REQUEST; optional precheck and paired operator/generation retain their original live contracts";

const SEQUENCE_USAGE: &str = "resident learned evaluation requires serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE with --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; --continue-generation or --after-message PREVIOUS_REQUEST requires --history-anchor INPUT; operator/generation remains an optional pair";

pub(super) struct Options {
    accounting: PathBuf,
    peer: PathBuf,
    history_input: Option<PathBuf>,
    pub(super) evaluate_only: bool,
}

/// Own the complete opted-in grammar before another parser can discard a mode.
/// Existing invocations without either evaluation flag keep their exact argv.
pub(super) fn take_options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<Options>), String> {
    parse_options(args, credibility, false)
}

pub(super) fn take_sequence_options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<Options>), String> {
    parse_options(args, credibility, true)
}

fn parse_options(
    args: &[String],
    credibility: Option<&Path>,
    resident: bool,
) -> Result<(Vec<String>, Option<Options>), String> {
    let (command, usage) = if resident {
        ("serve-learned-generated", SEQUENCE_USAGE)
    } else {
        ("create-learned-generated", USAGE)
    };
    if !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--andon" | "--evaluator-profile" | "--evaluate-only"))
    {
        return Ok((args.to_vec(), None));
    }
    if credibility.is_some()
        || args.first().map(String::as_str) != Some(command)
        || args.len() < 4
        || args[1..4]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(usage.into());
    }
    let mut accounting = None;
    let mut peer = None;
    let mut history_input = None;
    let mut seen = BTreeSet::new();
    let mut remaining = args[..4].to_vec();
    let mut continuing = false;
    let mut evaluate_only = false;
    let mut at = 4;
    while at < args.len() {
        let flag = args[at].as_str();
        if !seen.insert(flag) {
            return Err(usage.into());
        }
        if flag == "--evaluate-only" && !resident {
            evaluate_only = true;
            at += 1;
            continue;
        }
        if matches!(flag, "--continue-generation" | "--after-message" | "--finish-after-message")
            && std::mem::replace(&mut continuing, true)
        {
            return Err(usage.into());
        }
        if flag == "--continue-generation" {
            remaining.push(args[at].clone());
            at += 1;
            continue;
        }
        let value = args.get(at + 1).ok_or(usage)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(usage.into());
        }
        match flag {
            "--andon" => accounting = Some(PathBuf::from(value)),
            "--evaluator-profile" => peer = Some(PathBuf::from(value)),
            "--history-anchor" => {
                history_input = Some(PathBuf::from(value));
                remaining.extend_from_slice(&args[at..at + 2]);
            }
            "--after-message" | "--finish-after-message" if !resident || flag == "--after-message" => {
                let predecessor = value.parse::<u64>().map_err(|_| usage)?;
                if predecessor == 0 || predecessor.to_string() != *value {
                    return Err(usage.into());
                }
                remaining.extend_from_slice(&args[at..at + 2]);
            }
            "--actor-profile" | "--precheck-profile" if !resident => remaining.extend_from_slice(&args[at..at + 2]),
            "--retain-history-anchor"
            | "--operator-profile"
            | "--tower-generation" => remaining.extend_from_slice(&args[at..at + 2]),
            _ => return Err(usage.into()),
        }
        at += 2;
    }
    if (!resident && !seen.contains("--actor-profile"))
        || !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
        || (continuing || evaluate_only) != history_input.is_some()
        || (evaluate_only
            && [
                "--continue-generation",
                "--after-message",
                "--precheck-profile",
                "--operator-profile",
                "--tower-generation",
            ]
            .iter()
            .any(|flag| seen.contains(flag)))
    {
        return Err(usage.into());
    }
    Ok((
        remaining,
        Some(Options {
            accounting: accounting.ok_or(usage)?,
            peer: peer.ok_or(usage)?,
            history_input,
            evaluate_only,
        }),
    ))
}

pub(super) struct Selection {
    settings: andon_settings::Settings,
    pub(super) peer: evaluator::Settings,
    inputs: [PathBuf; 2],
    pub(super) history_input: Option<PathBuf>,
}

impl Options {
    pub(super) fn read(self) -> Result<Selection, String> {
        let settings = andon_settings::Settings::read(&self.accounting)?;
        if self.history_input.is_none() {
            settings.creation()?;
        }
        let peer = evaluator::Settings::read(&self.peer)?;
        Ok(Selection {
            settings,
            peer,
            inputs: [self.accounting, self.peer],
            history_input: self.history_input,
        })
    }
}

// An archive or socket cannot occupy a directory needed by another file.
// Resolve existing ancestors first so this holds for offline witnesses too.
fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

impl Selection {
    /// Admit the entire finite resident schedule without acquiring an owner,
    /// reading a clock or issuing a new role for any later request.
    pub(super) fn check_sequence(
        &self,
        config: &Config,
        entries: &[super::sequence::Entry],
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
    ) -> Result<(), String> {
        let first = entries.first().ok_or(SEQUENCE_USAGE)?;
        let history = first.loaded.anchor_output.as_deref().ok_or(SEQUENCE_USAGE)?;
        self.check_sequence_inputs(history, &[config.source.path()])?;
        if self.history_input.is_some() != first.loaded.recovery_anchor.is_some() {
            return Err(SEQUENCE_USAGE.into());
        }
        if let Some(path) = self.history_input.as_deref() {
            self.check_sequence_inputs(history, &[path])?;
            self.recovery_requirements(config, &first.loaded)?;
        }
        let actors = entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>();
        self.check_sequence_actors(config, &actors, reviewer, selected_operator)?;
        let mut endpoints = vec![self.peer.socket_path().to_path_buf()];
        let mut inputs = vec![config.source.path().to_path_buf()];
        if let Some(selected) = selected_operator {
            endpoints.push(selected.settings.socket.clone());
        }
        inputs.extend(self.history_input.iter().cloned());
        for (index, entry) in entries.iter().enumerate() {
            let loaded = &entry.loaded;
            if (index != 0 && loaded.recovery_anchor.is_some())
                || loaded.topology.is_some()
                || loaded.checkpoint_on_generation.is_some()
                || loaded.creation_campaigns.is_some()
                || loaded.expected_campaigns.is_some()
                || loaded.generation.required_pre_output_forecast().is_some()
                || loaded.generation.required_owned_pre_output_forecast().is_some()
                || loaded.generation.required_held_out_joint().is_some()
            {
                return Err(SEQUENCE_USAGE.into());
            }
            if self.history_input.is_some() {
                super::credential::check_selection(loaded)?;
            } else {
                super::recipe::check_creation(config, loaded)?;
                super::credential::check_creation(loaded)?;
            }
            if let Some(publication) = &loaded.publication {
                publication.simple_limits()?;
                self.check_sequence_inputs(history, &publication.input_paths()
                    .iter().map(PathBuf::as_path).collect::<Vec<_>>())?;
                inputs.extend(publication.input_paths().iter().cloned());
            }
            if let Some(identity) = &loaded.identity {
                self.check_sequence_inputs(history, &[&identity.observed_manifest])?;
                inputs.push(identity.observed_manifest.clone());
            }
            if let Some(credential) = &loaded.credential {
                self.check_sequence_inputs(history, &credential.input_paths())?;
                inputs.extend(credential.input_paths().into_iter().map(Path::to_path_buf));
            }
            endpoints.extend([
                entry.actor.socket.clone(),
                reviewer.socket(loaded.request),
                crate::workflow::control::socket_path(reviewer, loaded.request),
            ]);
            if let Some(precheck) = &entry.precheck {
                endpoints.push(precheck.socket_path().to_path_buf());
                self.peer.check_custody_paths(&[precheck.socket_path()])?;
            }
        }
        self.check_history_endpoints(history, &endpoints)?;
        self.check_sequence_endpoint_inputs(&endpoints, &inputs)?;
        Ok(())
    }

    pub(super) fn check_sequence_actors(
        &self,
        config: &Config,
        actors: &[&Profile],
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
    ) -> Result<(), String> {
        if self.history_input.is_none() {
            self.settings.creation()?;
        }
        if config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
            || selected_operator.is_some_and(|selected| selected.settings.policy_promotion.is_some())
        {
            return Err(SEQUENCE_USAGE.into());
        }
        let requests = actors.iter().map(|actor| actor.request).collect::<Vec<_>>();
        for actor in actors {
            self.peer.check_host(config, actor, reviewer,
                selected_operator.map(|selected| &selected.settings), &requests,
                self.settings.config.identity())?;
        }
        Ok(())
    }

    /// Preserve selected input names even when a file has disappeared or will
    /// be read only by a later request. Existing-ancestor resolution catches
    /// canonical aliases before any listener or retained output can claim them.
    pub(super) fn check_sequence_inputs(&self, history: &Path, paths: &[&Path])
        -> Result<(), String>
    {
        let history = fields::resolve_root(history)?;
        let evaluator = fields::resolve_root(self.peer.socket_path())?;
        for path in paths.iter().copied().chain(self.inputs.iter().map(PathBuf::as_path)) {
            let input = fields::resolve_root(path)?;
            if overlaps(&history, &input) {
                return Err("learned evaluation history output conflicts with a selected input".into());
            }
            if overlaps(&evaluator, &input) {
                return Err("learned evaluator endpoint conflicts with a selected input".into());
            }
        }
        Ok(())
    }

    pub(super) fn check_history_endpoints(&self, history: &Path, endpoints: &[PathBuf])
        -> Result<(), String>
    {
        let history = fields::resolve_root(history)?;
        for endpoint in endpoints {
            if overlaps(&history, &fields::resolve_root(endpoint)?) {
                return Err("learned evaluation history output conflicts with a service endpoint".into());
            }
        }
        Ok(())
    }

    /// All selected file names remain inputs for this whole invocation. A
    /// later request's listener cannot acquire one after its file disappears.
    pub(super) fn check_sequence_endpoint_inputs(
        &self,
        endpoints: &[PathBuf],
        paths: &[PathBuf],
    ) -> Result<(), String> {
        let endpoints = endpoints.iter().map(|path| fields::resolve_root(path))
            .collect::<Result<Vec<_>, _>>()?;
        for path in paths.iter().chain(self.inputs.iter()) {
            let input = fields::resolve_root(path)?;
            if endpoints.iter().any(|endpoint| overlaps(endpoint, &input)) {
                return Err("learned service endpoint conflicts with a selected input".into());
            }
        }
        Ok(())
    }

    /// Validate every selected contract before a clock, listener, store or source
    /// operation. Unsupported compositions never silently lose an observer.
    pub(super) fn check(
        &self,
        config: &Config,
        loaded: &Loaded,
        actor: Option<&Profile>,
        reviewer: &PeerProfile,
        services: (Option<&operator::Selection>, Option<&precheck::Settings>),
        continuing: bool,
        finishing: Option<u64>,
        after_message: Option<u64>,
        credibility: Option<&Path>,
    ) -> Result<(), String> {
        let actor = actor.ok_or(USAGE)?;
        if ((finishing.is_some() || after_message.is_some()) && !continuing)
            || continuing != loaded.recovery_anchor.is_some()
            || continuing != self.history_input.is_some()
            || credibility.is_some()
            || loaded.anchor_output.is_none()
            || loaded.topology.is_some()
            || loaded.checkpoint_on_generation.is_some()
            || loaded.creation_campaigns.is_some()
            || loaded.expected_campaigns.is_some()
            || loaded.generation.required_pre_output_forecast().is_some()
            || loaded
                .generation
                .required_owned_pre_output_forecast()
                .is_some()
            || loaded.generation.required_held_out_joint().is_some()
            || config.profile.delivery.clock_domain != crate::config::CLOCK_DOMAIN
        {
            return Err(USAGE.into());
        }
        if continuing {
            super::credential::check_selection(loaded)?;
            self.recovery_requirements(config, loaded)?;
        } else {
            self.settings.creation()?;
            super::recipe::check_creation(config, loaded)?;
            super::credential::check_creation(loaded)?;
        }
        if let Some(publication) = &loaded.publication {
            publication.simple_limits()?;
        }
        let (operator, precheck) = services;
        self.peer.check_host(
            config,
            actor,
            reviewer,
            operator.map(|selected| &selected.settings),
            &[loaded.request],
            self.settings.config.identity(),
        )?;
        let history = loaded.anchor_output.as_deref().expect("required above");
        let mut custody = vec![config.source.path()];
        custody.extend(self.history_input.as_deref());
        if let Some(precheck) = precheck {
            self.peer.check_custody_paths(&[precheck.socket_path()])?;
        }
        if let Some(publication) = &loaded.publication {
            custody.extend(publication.input_paths().iter().map(PathBuf::as_path));
        }
        if let Some(identity) = &loaded.identity {
            custody.push(&identity.observed_manifest);
        }
        if let Some(credential) = &loaded.credential {
            custody.extend(credential.input_paths());
        }
        self.check_sequence_inputs(history, &custody)?;
        // A new output can alias a future endpoint even though neither exists.
        // Refuse before binding any listener or publishing the evaluated owner.
        let review_socket = reviewer.socket(loaded.request);
        let stop_socket = crate::workflow::control::socket_path(reviewer, loaded.request);
        let mut endpoints = vec![
            actor.socket.clone(),
            review_socket,
            stop_socket,
            self.peer.socket_path().to_path_buf(),
        ];
        if let Some(operator) = operator {
            endpoints.push(operator.settings.socket.clone());
        }
        if let Some(precheck) = precheck {
            endpoints.push(precheck.socket_path().to_path_buf());
        }
        self.check_history_endpoints(history, &endpoints)?;
        let custody = custody.into_iter().map(Path::to_path_buf).collect::<Vec<_>>();
        self.check_sequence_endpoint_inputs(&endpoints, &custody)?;
        Ok(())
    }

    pub(super) fn check_inputs(&self, loaded: &Loaded, paths: &[&Path]) -> Result<(), String> {
        let history = loaded.anchor_output.as_deref().ok_or(USAGE)?;
        self.check_sequence_inputs(history, paths)?;
        if let Some(path) = &self.history_input {
            self.check_sequence_inputs(history, &[path])?;
        }
        Ok(())
    }

    pub(super) fn create(
        &self,
        config: &Config,
        loaded: &Loaded,
    ) -> Result<(learned_recovery::Custody, FileIndependentEvaluator), String> {
        self.settings.creation()?;
        let guards = learned_recovery::requirements(config, loaded).guards;
        let registration = super::credential::registration(loaded)?;
        // The independently selected joint policy is part of the FIRST image.
        // A /2 failure cannot fall back to marginal evaluation, and this role
        // does not qualify or activate a held-out publication committee.
        let (host, roles) = match self.settings.joint {
            Some(joint) => {
                FileOversight::create_joint_andon_guarded_with_learned_generation_reserved(
                    &config.store,
                    config.profile.clone(),
                    &guards,
                    registration,
                    self.settings.protocol.clone(),
                    joint,
                    &self.settings.config,
                    loaded.generation.clone(),
                    RecoveryReserve::terminal(),
                )
            }
            None => FileOversight::create_andon_guarded_with_learned_generation_reserved(
                &config.store,
                config.profile.clone(),
                &guards,
                registration,
                self.settings.protocol.clone(),
                &self.settings.config,
                loaded.generation.clone(),
                RecoveryReserve::terminal(),
            ),
        }
        .map_err(debug)?;
        Ok((
            (
                host,
                roles.oversight.human,
                None,
                roles.oversight.identity_observer,
            ),
            roles.evaluator,
        ))
    }
}

/// Original generation/review/dispatch have ended. Keep explicit case discovery
/// and labeling available after Hold, without reacquiring source, numerical or
/// human custody. This is one remaining lifetime, never a new service lease.
pub(super) fn complete<F>(
    service: &mut evaluator::Service,
    driver: &mut FileSupervisedDriver,
    reviewer: &FileHumanReviewer,
    control: &mut Control,
    deadline: &Deadline,
    time: &mut F,
    intake: &mut Option<ingress::Intake>,
    operator: &mut Option<operator::Service>,
    poll_ms: u64,
    failure: &mut Option<String>,
) where
    F: FnMut() -> ElapsedTick,
{
    service.narrow_deadline(deadline);
    let may_assess = failure.is_none();
    while may_assess && !service.is_retired() && deadline.check(time()).is_ok() {
        // Stop owns every quantum, including the phase after a final Hold.
        match control.checkpoint(driver, reviewer, deadline, time) {
            Ok(false) => {}
            Ok(true) => break,
            Err(error) => {
                retain_failure(failure, error);
                break;
            }
        }
        operator::checkpoint_driver(operator, driver);
        match control.native_stop(driver, time) {
            Ok(false) => {}
            Ok(true) => break,
            Err(error) => {
                retain_failure(failure, error);
                break;
            }
        }
        service.checkpoint_driver(driver);
        // A failed actor socket cannot restart work or discard the independently
        // selected evaluator. Retire just that transport and keep its error.
        if let Err(error) = ingress::observe(intake, driver.supervisor_mut()) {
            retain_failure(
                failure,
                format!("learned actor response service failed: {error}; no effect retry was sent"),
            );
            drop(intake.take());
        }
        if !service.is_retired() {
            crate::workflow::pause(poll_ms.clamp(1, 50));
        }
    }
    // No queued assessment survives into actor reply grace or archive capture.
    service.finish();
    service.retire();
}

fn retain_failure(failure: &mut Option<String>, error: String) {
    *failure = Some(match failure.take() {
        Some(original) => format!("{original}; {error}"),
        None => error,
    });
}
