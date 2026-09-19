//! Acquire feed freshness at the ORIGINAL supervised evidence boundaries.
//! Independent feed failures are not falsely labeled as committee-input drift.
use super::{Callback, CommitteeContract, DriverEvidence, ElapsedTick, Error,
    EvidenceFile, EvidenceProvider, FileCaptureError, FileCredentialPermit,
    FileDriverEvent, FileEvidenceReport, FileHumanPermit, FileOversight,
    FileProvider, FilePublicationDriverReport, FileSupervisedDriver, FrozenAction,
    JournalError, PublicationInputFile, PublicationProvider};
use super::super::super::super::publication::capture::heartbeat::PublicationHeartbeatFile;
use crate::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessStatus;

/// Each entry records an actual heartbeat acquisition or read failure. An
/// acquired status is historical: the effect gate independently checks expiry
/// again AFTER committee/witness capture. Outer persistence failure supplies no
/// fabricated observation. At most two acquisitions occur in a driver step.
#[derive(Debug)]
pub struct FileHeartbeatDriverReport<T> {
    pub heartbeats: Vec<Result<PublicationFreshnessStatus, FileCaptureError>>,
    pub publication: FilePublicationDriverReport<T>,
}

impl FileSupervisedDriver {
    /// Renew the concrete feed observation, not its producer timestamp, before
    /// the existing committee callback and witness-file capture at each original
    /// evidence boundary. Both profiles and the witness source must be bound.
    /// Idle/reconciliation and already resolved/expired publication skip readers.
    pub fn step_with_publication_heartbeat<F, P>(&mut self,
        source: &PublicationInputFile, heartbeat: &PublicationHeartbeatFile,
        clock: F, provider: P, human: Option<&FileHumanPermit>,
        credential: Option<&FileCredentialPermit>) -> FileHeartbeatDriverReport<FileDriverEvent>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let mut heartbeats = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut PublicationProvider {
            inner: HeartbeatProvider { inner: Callback(provider), source: heartbeat, reports: &mut heartbeats },
            attempt, source, reads: &mut reads,
        }, human, credential);
        FileHeartbeatDriverReport { heartbeats, publication: FilePublicationDriverReport {
            reads, evidence: FileEvidenceReport { observations: Vec::new(), source_updates: Vec::new(), result },
        } }
    }

    /// Three concrete readers, one original driver: heartbeat, native policy/
    /// committee source, then witness source. The policy-source adapter preserves
    /// its existing leases, producer floors and durable withdrawal behavior.
    /// These sequential reads are NOT an atomic cross-producer snapshot.
    pub fn step_from_files_with_publication_heartbeat<S, F>(&mut self,
        evidence_source: &mut S, source: &PublicationInputFile,
        heartbeat: &PublicationHeartbeatFile, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileHeartbeatDriverReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let attempt = self.job.as_ref().map(|job| job.attempt);
        let mut heartbeats = Vec::with_capacity(2);
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let mut reads = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut PublicationProvider {
            inner: HeartbeatProvider {
                inner: FileProvider { source: evidence_source, observations: &mut observations, updates: &mut source_updates },
                source: heartbeat, reports: &mut heartbeats,
            },
            attempt, source, reads: &mut reads,
        }, human, credential);
        FileHeartbeatDriverReport { heartbeats, publication: FilePublicationDriverReport {
            reads, evidence: FileEvidenceReport { observations, source_updates, result },
        } }
    }
}

struct HeartbeatProvider<'a, P> {
    inner: P,
    source: &'a PublicationHeartbeatFile,
    reports: &'a mut Vec<Result<PublicationFreshnessStatus, FileCaptureError>>,
}
impl<P: EvidenceProvider> EvidenceProvider for HeartbeatProvider<'_, P> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        // The OUTER original PublicationProvider has already durably withdrawn
        // witness eligibility. This read independently withdraws feed eligibility
        // before I/O and quarantines before its post-read clock can unwind.
        let report = host.refresh_publication_heartbeat(host.revision(), self.source, &mut *clock)?;
        self.reports.push(report);
        // Do not turn a feed outage into an invented changed committee packet.
        // Actual committee/witness capture still runs and its own error stays
        // distinct. A missing/expired feed can never permit an effect: the
        // mandatory native gate rechecks it at authorize/dispatch/publication.
        // This preserves the original unspent permit for a genuine fresh retry.
        self.inner.capture(host, action, clock)
    }
}
