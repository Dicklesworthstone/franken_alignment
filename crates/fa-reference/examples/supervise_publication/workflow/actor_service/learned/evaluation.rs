//! Independent post-review assessment of the same original learned owner.
//! Selection precedes work; the finite evaluator lifetime includes generation
//! and review. A late label never reruns either or supplies an effect key.
use super::super::{Profile, andon_settings, evaluator};
use super::{
    Config, Control, Deadline, ElapsedTick, FileHumanReviewer, FileOversight, FileSupervisedDriver,
    Loaded, PeerProfile, RecoveryReserve, debug, fields, ingress, operator, precheck, recovery,
};
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const USAGE: &str = "learned evaluation requires fresh create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE with --actor-profile ACTOR_PROFILE, --andon ANDON_PROFILE, --evaluator-profile EVALUATOR_PROFILE and --retain-history-anchor NEW_OUTPUT; optional precheck and paired operator/generation retain their original contracts";

pub(super) struct Options {
    accounting: PathBuf,
    peer: PathBuf,
}

/// Own the complete opted-in grammar before another parser can discard a mode.
/// Existing invocations without either evaluation flag keep their exact argv.
pub(super) fn take_options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<Options>), String> {
    if !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--andon" | "--evaluator-profile"))
    {
        return Ok((args.to_vec(), None));
    }
    if credibility.is_some()
        || args.first().map(String::as_str) != Some("create-learned-generated")
        || args.len() < 12
        || (args.len() - 4) % 2 != 0
        || args[1..4]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut accounting = None;
    let mut peer = None;
    let mut seen = BTreeSet::new();
    let mut remaining = args[..4].to_vec();
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--andon" => accounting = Some(PathBuf::from(&pair[1])),
            "--evaluator-profile" => peer = Some(PathBuf::from(&pair[1])),
            "--actor-profile"
            | "--retain-history-anchor"
            | "--precheck-profile"
            | "--operator-profile"
            | "--tower-generation" => remaining.extend_from_slice(pair),
            _ => return Err(USAGE.into()),
        }
    }
    if !seen.contains("--actor-profile")
        || !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
    {
        return Err(USAGE.into());
    }
    Ok((
        remaining,
        Some(Options {
            accounting: accounting.ok_or(USAGE)?,
            peer: peer.ok_or(USAGE)?,
        }),
    ))
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
        if settings.joint.is_some() {
            return Err(
                "learned live evaluation requires the marginal evaluation/Andon profile".into(),
            );
        }
        let peer = evaluator::Settings::read(&self.peer)?;
        Ok(Selection {
            settings,
            peer,
            inputs: [self.accounting, self.peer],
        })
    }
}

impl Selection {
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
        if continuing
            || finishing.is_some()
            || after_message.is_some()
            || credibility.is_some()
            || loaded.recovery_anchor.is_some()
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
            || self.settings.joint.is_some()
        {
            return Err(USAGE.into());
        }
        self.settings.creation()?;
        super::recipe::check_creation(config, loaded)?;
        super::credential::check_creation(loaded)?;
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
        let mut custody = vec![
            config.source.path(),
            history,
            self.inputs[0].as_path(),
            self.inputs[1].as_path(),
        ];
        if let Some(precheck) = precheck {
            custody.push(precheck.socket_path());
        }
        if let Some(publication) = &loaded.publication {
            custody.extend(publication.input_paths().iter().map(PathBuf::as_path));
        }
        self.peer.check_custody_paths(&custody)?;
        // A new output can alias a future endpoint even though neither exists.
        // Refuse before binding any listener or publishing the evaluated owner.
        let history = fields::resolve_root(history)?;
        let review_socket = reviewer.socket(loaded.request);
        let stop_socket = crate::workflow::control::socket_path(reviewer, loaded.request);
        let mut endpoints = vec![
            actor.socket.as_path(),
            review_socket.as_path(),
            stop_socket.as_path(),
            self.peer.socket_path(),
        ];
        if let Some(operator) = operator {
            endpoints.push(&operator.settings.socket);
        }
        if let Some(precheck) = precheck {
            endpoints.push(precheck.socket_path());
        }
        for endpoint in endpoints {
            if history == fields::resolve_root(endpoint)? {
                return Err(
                    "learned evaluation history output conflicts with a service endpoint".into(),
                );
            }
        }
        Ok(())
    }

    pub(super) fn check_inputs(&self, paths: &[&Path]) -> Result<(), String> {
        self.peer.check_custody_paths(paths)
    }

    pub(super) fn create(
        &self,
        config: &Config,
        loaded: &Loaded,
    ) -> Result<(recovery::Custody, FileIndependentEvaluator), String> {
        self.settings.creation()?;
        let guards = recovery::requirements(config, loaded).guards;
        let registration = super::credential::registration(loaded)?;
        let (host, roles) = FileOversight::create_andon_guarded_with_learned_generation_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            registration,
            self.settings.protocol.clone(),
            &self.settings.config,
            loaded.generation.clone(),
            RecoveryReserve::terminal(),
        )
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
