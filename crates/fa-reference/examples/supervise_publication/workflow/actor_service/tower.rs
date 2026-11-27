//! The original ordinary actor lifecycle with the common Tower request format.
//! This changes ingress only: the same supervisor acquires source evidence,
//! launches reviews, applies both keys, publishes, stops and cleans up.

#[path = "tower/client.rs"]
pub(super) mod client;

use super::super::continuation::creation::topology::Selection as TopologySelection;
use super::super::continuation::guarded::mediated::profile::Selection as RecoveryTopologySelection;
use super::{
    ActorWire, BoundSocket, Config, ElapsedTick, FileSupervisedDriver, Intake, PeerProfile,
    PeerRefusal, PeerSession, Port, Profile, PublicationProfile, debug, drive_budget, io,
    serve_selected_ingress,
};
use crate::recover_stop::guarded::SubmitProfile;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPrecheckPort;
use fa_reference::action::consequence::oversight::actor_wire::ChannelLimits;
use fa_reference::action::consequence::oversight::evidence_source::EvidenceFile;
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::observation_algebra::AuthorityKind;
use fa_reference::tower::publication::peer::PublicationPeerSession;
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{MAX_REQUEST_BYTES, TowerAccess};
use fa_reference::typestate_cx::PurposeTag;
use std::collections::BTreeSet;
use std::path::Path;

const FLAG: &str = "--tower-generation";
const HISTORY_USAGE: &str = "--evaluate-only requires serve-open or serve-open-checked with --tower-generation, --requests, --andon, --evaluator-profile and --guarded; select an independently retained input history and new retain_history_anchor in the guarded profile; a governed history requires its exact effective policy and campaign limits in fa.guarded-submit/2; operator, precheck, topology, live governor and credibility selections are not supported";
const USAGE: &str = "serve-create|serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation GENERATION [--requests KEYS | --precheck-profile PRECHECK_PROFILE] [--operator-profile OPERATOR_PROFILE]; serve-create-checked|serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation GENERATION [--requests KEYS | --precheck-profile PRECHECK_PROFILE] [--operator-profile OPERATOR_PROFILE]; create modes also accept --topology-profile TOPOLOGY_PROFILE with optional --requests KEYS or single-request --precheck-profile PRECHECK_PROFILE; ordinary open modes accept --guarded GUARDED_SUBMIT_V1_PROFILE --requests KEYS with an optional operator and one or more requests; mediated open modes accept --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE, with optional --requests KEYS or single new-request --precheck-profile PRECHECK_PROFILE, and --operator-profile; plain and simple-witness checked scheduled modes with at least two requests also accept --governor-profile GOVERNOR_PROFILE paired with the independent operator policy_promotion selection and optional original topology; governed open additionally requires --guarded GUARDED_SUBMIT_V2_PROFILE with independent effective-policy and campaign expectations, and governed mediated recovery requires at least one new scheduled request; ordinary plain/simple-witness create and independently guarded open modes with --requests also accept --precheck-schedule SCHEDULE, optionally with operator, its independent governor and original topology; recovery requires explicit null rows for recorded requests and current selected epochs for new disclosures; never combine a schedule with --precheck-profile; ordinary Tower plain/simple-witness Andon creation requires --andon ANDON_PROFILE --retain-history-anchor NEW_FILE and permits --requests KEYS; Andon recovery requires --andon ANDON_PROFILE --guarded GUARDED_SUBMIT_V1_PROFILE --requests KEYS with a new retain_history_anchor in that profile; both permit --operator-profile OPERATOR_PROFILE, --evaluator-profile EVALUATOR_PROFILE and --precheck-schedule SCHEDULE with explicit --requests, and paired --governor-profile with independently funded operator policy promotion; governed creation requires at least two explicit requests, governed recovery also requires fa.guarded-submit/2 with exact effective policy and campaign expectations; topology, standalone prechecks and credibility activation remain unsupported; ordinary Andon open modes accept --evaluate-only with --evaluator-profile and explicit recorded --requests, retaining independent input/output history in --guarded; governed histories require fa.guarded-submit/2; this mode rejects operator, precheck, topology, live governor and credibility selections; evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON";

