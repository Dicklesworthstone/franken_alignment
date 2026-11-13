//! Separate authenticated custody of the ORIGINAL live policy governor.
//! Only an explicit complete-report approval delegates its original opaque key;
//! the independently selected operator still performs the policy transition.
#[path = "governor/profile.rs"]
mod profile;
#[path = "governor/protocol.rs"]
pub(crate) mod protocol;
pub(crate) use profile::Settings;
#[path = "governor/client.rs"]
pub(crate) mod client;

#[cfg(test)]
use super::FileSupervisedDriver;
use super::{BoundSocket, Deadline, Duration, Instant, Profile, operator};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::governance::campaigns::{
    FilePolicyCampaignReview, FilePolicyGovernor,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, governance::PolicyUpdate,
};
use protocol::{Command, Request};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

const MAX_IO_BYTES: usize = 64 * 1_048_576;
const MAX_IO_CALLS: usize = 65_536;
const TURN_CALLS: usize = 16;
const TURN_BYTES: usize = 65_536;
const MAX_CACHE_BYTES: usize = 32 * 1_048_576;
const MAX_CACHE_ENTRIES: usize = 32;

struct Cached {
    request: Vec<u8>,
    correlation: protocol::Correlation,
    phase: &'static str,
    entered: bool,
    // Marking entry precedes native work. An unwound/unacknowledged operation
    // can only return Unconfirmed, never repeat Approve to recover a lost key.
    response: Option<Rc<Vec<u8>>>,
}

struct Prepared {
    review: FilePolicyCampaignReview,
    report: String,
}

struct Connection {
    socket: UnixStream,
    header: [u8; 4],
    header_read: usize,
    length: Option<usize>,
    input: Vec<u8>,
    output: Option<(Rc<Vec<u8>>, usize)>,
}
impl Connection {
    fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            header: [0; 4],
            header_read: 0,
            length: None,
            input: Vec::new(),
            output: None,
        }
    }
    fn reset_input(&mut self) {
        self.header_read = 0;
        self.length = None;
        self.input.clear();
    }
}

pub(super) struct Service {
    settings: Settings,
    socket: Option<BoundSocket>,
    connection: Option<Connection>,
    role: Option<FilePolicyGovernor>,
    started: Instant,
    wall: Duration,
    candidates: u64,
    connections: u64,
    exchanges: u64,
    read: usize,
    written: usize,
    io_calls: usize,
    campaigns_entered: usize,
    approval_entered: bool,
    remaining_numerical_replay_events: Option<usize>,
    prepared: BTreeMap<u64, Prepared>,
    cache: Vec<Cached>,
    cache_bytes: usize,
    retired: bool,
}

impl Service {
    pub(super) fn start(
        settings: &Settings,
        actor: &Profile,
        role: FilePolicyGovernor,
        deadline: &Deadline,
    ) -> Result<Self, String> {
        settings.check_actor(actor)?;
        settings.check_unbound()?;
        let started = Instant::now();
        let wall = Duration::from_millis(settings.runtime_ms.min(actor.runtime_ms))
            .min(deadline.wall.saturating_sub(deadline.started.elapsed()));
        if wall.is_zero() {
            return Err("governor setup exceeded the original service lifetime".into());
        }
        let socket = BoundSocket::bind(&settings.socket, None)?;
        settings.secure_socket()?;
        if started.elapsed() >= wall {
            return Err("governor setup exceeded the original service lifetime".into());
        }
        Ok(Self {
            settings: settings.clone(),
            socket: Some(socket),
            connection: None,
            role: Some(role),
            started,
            wall,
            candidates: 0,
            connections: 0,
            exchanges: 0,
            read: 0,
            written: 0,
            io_calls: 0,
            campaigns_entered: 0,
            approval_entered: false,
            remaining_numerical_replay_events: settings
                .numerical_replay
                .map(|selected| selected.events),
            prepared: BTreeMap::new(),
            cache: Vec::new(),
            cache_bytes: 0,
            retired: false,
        })
    }

    /// A bounded cooperative turn AFTER the original independent Stop check.
    /// This acquires no source snapshot or native clock observation. A dropped
    /// transport preserves native/cache quotas; retiring this sidecar never
    /// drops an already delegated grant held by the independent operator.
    #[cfg(test)]
    pub(super) fn checkpoint(
        &mut self,
        driver: &mut FileSupervisedDriver,
        operator: &mut operator::Service,
    ) {
        self.checkpoint_supervisor(driver.supervisor_mut(), operator);
    }

