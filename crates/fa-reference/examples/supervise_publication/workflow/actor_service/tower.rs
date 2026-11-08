//! The original ordinary actor lifecycle with the common Tower request format.
//! This changes ingress only: the same supervisor acquires source evidence,
//! launches reviews, applies both keys, publishes, stops and cleans up.

#[path = "tower/client.rs"]
pub(super) mod client;

use super::{
    ActorWire, BoundSocket, Config, ElapsedTick, FileSupervisedDriver, Intake, PeerProfile,
    PeerRefusal, PeerSession, Port, Profile, PublicationProfile, debug, drive_budget, io,
    serve_selected_ingress,
};
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
const USAGE: &str = "serve-create|serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation GENERATION [--requests KEYS | --precheck-profile PRECHECK_PROFILE] [--operator-profile OPERATOR_PROFILE]; serve-create-checked|serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation GENERATION [--requests KEYS | --precheck-profile PRECHECK_PROFILE] [--operator-profile OPERATOR_PROFILE] (operator profile requires a single-request service)";

struct Arguments<'a> {
    config: &'a Path,
    actor: &'a Path,
    reviewer: &'a Path,
    publication: Option<&'a Path>,
    open: bool,
    generation: u64,
    requests: Option<Vec<u64>>,
    precheck: Option<&'a Path>,
    operator: Option<&'a Path>,
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
        if ![positional + 2, positional + 4, positional + 6].contains(&args.len())
            || args[1..positional]
                .iter()
                .any(|value| value.is_empty() || value.starts_with("--"))
        {
            return Err(USAGE.into());
        }
        let mut generation = None;
        let mut requests = None;
        let mut precheck = None;
        let mut operator = None;
        // Each opt-in owns one complete pair. Operator read custody is separate
        // from admission-time precheck disclosure and cannot widen actor grants.
        for pair in args[positional..].chunks_exact(2) {
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
                "--operator-profile" if operator.is_none() => {
                    if pair[1].is_empty() || pair[1].starts_with("--") {
                        return Err(USAGE.into());
                    }
                    operator = Some(Path::new(&pair[1]));
                }
                _ => return Err(USAGE.into()),
            }
        }
        if requests.is_some() && (precheck.is_some() || operator.is_some()) {
            return Err(USAGE.into());
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
            operator,
        })
    }
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    // Own all options before opening files or allowing another service mode to
    // consume an unsupported Tower selection. No transport/profile fallback.
    let selected = Arguments::parse(args)?;
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
    if let Some(requests) = selected.requests.as_deref() {
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
            super::super::clock,
        );
    }
    serve_selected_ingress(
        config,
        &actor,
        &reviewer,
        publication.as_ref(),
        selected.open,
        credibility,
        None,
        precheck.as_ref(),
        Some(selected.generation),
        operator.as_ref(),
        super::super::clock,
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