struct Arguments<'a> {
    config: &'a Path,
    actor: &'a Path,
    reviewer: &'a Path,
    publication: Option<&'a Path>,
    open: bool,
    generation: u64,
    requests: Option<Vec<u64>>,
    precheck: Option<&'a Path>,
    precheck_schedule: Option<&'a Path>,
    operator: Option<&'a Path>,
    topology: Option<&'a Path>,
    guarded: Option<&'a Path>,
    governor: Option<&'a Path>,
    andon: Option<&'a Path>,
    retain_history: Option<&'a Path>,
    evaluator: Option<&'a Path>,
    evaluate_only: bool,
}

impl<'a> Arguments<'a> {
    fn parse(args: &'a [String]) -> Result<Self, String> {
        let (open, checked) = match args.first().map(String::as_str) {
            Some("serve-create") => (false, false),
            Some("serve-open") => (true, false),
            Some("serve-create-checked") => (false, true),
            Some("serve-open-checked") => (true, true),
            _ => return Err(USAGE.into()),
        };
        let positional = if checked { 5 } else { 4 };
        if args.len() < positional + 2
            || args.len() > positional + 21
            || args[1..positional]
                .iter()
                .any(|value| value.is_empty() || value.starts_with("--"))
        {
            return Err(USAGE.into());
        }
        let mut generation = None;
        let mut requests = None;
        let mut precheck = None;
        let mut precheck_schedule = None;
        let mut operator = None;
        let mut topology = None;
        let mut guarded = None;
        let mut governor = None;
        let mut andon = None;
        let mut retain_history = None;
        let mut evaluator = None;
        let mut evaluate_only = false;
        // Each opt-in owns one complete pair. Operator read custody is separate
        // from admission-time precheck disclosure and cannot widen actor grants.
        // Historical evaluation is an explicit unary mode, never a value that
        // another option or a different service grammar may silently consume.
        let mut index = positional;
        while index < args.len() {
            if args[index] == "--evaluate-only" {
                if evaluate_only {
                    return Err(HISTORY_USAGE.into());
                }
                evaluate_only = true;
                index += 1;
                continue;
            }
            let pair = args.get(index..index + 2).ok_or(USAGE)?;
            match pair[0].as_str() {
                FLAG if generation.is_none() => {
                    let value = &pair[1];
                    if value.is_empty()
                        || value.len() > 20
                        || value.starts_with('0')
                        || !value.bytes().all(|byte| byte.is_ascii_digit())
                    {
                        return Err(
                            "Tower registration generation must be a nonzero canonical decimal u64"
                                .into(),
                        );
                    }
                    generation = Some(
                        value
                            .parse()
                            .map_err(|_| "Tower registration generation exceeds u64".to_owned())?,
                    );
                }
                "--requests" if requests.is_none() => {
                    requests = Some(super::series::parse_requests(&pair[1])?);
                }
                "--precheck-profile" if precheck.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    precheck = Some(Path::new(&pair[1]));
                }
                "--precheck-schedule" if precheck_schedule.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    precheck_schedule = Some(Path::new(&pair[1]));
                }
                "--operator-profile" if operator.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    operator = Some(Path::new(&pair[1]));
                }
                "--topology-profile" if topology.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    topology = Some(Path::new(&pair[1]));
                }
                "--guarded" if guarded.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    guarded = Some(Path::new(&pair[1]));
                }
                "--governor-profile" if governor.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    governor = Some(Path::new(&pair[1]));
                }
                "--andon" if andon.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    andon = Some(Path::new(&pair[1]));
                }
                "--retain-history-anchor" if retain_history.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    retain_history = Some(Path::new(&pair[1]));
                }
                "--evaluator-profile" if evaluator.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    evaluator = Some(Path::new(&pair[1]));
                }
                _ => return Err(USAGE.into()),
            }
            index += 2;
        }
        if evaluate_only
            && (!open
                || andon.is_none()
                || evaluator.is_none()
                || guarded.is_none()
                || requests.is_none()
                || retain_history.is_some()
                || operator.is_some()
                || precheck.is_some()
                || precheck_schedule.is_some()
                || topology.is_some()
                || governor.is_some())
        {
            return Err(HISTORY_USAGE.into());
        }
        if (precheck_schedule.is_some()
            && (requests.is_none() || (open && guarded.is_none()) || precheck.is_some()))
            || (requests.is_some() && precheck.is_some())
            || (guarded.is_some() && !open)
            || (guarded.is_some()
                && topology.is_none()
                && governor.is_none()
                && (requests.is_none() || precheck.is_some()))
            || (open && topology.is_some() && guarded.is_none())
            || (governor.is_some()
                && (precheck.is_some()
                    || open != guarded.is_some()
                    || operator.is_none()
                    || requests.as_ref().is_none_or(|requests| requests.len() < 2)))
        {
            return Err(USAGE.into());
        }
        if andon.is_some() {
            if topology.is_some()
                || precheck.is_some()
                || (open && (guarded.is_none() || requests.is_none() || retain_history.is_some()))
                || (!open && retain_history.is_none())
            {
                return Err("live Andon requires ordinary plain/simple-witness Tower service, a new --retain-history-anchor on creation or --guarded plus explicit --requests on recovery; topology and single-profile prechecks require separate composition".into());
            }
        } else if retain_history.is_some() || evaluator.is_some() {
            return Err(
                "live history retention and evaluator custody require an explicit --andon profile"
                    .into(),
            );
        }
        let generation = generation.ok_or(USAGE)?;
        Ok(Self {
            config: Path::new(&args[1]),
            actor: Path::new(&args[2]),
            reviewer: Path::new(&args[3]),
            publication: checked.then(|| Path::new(&args[4])),
            open,
            generation,
            requests,
            precheck,
            precheck_schedule,
            operator,
            topology,
            guarded,
            governor,
            andon,
            retain_history,
            evaluator,
            evaluate_only,
        })
    }
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    command_with_clock(args, credibility, super::super::clock)
}