    pub(super) fn checkpoint_supervisor(
        &mut self,
        supervisor: &mut FileActorSupervisor<FileOversight>,
        operator: &mut operator::Service,
    ) {
        if self.retired {
            return;
        }
        if self.started.elapsed() >= self.wall {
            self.retire();
            return;
        }
        let mut calls = 0;
        if self.connection.is_none() {
            if self.candidates == self.settings.candidate_limit
                || self.connections == self.settings.connection_limit
                || self.exchanges == self.settings.exchange_limit
            {
                self.retire();
                return;
            }
            if !self.call(&mut calls) {
                self.retire();
                return;
            }
            let accepted = self
                .socket
                .as_ref()
                .expect("live governor socket")
                .listener
                .accept();
            let socket = match accepted {
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
                    self.retire();
                    return;
                }
            };
            self.candidates += 1;
            if !self.call(&mut calls) {
                self.retire();
                return;
            }
            // SO_PEERCRED is observed on THIS connection before any byte read.
            if self.settings.governor.verify(&socket).is_err() {
                return;
            }
            if !self.call(&mut calls) || socket.set_nonblocking(true).is_err() {
                self.retire();
                return;
            }
            self.connections += 1;
            self.connection = Some(Connection::new(socket));
        }
        let mut connection = self
            .connection
            .take()
            .expect("accepted governor connection");
        let mut transferred = 0;
        let mut frames = 0;
        while calls < TURN_CALLS && transferred < TURN_BYTES {
            if self.started.elapsed() >= self.wall {
                self.retire();
                return;
            }
            if let Some((bytes, offset)) = &mut connection.output {
                let allowed = (bytes.len() - *offset)
                    .min(TURN_BYTES - transferred)
                    .min(MAX_IO_BYTES.saturating_sub(self.written));
                if allowed == 0 || !self.call(&mut calls) {
                    self.retire();
                    return;
                }
                match connection.socket.write(&bytes[*offset..*offset + allowed]) {
                    Ok(0) => return,
                    Ok(count) => {
                        self.written += count;
                        transferred += count;
                        *offset += count;
                        if *offset == bytes.len() {
                            connection.output = None;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    // A lost reply never revokes the already installed original
                    // grant. A new authenticated connection can ask its exact
                    // cached acknowledgment under the SAME lifetime allowances.
                    Err(_) => return,
                }
                continue;
            }
            if self.exchanges == self.settings.exchange_limit {
                self.retire();
                return;
            }
            if frames == 1 {
                break;
            }
            if connection.header_read < 4 {
                let allowed = (4 - connection.header_read)
                    .min(TURN_BYTES - transferred)
                    .min(MAX_IO_BYTES.saturating_sub(self.read));
                if allowed == 0 || !self.call(&mut calls) {
                    self.retire();
                    return;
                }
                match connection.socket.read(
                    &mut connection.header
                        [connection.header_read..connection.header_read + allowed],
                ) {
                    Ok(0) => return,
                    Ok(count) => {
                        self.read += count;
                        transferred += count;
                        connection.header_read += count;
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => return,
                }
                if connection.header_read < 4 {
                    continue;
                }
                let length = u32::from_be_bytes(connection.header) as usize;
                if length == 0 || length > protocol::MAX_REQUEST_BYTES {
                    self.exchanges += 1;
                    return;
                }
                if connection.input.try_reserve_exact(length).is_err() {
                    self.retire();
                    return;
                }
                connection.length = Some(length);
            }
            let length = connection.length.expect("complete bounded header");
            if connection.input.len() < length {
                if calls == TURN_CALLS || transferred == TURN_BYTES {
                    break;
                }
                let mut bytes = [0_u8; 8192];
                let allowed = (length - connection.input.len())
                    .min(bytes.len())
                    .min(TURN_BYTES - transferred)
                    .min(MAX_IO_BYTES.saturating_sub(self.read));
                if allowed == 0 || !self.call(&mut calls) {
                    self.retire();
                    return;
                }
                match connection.socket.read(&mut bytes[..allowed]) {
                    Ok(0) => return,
                    Ok(count) => {
                        self.read += count;
                        transferred += count;
                        connection.input.extend_from_slice(&bytes[..count]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => return,
                }
                if connection.input.len() < length {
                    continue;
                }
            }
            self.exchanges += 1;
            frames += 1;
            let request = match protocol::decode(&connection.input, self.settings.report_bytes) {
                Ok(request) => request,
                Err(_) => return,
            };
            connection.reset_input();
            if self.started.elapsed() >= self.wall {
                self.retire();
                return;
            }
            let response = self.respond_supervisor(&request, supervisor, operator);
            connection.output = Some((response, 0));
        }
        self.connection = Some(connection);
    }

    fn call(&mut self, turn: &mut usize) -> bool {
        if self.io_calls == MAX_IO_CALLS || *turn == TURN_CALLS {
            return false;
        }
        self.io_calls += 1;
        *turn += 1;
        true
    }

    #[cfg(test)]
    fn respond(
        &mut self,
        request: &Request,
        driver: &mut FileSupervisedDriver,
        operator: &mut operator::Service,
    ) -> Rc<Vec<u8>> {
        self.respond_supervisor(request, driver.supervisor_mut(), operator)
    }

    fn respond_supervisor(
        &mut self,
        request: &Request,
        supervisor: &mut FileActorSupervisor<FileOversight>,
        operator: &mut operator::Service,
    ) -> Rc<Vec<u8>> {
        self.respond_with(request, |service, index| match &request.command {
            Command::Inspect { .. } => service.inspect(request, supervisor),
            Command::Review { .. } => service.review(request, index, supervisor),
            Command::Approve { .. } => service.approve(request, index, supervisor, operator),
        })
    }

    /// All commands share the same finite canonical-request cache. Inspection
    /// retries therefore preserve their original cut without repeating a read.
    fn respond_with(
        &mut self,
        request: &Request,
        apply: impl FnOnce(&mut Self, usize) -> Result<Vec<u8>, &'static str>,
    ) -> Rc<Vec<u8>> {
        if request.scope != self.settings.scope {
            return frame(protocol::refused(request, "audience"));
        }
        if let Some(cached) = self
            .cache
            .iter()
            .find(|cached| cached.request == request.canonical)
        {
            return cached
                .response
                .as_ref()
                .map(Rc::clone)
                .unwrap_or_else(|| frame(protocol::refused(request, "unconfirmed")));
        }
        if self.cache.iter().any(|cached| {
            cached.entered
                && cached.phase == "review"
                && request.phase() == "review"
                && cached.correlation == request.correlation()
        }) {
            return frame(protocol::refused(request, "idempotency_conflict"));
        }
        let maximum_reply = match request.command {
            Command::Inspect { .. } => 4096,
            Command::Review { .. } => 2 * self.settings.report_bytes + 4096,
            Command::Approve { .. } => 1024,
        };
        if self.cache.len() == MAX_CACHE_ENTRIES
            || self
                .cache_bytes
                .checked_add(request.canonical.len())
                .and_then(|bytes| bytes.checked_add(maximum_reply + 4))
                .is_none_or(|bytes| bytes > MAX_CACHE_BYTES)
        {
            return frame(protocol::refused(request, "capacity"));
        }
        let index = self.cache.len();
        self.cache_bytes += request.canonical.len();
        self.cache.push(Cached {
            request: request.canonical.clone(),
            correlation: request.correlation(),
            phase: request.phase(),
            entered: false,
            response: None,
        });
        let response =
            apply(self, index).unwrap_or_else(|reason| protocol::refused(request, reason));
        let response = frame(response);
        self.cache_bytes += response.len();
        self.cache[index].response = Some(Rc::clone(&response));
        response
    }

    fn inspect(
        &self,
        request: &Request,
        supervisor: &FileActorSupervisor<FileOversight>,
    ) -> Result<Vec<u8>, &'static str> {
        let host = supervisor.host().map_err(|_| "unavailable")?;
        if !host.policy_campaigns_required()
            || (host.decoder_required() && self.settings.numerical_replay.is_none())
            || host.action_consistency_required()
        {
            return Err("unsupported");
        }
        let cut = host.inspect();
        let policy_generation = host.current_policy().map_err(native_reason)?.generation();
        protocol::inspected(
            request,
            protocol::Inspection {
                cut: protocol::Expected {
                    journal_revision: cut.revision,
                    control_sequence: cut.control.sequence,
                    authority_epoch: cut.control.ledger.epoch,
                    policy_generation,
                },
                campaigns: self.settings.campaign_requirement(),
                campaign_limit: self.settings.campaign_limit,
                report_bytes: self.settings.report_bytes,
                stopped: cut.stop.is_some(),
                source_interrupted: host.file_source_status().map(|source| source.interrupted),
                numerical_replay: self.settings.numerical_replay.map(|selected| {
                    protocol::NumericalReplayInspection {
                        selected,
                        remaining_events: self
                            .remaining_numerical_replay_events
                            .expect("selected original numerical allowance"),
                    }
                }),
            },
        )
        .map_err(|_| "capacity")
    }

    fn review(
        &mut self,
        request: &Request,
        cache: usize,
        supervisor: &mut FileActorSupervisor<FileOversight>,
    ) -> Result<Vec<u8>, &'static str> {
        let Command::Review {
            operation,
            expected,
            candidate,
        } = &request.command
        else {
            unreachable!()
        };
        if self.approval_entered {
            return Err("operation_limit");
        }
        if self.campaigns_entered == self.settings.campaign_limit {
            return Err("capacity");
        }
        if candidate.limits != self.settings.campaign_requirement().limits {
            return Err("binding");
        }
        let host = supervisor.host().map_err(|_| "unavailable")?;
        if !host.policy_campaigns_required()
            || (host.decoder_required() && self.settings.numerical_replay.is_none())
            || host.action_consistency_required()
        {
            return Err("unsupported");
        }
        let before = host.inspect();
        if before.stop.is_some() {
            return Err("unsupported");
        }
        if host
            .file_source_status()
            .is_some_and(|source| source.interrupted)
        {
            return Err("source_unavailable");
        }
        if before.revision != expected.journal_revision
            || before.control.sequence != expected.control_sequence
            || before.control.ledger.epoch != expected.authority_epoch
            || host.current_policy().map_err(native_reason)?.generation()
                != expected.policy_generation
        {
            return Err("stale_predecessor");
        }
        match host.policy_campaign(*operation) {
            Err(JournalError::Contract(Error::Missing)) => {}
            Ok(_) => return Err("idempotency_conflict"),
            Err(error) => return Err(native_reason(error)),
        }
        let update = PolicyUpdate::new(
            *operation,
            expected.control_sequence,
            expected.authority_epoch,
            candidate.candidate.clone(),
        )
        .map_err(|_| "binding")?;
        let numerical = host.decoder_required();
        drop(host);
        self.reserve_numerical_replay(before.revision, numerical)?;
        // All exact native/source/Stop checks and work admission precede this
        // privileged borrow, which consumes an unused admission snapshot.
        let mut host = supervisor.host_mut().map_err(|_| "unavailable")?;
        self.cache[cache].entered = true;
        self.campaigns_entered += 1;
        let review = host
            .request_policy_campaign(expected.journal_revision, &update)
            .map_err(native_reason)?;
        if Some(review.observed_revision()) != expected.journal_revision.checked_add(1) {
            return Err("unconfirmed");
        }
        // The complete report retains the selected PRE-request cut. The reply
        // separately names the acknowledged Request revision used by Approve.
        let report = crate::policy_replay::encode(
            &before,
            review.report(),
            candidate.limits,
            candidate.output_bytes,
        )
        .map_err(|_| "capacity")?;
        let report = String::from_utf8(report).map_err(|_| "unavailable")?;
        let response = protocol::review(request, review.observed_revision(), &report)
            .map_err(|_| "capacity")?;
        self.prepared
            .insert(*operation, Prepared { review, report });
        Ok(response)
    }

    fn approve(
        &mut self,
        request: &Request,
        cache: usize,
        supervisor: &mut FileActorSupervisor<FileOversight>,
        operator: &mut operator::Service,
    ) -> Result<Vec<u8>, &'static str> {
        let Command::Approve {
            campaign,
            expected_journal_revision,
            reviewed_report,
            accept_newly_reviewable,
        } = &request.command
        else {
            unreachable!()
        };
        if self.approval_entered {
            return Err("operation_limit");
        }
        if !operator.promotion_available() {
            return Err("operator_unavailable");
        }
        let prepared = self.prepared.get(campaign).ok_or("campaign_not_approved")?;
        if prepared.report.as_bytes() != reviewed_report.as_bytes() {
            return Err("report_mismatch");
        }
        let host = supervisor.host().map_err(|_| "unavailable")?;
        if !host.policy_campaigns_required()
            || host.action_consistency_required()
            || (host.decoder_required() && self.settings.numerical_replay.is_none())
        {
            return Err("unsupported");
        }
        if host.revision() != *expected_journal_revision
            || prepared.review.observed_revision() != *expected_journal_revision
        {
            return Err("stale_predecessor");
        }
        if host
            .file_source_status()
            .is_some_and(|source| source.interrupted)
        {
            return Err("source_unavailable");
        }
        if host.inspect().stop.is_some() {
            return Err("unsupported");
        }
        if self.role.is_none() {
            return Err("unavailable");
        }
        let numerical = host.decoder_required();
        drop(host);
        self.reserve_numerical_replay(*expected_journal_revision, numerical)?;
        let mut host = supervisor.host_mut().map_err(|_| "unavailable")?;
        let role = self.role.as_ref().expect("original governor checked above");
        let prepared = self
            .prepared
            .get(campaign)
            .expect("original review checked above");
        self.approval_entered = true;
        self.cache[cache].entered = true;
        let key = match role.approve(
            &mut host,
            *expected_journal_revision,
            &prepared.review,
            *accept_newly_reviewable,
        ) {
            Ok(key) => key,
            // A known native contract refusal committed no approval. A newly
            // reviewed campaign may still use this role; this exact refusal is
            // cached and will never be silently reinterpreted as an approval.
            Err(error @ JournalError::Contract(_)) => {
                self.approval_entered = false;
                return Err(native_reason(error));
            }
            Err(_) => return Err("unconfirmed"),
        };
        let revision = host.revision();
        drop(host);
        if Some(revision) != expected_journal_revision.checked_add(1) {
            return Err("unconfirmed");
        }
        // Install once BEFORE acknowledgment output. The operator owns the
        // grant independently of this socket, its reply and governor lifetime.
        operator
            .install_policy_promotion_supervisor(supervisor, key)
            .map_err(|_| "unconfirmed")?;
        protocol::approved(request, revision).map_err(|_| "unconfirmed")
    }

    /// Reserve every entered native reconstruction from the original lifetime
    /// allowance. Cache hits never reach this method; failures never refund it.
    fn reserve_numerical_replay(
        &mut self,
        revision: u64,
        numerical: bool,
    ) -> Result<(), &'static str> {
        if !numerical {
            return Ok(());
        }
        let selected = self.settings.numerical_replay.ok_or("unsupported")?;
        let work = usize::try_from(revision)
            .ok()
            .and_then(|revision| revision.checked_add(1))
            .ok_or("capacity")?;
        let remaining = self
            .remaining_numerical_replay_events
            .as_mut()
            .ok_or("unsupported")?;
        if work > selected.max_replay_events || work > *remaining {
            return Err("capacity");
        }
        *remaining -= work;
        Ok(())
    }

    pub(super) fn retire(&mut self) {
        if self.retired {
            return;
        }
        self.retired = true;
        self.connection = None;
        self.socket = None;
        self.role = None;
        self.prepared.clear();
        self.cache.clear();
        self.cache_bytes = 0;
    }
}

fn native_reason(error: JournalError) -> &'static str {
    match error {
        JournalError::Contract(Error::Stale) => "stale_predecessor",
        JournalError::Contract(Error::Binding) => "binding",
        JournalError::Contract(Error::Limit) => "capacity",
        JournalError::Contract(_) => "campaign_not_approved",
        JournalError::Io(_) => "unconfirmed",
        _ => "unavailable",
    }
}

fn frame(body: Vec<u8>) -> Rc<Vec<u8>> {
    let mut bytes = Vec::with_capacity(body.len() + 4);
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&body);
    Rc::new(bytes)
}

#[cfg(test)]
#[path = "governor/tests.rs"]
pub(super) mod tests;
