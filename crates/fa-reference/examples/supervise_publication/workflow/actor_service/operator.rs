//! Separately authenticated native inspection and policy replay beside the owner.
//! Observational grants never admit requests or enter the control path.
#[path = "operator/profile.rs"]
mod profile;
pub(crate) use profile::Settings;

use super::{
    BoundSocket, Deadline, Duration, FileSupervisedDriver, Instant, Port, Profile, debug, io,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileLearnedTextStreamActorPort, FileSupervisorExplanationGrant, FileSupervisorHoldDiagnosisGrant,
    FileSupervisorInspectionGrant, FileSupervisorPolicyReplayGrant,
    FileSupervisorReviewExplanationGrant,
};
use fa_reference::action::consequence::oversight::actor_peer::PeerRefusal;
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{
    ChannelLimits, ChannelState, CloseReason,
};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::observation_algebra::AuthorityKind;
use fa_reference::tower::publication::peer::PublicationPeerSession;
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{MAX_REQUEST_BYTES, TowerAccess, TowerInstallError};
use fa_reference::typestate_cx::PurposeTag;
use std::collections::BTreeSet;

struct Active {
    socket: BoundSocket,
    session: PublicationPeerSession,
    inspection: Option<FileSupervisorInspectionGrant>,
    explanation: Option<FileSupervisorExplanationGrant>,
    hold_diagnosis: Option<FileSupervisorHoldDiagnosisGrant>,
    review_explanation: Option<FileSupervisorReviewExplanationGrant>,
    policy_replay: Option<FileSupervisorPolicyReplayGrant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Retirement {
    SupervisorEnded,
    Expired,
    CandidateLimit,
    ConnectionLimit,
    ExchangeLimit,
    TransportFailure,
}

pub(super) struct Service {
    active: Option<Active>,
    started: Instant,
    wall: Duration,
    candidates: u64,
    candidate_limit: u64,
    frames: usize,
    retirement: Option<Retirement>,
}

impl Service {
    /// Issue only selected original observational grants. No source, clock, snapshot,
    /// helper input or journal mutation is needed to expose this endpoint.
    pub(super) fn start(
        settings: &Settings,
        actor: &Profile,
        port: Port,
        driver: &FileSupervisedDriver,
        generation: u64,
        deadline: &Deadline,
    ) -> Result<Self, String> {
        Self::start_with(settings, actor, driver, generation, deadline,
            |mount, limits| PublicationTower::new(mount, port, limits))
    }

    /// Observe this exact learned owner without obtaining an ordinary actor
    /// port. The native facade permanently refuses effectful Tower operations.
    pub(super) fn start_learned(
        settings: &Settings,
        actor: &Profile,
        port: FileLearnedTextStreamActorPort,
        driver: &FileSupervisedDriver,
        generation: u64,
        deadline: &Deadline,
    ) -> Result<Self, String> {
        Self::start_with(settings, actor, driver, generation, deadline,
            |mount, limits| PublicationTower::new_learned_observer(mount, port, limits))
    }

    fn start_with(
        settings: &Settings,
        actor: &Profile,
        driver: &FileSupervisedDriver,
        generation: u64,
        deadline: &Deadline,
        build: impl FnOnce(Address, PublicationLimits) -> Result<PublicationTower, TowerInstallError>,
    ) -> Result<Self, String> {
        settings.check_actor(actor)?;
        settings.check_unbound()?;
        if generation == 0 {
            return Err("operator Tower registration generation must be nonzero".into());
        }
        let started = Instant::now();
        let wall = Duration::from_millis(settings.runtime_ms.min(actor.runtime_ms))
            .min(deadline.wall.saturating_sub(deadline.started.elapsed()));
        if wall.is_zero() {
            return Err("operator listener exceeded the original service lifetime".into());
        }
        let mount = Address::new(
            settings.scope.tenant.to_string(),
            Noun::Run,
            settings.scope.run.to_string(),
            Some(generation),
        )
        .map_err(debug)?;
        let mut tower = build(mount, PublicationLimits {
            max_response_bytes: settings.response_bytes,
        }).map_err(debug)?;
        let inspection = if settings.inspection_queries != 0 {
            let (port, custodian) = driver
                .supervisor()
                .inspection_port(settings.inspection_queries)
                .map_err(debug)?;
            tower.set_inspection(Some(port)).map_err(debug)?;
            Some(custodian)
        } else {
            None
        };
        let explanation = if settings.explanation_queries != 0 {
            let (port, custodian) = driver
                .supervisor()
                .explanation_port_with_budget(
                    settings.explanation_queries,
                    settings.explanation_replay_events,
                )
                .map_err(debug)?;
            tower.set_explanation(Some(port)).map_err(debug)?;
            Some(custodian)
        } else {
            None
        };
        let hold_diagnosis = if let Some(selected) = &settings.hold_diagnosis {
            let (port, custodian) = driver
                .supervisor()
                .hold_diagnosis_port(selected.queries)
                .map_err(debug)?;
            tower.set_hold_diagnosis(Some(port)).map_err(debug)?;
            Some(custodian)
        } else {
            None
        };
        let review_explanation = if let Some(selected) = &settings.review_explanation {
            let (port, custodian) = match selected.numerical_replay_events {
                Some(events) => driver.supervisor().review_explanation_port_with_numerical_budget(
                    selected.queries,
                    selected.max_replay_events,
                    events,
                ),
                None => driver
                    .supervisor()
                    .review_explanation_port_with_budget(
                        selected.queries,
                        selected.max_replay_events,
                    ),
            }
            .map_err(debug)?;
            tower.set_review_explanation(Some(port)).map_err(debug)?;
            Some(custodian)
        } else {
            None
        };
        let policy_replay = if let Some(selected) = &settings.policy_replay {
            let (port, custodian) = if selected.page_queries == 0 {
                driver
                    .supervisor()
                    .policy_replay_port(selected.queries, selected.limits)
            } else {
                driver.supervisor().policy_replay_port_with_pages(
                    selected.queries,
                    selected.limits,
                    selected.page_queries,
                )
            }
            .map_err(debug)?;
            tower.set_policy_replay(Some(port)).map_err(debug)?;
            Some(custodian)
        } else {
            None
        };
        // Experiment permits only the explicitly selected native comparison.
        // It grants neither publication admission nor policy promotion.
        let purposes = if policy_replay.is_some() {
            vec![PurposeTag::Experiment]
        } else {
            Vec::new()
        };
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: settings.scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            purposes,
            0,
        );
        let session = PublicationPeerSession::new(
            settings.operator,
            tower,
            access,
            ChannelLimits {
                frame_bytes: MAX_REQUEST_BYTES,
                exchanges: settings.exchange_limit,
            },
            settings.connection_limit,
        )
        .map_err(debug)?;
        if started.elapsed() >= wall {
            return Err("operator listener setup exceeded its original lifetime".into());
        }
        let socket = BoundSocket::bind(&settings.socket, None)?;
        settings.secure_socket()?;
        let mut service = Self {
            active: Some(Active {
                socket,
                session,
                inspection,
                explanation,
                hold_diagnosis,
                review_explanation,
                policy_replay,
            }),
            started,
            wall,
            candidates: 0,
            candidate_limit: settings.candidate_limit,
            frames: 0,
            retirement: None,
        };
        if service.started.elapsed() >= service.wall {
            service.retire_with(Retirement::Expired);
            return Err("operator listener setup exceeded its original lifetime".into());
        }
        eprintln!(
            "Tower operator endpoint ready: {:?}; registration={generation}",
            settings.socket
        );
        Ok(service)
    }