pub(super) fn command_with_clock<F>(
    args: &[String],
    credibility: Option<&Path>,
    time: F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    // Own all options before opening files or allowing another service mode to
    // consume an unsupported Tower selection. No transport/profile fallback.
    let selected = Arguments::parse(args)?;
    if selected.andon.is_some() && credibility.is_some() {
        return Err("live Andon service does not select a credibility activation capsule".into());
    }
    if !selected.open && credibility.is_some() {
        return Err("live credibility activation requires an existing journal".into());
    }
    let config = Config::read(selected.config)?;
    let actor = Profile::read(selected.actor)?;
    let reviewer = PeerProfile::read(selected.reviewer)?;
    let publication = selected
        .publication
        .map(PublicationProfile::read)
        .transpose()?;
    let precheck = selected
        .precheck
        .map(super::precheck::Settings::read)
        .transpose()?;
    let operator = selected
        .operator
        .map(super::operator::Settings::read)
        .transpose()?;
    if operator
        .as_ref()
        .is_some_and(|settings| settings.policy_promotion.is_some())
        != selected.governor.is_some()
    {
        return Err("live policy promotion requires both an independent governor profile and an explicit operator promotion allowance".into());
    }
    if let Some(path) = selected.andon {
        let settings = super::andon_settings::Settings::read(path)?;
        let recovery = selected.guarded.map(SubmitProfile::read).transpose()?;
        let evaluator = selected
            .evaluator
            .map(super::evaluator::Settings::read)
            .transpose()?;
        let governor = selected
            .governor
            .map(super::governor::Settings::read)
            .transpose()?;
        let singleton = [actor.request];
        let requests = selected.requests.as_deref().unwrap_or(&singleton);
        let prechecks = selected
            .precheck_schedule
            .map(|path| super::precheck::schedule::Schedule::read(path, requests))
            .transpose()?;
        let mut inputs = vec![selected.config, selected.actor, selected.reviewer, path];
        inputs.extend(selected.publication);
        inputs.extend(selected.operator);
        inputs.extend(selected.governor);
        inputs.extend(selected.evaluator);
        inputs.extend(selected.guarded);
        inputs.extend(selected.precheck_schedule);
        let selection = super::andon::Selection {
            settings: &settings,
            mode: match recovery.as_ref() {
                Some(profile) => super::andon::Mode::Existing(profile),
                None => super::andon::Mode::Create(
                    selected
                        .retain_history
                        .expect("Andon creation custody checked before reads"),
                ),
            },
            requests,
            generation: selected.generation,
            operator: operator.as_ref(),
            governor: governor.as_ref(),
            evaluator: evaluator.as_ref(),
            prechecks: prechecks.as_ref(),
            inputs: &inputs,
        };
        return if selected.evaluate_only {
            super::andon::serve_history(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                selection,
                time,
            )
        } else {
            super::andon::serve(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                selection,
                time,
            )
        };
    }
    let recovery_prechecks = if selected.open {
        selected
            .precheck_schedule
            .map(|path| {
                let schedule = super::precheck::schedule::Schedule::read(
                    path,
                    selected
                        .requests
                        .as_deref()
                        .expect("precheck schedule checked before reads"),
                )?;
                let mut inputs = vec![selected.config, selected.actor, selected.reviewer];
                inputs.extend(selected.publication);
                inputs.extend(selected.operator);
                inputs.extend(selected.governor);
                inputs.extend(selected.guarded);
                inputs.extend(selected.topology);
                inputs.extend(credibility);
                schedule.check_recovery_inputs(&inputs)?;
                Ok::<_, String>(schedule)
            })
            .transpose()?
    } else {
        None
    };
    if let Some(path) = selected.governor {
        let governor = super::governor::Settings::read(path)?;
        if selected.open {
            let recovery = SubmitProfile::read(
                selected
                    .guarded
                    .expect("governed recovery selection checked before reads"),
            )?;
            if let Some(path) = selected.topology {
                let topology = RecoveryTopologySelection::read(path)?;
                return super::topology::recovery::serve_sequence_governed(
                    config,
                    &actor,
                    &reviewer,
                    publication.as_ref(),
                    credibility,
                    &recovery,
                    &topology,
                    selected.generation,
                    operator
                        .as_ref()
                        .expect("independent operator checked before reads"),
                    &governor,
                    selected
                        .requests
                        .as_deref()
                        .expect("governor schedule checked before reads"),
                    recovery_prechecks.as_ref(),
                    time,
                );
            }
            return super::policy_recovery::serve(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                credibility,
                &recovery,
                selected
                    .requests
                    .as_deref()
                    .expect("governor schedule checked before reads"),
                selected.generation,
                operator
                    .as_ref()
                    .expect("independent operator checked before reads"),
                &governor,
                recovery_prechecks.as_ref(),
                time,
            );
        }
        if let Some(path) = selected.precheck_schedule {
            let mut inputs = vec![selected.config, selected.actor, selected.reviewer, path];
            inputs.extend(selected.publication);
            inputs.extend(selected.operator);
            inputs.extend(selected.topology);
            inputs.extend(selected.governor);
            governor.check_custody_paths(&inputs)?;
            let requests = selected
                .requests
                .as_deref()
                .expect("governor schedule checked before reads");
            let schedule = super::precheck::schedule::Schedule::read(path, requests)?;
            let topology = selected.topology.map(TopologySelection::read).transpose()?;
            return super::series::serve_tower_governed_prechecked(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                requests,
                selected.generation,
                operator
                    .as_ref()
                    .expect("independent operator checked before reads"),
                &governor,
                topology,
                &schedule,
                time,
            );
        }
        if let Some(path) = selected.topology {
            let topology = TopologySelection::read(path)?;
            return super::series::serve_tower_governed_with_topology(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                selected
                    .requests
                    .as_deref()
                    .expect("governor schedule checked before reads"),
                selected.generation,
                operator
                    .as_ref()
                    .expect("independent operator checked before reads"),
                &governor,
                topology,
                time,
            );
        }
        return super::series::serve_tower_governed_checked(
            config,
            &actor,
            &reviewer,
            publication.as_ref(),
            selected
                .requests
                .as_deref()
                .expect("governor schedule checked before reads"),
            selected.generation,
            operator
                .as_ref()
                .expect("independent operator checked before reads"),
            &governor,
            time,
        );
    }
    if let Some(path) = selected.guarded {
        let recovery = SubmitProfile::read(path)?;
        if selected.topology.is_none() {
            return super::policy_recovery::serve_plain(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                credibility,
                &recovery,
                selected
                    .requests
                    .as_deref()
                    .expect("plain guarded request schedule checked before reads"),
                selected.generation,
                operator.as_ref(),
                recovery_prechecks.as_ref(),
                time,
            );
        }
        let topology = RecoveryTopologySelection::read(
            selected
                .topology
                .expect("guarded topology pair checked before reads"),
        )?;
        if let Some(requests) = selected.requests.as_deref() {
            return super::topology::recovery::serve_sequence(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                credibility,
                &recovery,
                &topology,
                selected.generation,
                operator.as_ref(),
                requests,
                recovery_prechecks.as_ref(),
                time,
            );
        }
        return super::topology::recovery::serve(
            config,
            &actor,
            &reviewer,
            publication.as_ref(),
            credibility,
            &recovery,
            &topology,
            selected.generation,
            operator.as_ref(),
            precheck.as_ref(),
            time,
        );
    }
    let topology = selected.topology.map(TopologySelection::read).transpose()?;
    if let Some(requests) = selected.requests.as_deref() {
        if let Some(path) = selected.precheck_schedule {
            let schedule = super::precheck::schedule::Schedule::read(path, requests)?;
            return super::series::serve_tower_prechecked(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                requests,
                selected.generation,
                operator.as_ref(),
                topology,
                &schedule,
                time,
            );
        }
        if let Some(topology) = topology {
            return super::series::serve_tower_with_topology(
                config,
                &actor,
                &reviewer,
                publication.as_ref(),
                super::series::Options {
                    requests,
                    open: selected.open,
                    credibility,
                },
                selected.generation,
                operator.as_ref(),
                topology,
                time,
            );
        }
        return super::series::serve_tower(
            config,
            &actor,
            &reviewer,
            publication.as_ref(),
            super::series::Options {
                requests,
                open: selected.open,
                credibility,
            },
            selected.generation,
            operator.as_ref(),
            time,
        );
    }
    serve_selected_ingress(
        config,
        &actor,
        &reviewer,
        publication.as_ref(),
        selected.open,
        credibility,
        topology,
        precheck.as_ref(),
        Some(selected.generation),
        operator.as_ref(),
        time,
    )
}

