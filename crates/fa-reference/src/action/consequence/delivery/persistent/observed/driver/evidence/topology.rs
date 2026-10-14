//! Current topology AND original policy evidence at the driver's capture cuts.
//! These explicit methods do not install a new persisted deployment profile.
use super::{EvidenceFile, EvidenceIdentity, EvidenceProvider, FileCredentialPermit,
    FileDriverEvent, FileEvidenceReport, FileHumanPermit, FileHumanRequest,
    FileOversight, FileProvider, FileReviewLaunch, FileSourceReviewError,
    FileSupervisedDriver, JournalError, DriverEvidence, ElapsedTick, FrozenAction, Error};
use super::super::super::mediation::source::{FileTopologySource, TopologyRefresh};
use crate::action::consequence::mediation::CutCheck;
use crate::action::consequence::oversight::helper_processes::HelperProgram;
use std::os::unix::net::UnixStream;

/// Supervisor-only diagnostics: keep source acknowledgment separate from the
/// ORIGINAL operation result. At most two topology observations per step, one
/// per launch/approval. An empty vector means capture was never reached; it is
/// not a successful check. No partial observation supplies a publication key.
#[derive(Debug)]
pub struct TopologyReport<T> {
    pub topology: Vec<Result<TopologyRefresh, JournalError>>,
    pub result: T,
}

impl FileSupervisedDriver {
    /// Capture topology immediately BEFORE each original policy/input capture,
    /// not before phase selection. Loss discovered after dispatch can therefore
    /// reach the original checked endpoint's nonexecution seal. Reconciliation,
    /// already-resolved outcomes and expired effects keep their source-free path.
    pub fn step_from_file_with_topology<S, F>(&mut self, source: &mut S,
        topology: &mut FileTopologySource, clock: F, human: Option<&FileHumanPermit>)
        -> TopologyReport<FileEvidenceReport<FileDriverEvent>>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_topology(source, topology, clock, human, None)
    }

    /// Same evidence cuts with separately held original publication credentials.
    /// This never substitutes topology custody for either approval or credential.
    pub fn step_from_file_with_topology_and_credential<S, F>(&mut self, source: &mut S,
        topology: &mut FileTopologySource, clock: F, human: Option<&FileHumanPermit>,
        credential: &FileCredentialPermit) -> TopologyReport<FileEvidenceReport<FileDriverEvent>>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_topology(source, topology, clock, human, Some(credential))
    }

    fn step_topology<S, F>(&mut self, source: &mut S, topology: &mut FileTopologySource,
        clock: F, human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> TopologyReport<FileEvidenceReport<FileDriverEvent>>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut reports = Vec::with_capacity(2);
        let mut observations = Vec::with_capacity(2);
        let mut source_updates = Vec::with_capacity(2);
        let result = self.step_with_provider(clock, &mut TopologyProvider {
            topology, reports: &mut reports,
            policy: FileProvider { source, observations: &mut observations, updates: &mut source_updates },
        }, human, credential);
        TopologyReport { topology: reports, result: FileEvidenceReport { observations, source_updates, result } }
    }

    /// Obtain the original human request only after both current sources. The
    /// original job/input/control checks run before reads and again afterward.
    /// This does not approve the request or reserve automatic effect rights.
    pub fn request_human_approval_from_file_with_topology<S, F>(&mut self, source: &mut S,
        topology: &mut FileTopologySource, key: u64, expires_at: ElapsedTick, clock: F)
        -> TopologyReport<FileEvidenceReport<FileHumanRequest>>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut reports = Vec::with_capacity(1);
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        let result = self.request_human_from_provider(key, expires_at, clock, &mut TopologyProvider {
            topology, reports: &mut reports,
            policy: FileProvider { source, observations: &mut observations, updates: &mut source_updates },
        });
        TopologyReport { topology: reports, result: FileEvidenceReport { observations, source_updates, result } }
    }

    /// Check the caller's original request/input/worker predecessor before any
    /// topology I/O; check topology before policy capture and socket admission.
    pub fn start_file_review_with_topology<S, F>(&mut self, source: &mut S,
        topology: &mut FileTopologySource, launch: FileReviewLaunch<UnixStream>, clock: F)
        -> TopologyReport<Result<EvidenceIdentity, FileSourceReviewError>>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut reports = Vec::with_capacity(1);
        let result = self.start_file_review_checked(source, launch, clock,
            |host| require_topology(host, topology, &mut reports));
        TopologyReport { topology: reports, result }
    }

    /// Same exact admission before original executable-helper creation. A
    /// topology failure cannot spawn a helper or fall back to manual ballots.
    pub fn start_file_process_review_with_topology<S, F>(&mut self, source: &mut S,
        topology: &mut FileTopologySource, launch: FileReviewLaunch<HelperProgram>, clock: F)
        -> TopologyReport<Result<EvidenceIdentity, FileSourceReviewError>>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut reports = Vec::with_capacity(1);
        let result = self.start_file_process_review_checked(source, launch, clock,
            |host| require_topology(host, topology, &mut reports));
        TopologyReport { topology: reports, result }
    }
}

struct TopologyProvider<'a, S: ?Sized> {
    topology: &'a mut FileTopologySource,
    reports: &'a mut Vec<Result<TopologyRefresh, JournalError>>,
    policy: FileProvider<'a, S>,
}
impl<S: EvidenceFile + ?Sized> EvidenceProvider for TopologyProvider<'_, S> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        // An acknowledged withdrawal is missing evidence, so the original
        // publication branch may seal it. An unacknowledged source operation
        // remains an OUTER failure and cannot attempt publication afterward.
        if !refresh(host, self.topology, self.reports)? { return Ok(Err(Error::Incomplete)); }
        self.policy.capture(host, action, clock)
    }
}

fn refresh(host: &mut FileOversight, source: &mut FileTopologySource,
    reports: &mut Vec<Result<TopologyRefresh, JournalError>>) -> Result<bool, JournalError>
{
    let revision = host.revision();
    let observed = source.refresh_next(host, revision);
    let accepted = match &observed {
        Ok(report) => Ok(report.refusal.is_none() && matches!(&report.checked, Some(Ok(CutCheck::Verified(_))))),
        Err(error) => Err(error.clone()),
    };
    reports.push(observed);
    accepted
}

fn require_topology(host: &mut FileOversight, source: &mut FileTopologySource,
    reports: &mut Vec<Result<TopologyRefresh, JournalError>>) -> Result<(), JournalError>
{
    if refresh(host, source, reports)? { Ok(()) } else { Err(Error::Incomplete.into()) }
}