    /// One bounded operator turn. Call only AFTER the original independent Stop
    /// checkpoint. A transport fault retires this sidecar, never publication.
    pub(super) fn checkpoint(&mut self) {
        if self.retirement.is_some() {
            return;
        }
        if self.started.elapsed() >= self.wall {
            self.retire_with(Retirement::Expired);
            return;
        }
        let active = self.active.as_mut().expect("active operator service");
        if active.session.status().active.is_none() {
            if self.candidates == self.candidate_limit {
                self.retire_with(Retirement::CandidateLimit);
                return;
            }
            let status = active.session.status();
            if status.connections_admitted == status.connection_limit {
                self.retire_with(Retirement::ConnectionLimit);
                return;
            }
            if active.session.remaining_exchanges() == 0 {
                self.retire_with(Retirement::ExchangeLimit);
                return;
            }
            let socket = match active.socket.listener.accept() {
                Ok((socket, _)) => socket,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    return;
                }
                Err(_) => {
                    self.retire_with(Retirement::TransportFailure);
                    return;
                }
            };
            self.candidates += 1;
            match active.session.attach(socket) {
                Ok(_) => {}
                Err(PeerRefusal::CredentialsRejected) => {
                    if self.candidates == self.candidate_limit {
                        self.retire_with(Retirement::CandidateLimit);
                    }
                    return;
                }
                Err(_) => {
                    self.retire_with(Retirement::TransportFailure);
                    return;
                }
            }
        }
        let report = match active.session.drive(DriveBudget {
            frames: 1,
            ..DriveBudget::default()
        }) {
            Ok(report) => report,
            Err(_) => {
                self.retire_with(Retirement::TransportFailure);
                return;
            }
        };
        self.frames += report.progress.frames;
        if report.status.failure.is_some() {
            self.retire_with(Retirement::TransportFailure);
        } else if self.started.elapsed() >= self.wall {
            self.retire_with(Retirement::Expired);
        } else if let ChannelState::Closed(reason) = report.status.channel {
            match reason {
                CloseReason::PeerClosed => {
                    active.session.disconnect();
                }
                CloseReason::ExchangeLimit => self.retire_with(Retirement::ExchangeLimit),
                CloseReason::TruncatedFrame
                | CloseReason::FrameTooLarge
                | CloseReason::IoFailure => {
                    self.retire_with(Retirement::TransportFailure);
                }
            }
        }
    }

    pub(super) fn retire(&mut self) {
        self.retire_with(Retirement::SupervisorEnded);
    }

    fn retire_with(&mut self, reason: Retirement) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        active.session.revoke();
        self.retirement = Some(reason);
        eprintln!(
            "Tower operator ingress retired: {reason:?}; candidates={}; frames={}; inspection_queries={:?}; explanation_queries={:?}; hold_diagnosis_queries={:?}; review_explanation_queries={:?}; numerical_replay_events={:?}; policy_replay_queries={:?}",
            self.candidates,
            self.frames,
            active
                .inspection
                .as_ref()
                .map(FileSupervisorInspectionGrant::remaining_queries),
            active
                .explanation
                .as_ref()
                .map(FileSupervisorExplanationGrant::remaining_queries),
            active
                .hold_diagnosis
                .as_ref()
                .map(FileSupervisorHoldDiagnosisGrant::remaining_queries),
            active
                .review_explanation
                .as_ref()
                .map(FileSupervisorReviewExplanationGrant::remaining_queries),
            active
                .review_explanation
                .as_ref()
                .and_then(FileSupervisorReviewExplanationGrant::remaining_numerical_replay_events),
            active
                .policy_replay
                .as_ref()
                .map(FileSupervisorPolicyReplayGrant::remaining_queries)
        );
        // Dropping the only custodians revokes all their weak ports/cursors.
        // BoundSocket removes only this invocation's original socket inode.
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(test)]
#[path = "operator/tests.rs"]
mod tests;