pub(super) struct SelectedIngress {
    kind: Kind,
}

enum Kind {
    Native(Intake),
    Tower(TowerIntake),
}

impl SelectedIngress {
    pub(super) fn new(
        socket: BoundSocket,
        port: Port,
        actor: &Profile,
        generation: Option<u64>,
    ) -> Result<Self, String> {
        let kind = if let Some(generation) = generation {
            let mount = Address::new(
                actor.scope.tenant.to_string(),
                Noun::Run,
                actor.scope.run.to_string(),
                Some(generation),
            )
            .map_err(debug)?;
            let tower =
                PublicationTower::new(mount, port, PublicationLimits::default()).map_err(debug)?;
            let request_address = tower.request_address(actor.request).map_err(debug)?;
            // Only the existing actor intake is granted. This is not an operator,
            // full-domain grant, disclosure port, experiment context or permit.
            let access = TowerAccess::new(
                CallerScope {
                    caller_tenant: actor.scope.tenant.to_string(),
                    allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                    granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
                },
                false,
                vec![PurposeTag::Permit],
                0,
            );
            eprintln!(
                "Tower publication mount: {}; selected request: {}",
                tower.mount_address().canonical_string(),
                request_address.canonical_string(),
            );
            let session = PublicationPeerSession::new(
                actor.actor,
                tower,
                access,
                ChannelLimits {
                    frame_bytes: MAX_REQUEST_BYTES,
                    exchanges: actor.exchanges,
                },
                actor.connections,
            )
            .map_err(debug)?;
            Kind::Tower(TowerIntake {
                socket,
                session,
                selected: actor.request,
                attempted: 0,
                maximum: actor.candidates,
            })
        } else {
            let session = PeerSession::new(
                actor.actor,
                ActorWire::new(port),
                actor.channels(),
                actor.connections,
            )
            .map_err(debug)?;
            Kind::Native(Intake {
                socket,
                session,
                selected: actor.request,
                attempted: 0,
                maximum: actor.candidates,
            })
        };
        Ok(Self { kind })
    }

