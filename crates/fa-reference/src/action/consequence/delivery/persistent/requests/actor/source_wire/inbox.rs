//! Bounded scheduling hints from the original authenticated actor session.
//! Requests, outcomes and rights remain exclusively in the native journal.
use super::{FileActorPeerDrive, FileActorPeerDriveError, FileSupervisedDriver, Port};
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::delivery::persistent::{JournalError, requests::{FileRequestDisposition, FileRequestStatus, MAX_FILE_REQUESTS}};
use crate::action::consequence::oversight::actor_peer::{PeerAdmission, PeerPolicy, PeerRefusal, PeerSession, PeerSessionStatus};
use crate::action::consequence::oversight::actor_transport::DriveBudget;
use crate::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits};
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
use crate::Error;
use std::collections::VecDeque;
use std::os::unix::net::UnixStream;

/// One authenticated actor session and a bounded, non-authoritative ready queue.
/// A queue entry carries only a request ID; dequeue rechecks its ORIGINAL status.
/// It cannot resurrect cancelled work, supply a verdict or turn a retry into a
/// new execution. Dropping this inbox does not cancel or refund accepted work.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorInbox;
/// fn grant(inbox: FileActorInbox) { inbox.authorize(); }
/// ```
pub struct FileActorInbox {
    session: PeerSession<Port>,
    ready: VecDeque<u64>,
}
impl FileActorInbox {
    pub fn new(policy: PeerPolicy, wire: ActorWire<Port>, limits: ChannelLimits,
        connection_limit: u64) -> Result<Self, Error>
    {
        let session = PeerSession::new(policy, wire, limits, connection_limit)?;
        let mut ready = VecDeque::new();
        ready.try_reserve_exact(MAX_FILE_REQUESTS).map_err(|_| Error::Limit)?;
        Ok(Self { session, ready })
    }
    pub fn status(&self) -> PeerSessionStatus { self.session.status() }
    pub fn queued(&self) -> usize { self.ready.len() }
    pub fn attach(&mut self, socket: UnixStream) -> Result<PeerAdmission, PeerRefusal> {
        self.session.attach(socket)
    }
    pub fn disconnect(&mut self) -> bool { self.session.disconnect() }
    /// Ingress revocation is not a native admission/endpoint stop. Preserve the
    /// queue so the real supervisor can still settle its original obligations.
    pub fn revoke(&mut self) -> bool { self.session.revoke() }

    /// Decode and submit through the ORIGINAL framed transport and source gate.
    /// Socket fragments, poll/cancel and exact/conflicting retries keep their
    /// existing source-read behavior. A seen Submit is only a scheduling hint:
    /// after the drive we query the journal, not an intake report or socket write.
    /// Lost output therefore cannot hide a durably accepted request from work.
    pub fn drive<S, F>(&mut self, driver: &mut FileSupervisedDriver, source: &mut S,
        mut clock: F, budget: DriveBudget) -> Result<FileActorPeerDrive, FileActorPeerDriveError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let result = (|| {
            budget.validate()?;
            let supervisor = driver.supervisor_mut();
            supervisor.check_source_wire(self.session.request_port())?;
            // Reserve bounded bookkeeping BEFORE any frame can change a journal.
            let mut seen = Vec::new();
            seen.try_reserve_exact(budget.frames).map_err(|_| JournalError::from(Error::Limit))?;
            let mut intakes = Vec::new();
            intakes.try_reserve_exact(budget.frames).map_err(|_| JournalError::from(Error::Limit))?;
            let drive = self.session.drive_with_admission(budget, |_, request, _| {
                seen.push(request);
                let mut intake = None;
                let result = supervisor.prepare_wire_submission(request, source, &mut clock, &mut intake);
                if let Some(report) = intake { intakes.push(report); }
                result
            })?;
            let host = supervisor.host()?;
            for request in seen {
                match host.request_status(request) {
                    Ok(status) if work(status) && !self.ready.contains(&request) => {
                        if self.ready.len() == MAX_FILE_REQUESTS {
                            return Err(JournalError::from(Error::Limit).into());
                        }
                        self.ready.push_back(request);
                    }
                    Ok(_) | Err(JournalError::Contract(Error::Missing)) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(FileActorPeerDrive { drive, intakes })
        })();
        driver.reap_helpers();
        result
    }

    /// FIFO by observed submission, not numeric ID. A requeued active request is
    /// harmless: its current terminal/cancelled status is skipped after the job.
    /// A returned dispatch/unknown request is reconciliation-only, NEVER review.
    /// This removes a scheduling hint, not a durable record or live capability.
    pub fn next_request(&mut self, driver: &FileSupervisedDriver) -> Result<Option<FileRequestStatus>, JournalError> {
        driver.supervisor().check_source_wire(self.session.request_port())?;
        let host = driver.supervisor().host()?;
        while let Some(&request) = self.ready.front() {
            // A faulted/missing owner must not consume the hint before recovery.
            let status = host.request_status(request)?;
            self.ready.pop_front();
            if work(status) { return Ok(Some(status)); }
        }
        Ok(None)
    }
}
fn work(status: FileRequestStatus) -> bool {
    matches!(status.disposition, FileRequestDisposition::Admitted {
        stage: ActionState::Reviewing | ActionState::Dispatching | ActionState::Unknown, ..
    })
}