    /// Share the selected supervisor's original exact-only disclosure with the
    /// existing Tower session. Native actor ingress keeps its separate precheck
    /// protocol. Neither path creates a grant or renews a session allowance.
    pub(super) fn set_precheck(
        &mut self,
        port: Option<FileActorPrecheckPort>,
    ) -> Result<(), String> {
        match &mut self.kind {
            Kind::Native(_) => Ok(()),
            Kind::Tower(intake) => intake.session.set_precheck(port).map_err(debug),
        }
    }

    pub(super) fn admit<S, F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        source: &mut S,
        time: &mut F,
    ) -> Result<(), String>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        match &mut self.kind {
            Kind::Native(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_some() {
                    let report = driver
                        .drive_peer_request_from_file(
                            &mut intake.session,
                            intake.selected,
                            source,
                            time,
                            drive_budget(),
                        )
                        .map_err(debug)?;
                    if report.drive.status.closed() {
                        intake.session.disconnect();
                    }
                }
            }
            Kind::Tower(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_some() {
                    let report = driver
                        .drive_tower_request_from_file(
                            &mut intake.session,
                            intake.selected,
                            source,
                            time,
                            drive_budget(),
                        )
                        .map_err(debug)?;
                    if report.drive.status.closed() {
                        intake.session.disconnect();
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn observe(&mut self, driver: &mut FileSupervisedDriver) -> Result<usize, String> {
        match &mut self.kind {
            Kind::Native(intake) => intake.observe(driver),
            Kind::Tower(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_none() {
                    return Ok(0);
                }
                // The observation-only bridge accepts no source or clock and
                // permits retries only for the original recorded selected key.
                let report = driver
                    .drive_tower_request_observe(
                        &mut intake.session,
                        intake.selected,
                        drive_budget(),
                    )
                    .map_err(debug)?;
                if report.status.closed() {
                    intake.session.disconnect();
                }
                Ok(report.progress.frames)
            }
        }
    }

    /// Advance only the operator's validated schedule selection. Neither arm
    /// replaces its original session, tickets, credentials or lifetime quotas.
    pub(super) fn select(&mut self, request: u64) {
        match &mut self.kind {
            Kind::Native(intake) => intake.selected = request,
            Kind::Tower(intake) => intake.selected = request,
        }
    }

    pub(super) fn selected(&self) -> u64 {
        match &self.kind {
            Kind::Native(intake) => intake.selected,
            Kind::Tower(intake) => intake.selected,
        }
    }

    pub(super) fn admit_sequence<S, F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        completed: &[u64],
        source: &mut S,
        time: &mut F,
    ) -> Result<usize, String>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        match &mut self.kind {
            Kind::Native(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_none() {
                    return Ok(0);
                }
                let report = driver
                    .drive_peer_sequence_from_file(
                        &mut intake.session,
                        intake.selected,
                        completed,
                        source,
                        time,
                        drive_budget(),
                    )
                    .map_err(debug)?;
                if report.drive.status.closed() {
                    intake.session.disconnect();
                }
                Ok(report.drive.progress.frames)
            }
            Kind::Tower(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_none() {
                    return Ok(0);
                }
                let report = driver
                    .drive_tower_sequence_from_file(
                        &mut intake.session,
                        intake.selected,
                        completed,
                        source,
                        time,
                        drive_budget(),
                    )
                    .map_err(debug)?;
                if report.drive.status.closed() {
                    intake.session.disconnect();
                }
                Ok(report.drive.progress.frames)
            }
        }
    }

    /// The original observation path remains source-free during review,
    /// recovery and final grace. Complete predecessors are rechecked by the
    /// same native owner before entering either transport.
    pub(super) fn observe_sequence(
        &mut self,
        driver: &mut FileSupervisedDriver,
        completed: &[u64],
    ) -> Result<usize, String> {
        match &mut self.kind {
            Kind::Native(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_none() {
                    return Ok(0);
                }
                let report = driver
                    .drive_peer_sequence_observe(
                        &mut intake.session,
                        intake.selected,
                        completed,
                        drive_budget(),
                    )
                    .map_err(debug)?;
                if report.status.closed() {
                    intake.session.disconnect();
                }
                Ok(report.progress.frames)
            }
            Kind::Tower(intake) => {
                intake.accept()?;
                if intake.session.status().active.is_none() {
                    return Ok(0);
                }
                let report = driver
                    .drive_tower_sequence_observe(
                        &mut intake.session,
                        intake.selected,
                        completed,
                        drive_budget(),
                    )
                    .map_err(debug)?;
                if report.status.closed() {
                    intake.session.disconnect();
                }
                Ok(report.progress.frames)
            }
        }
    }

    pub(super) fn revoke(&mut self) {
        match &mut self.kind {
            Kind::Native(intake) => {
                intake.session.revoke();
            }
            Kind::Tower(intake) => {
                intake.session.revoke();
            }
        }
    }
}

struct TowerIntake {
    socket: BoundSocket,
    session: PublicationPeerSession,
    selected: u64,
    attempted: u64,
    maximum: u64,
}

impl TowerIntake {
    fn accept(&mut self) -> Result<(), String> {
        if self.session.status().active.is_some() {
            return Ok(());
        }
        let stream = match self.socket.listener.accept() {
            Ok((stream, _)) => stream,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(debug(error)),
        };
        if self.attempted == self.maximum {
            return Err("actor candidate quota exhausted".into());
        }
        self.attempted += 1;
        match self.session.attach(stream) {
            Ok(_) => Ok(()),
            Err(PeerRefusal::CredentialsRejected) if self.attempted < self.maximum => {
                eprintln!(
                    "Rejected Tower actor process credentials; candidate={}",
                    self.attempted
                );
                Ok(())
            }
            Err(error) => Err(debug(error)),
        }
    }
}

#[cfg(test)]
#[path = "tower/tests.rs"]
mod tests;
