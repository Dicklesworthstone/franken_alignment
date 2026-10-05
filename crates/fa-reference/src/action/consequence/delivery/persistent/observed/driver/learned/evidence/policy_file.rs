//! Policy-only observations on the ORIGINAL learned and leased-policy gates.
//! The source transaction supplies policy state, never an alternate helper view.
use super::{CurrentLearned, CommitteeInput, DriverEvidence, ElapsedTick, Error,
    EvidenceError, EvidenceFile, EvidenceIdentity, EvidenceProvider, FileHumanRequest,
    FileOversight, FileSupervisedDriver, FrozenAction, JournalError, Phase,
    current_input, policy_snapshot};
use super::super::super::{observe, sample, stage};
use super::super::super::evidence::FileEvidenceReport;
use super::super::super::super::source::FileSourceError;
use crate::action::ActionState;

pub(in super::super::super) fn policy_file_provider<'a, S: EvidenceFile + ?Sized>(
    expected: Option<(u64, u64)>, source: &'a mut S,
    observations: &'a mut Vec<Result<EvidenceIdentity, EvidenceError>>,
    updates: &'a mut Vec<Result<EvidenceIdentity, FileSourceError>>,
) -> impl EvidenceProvider + 'a {
    PolicyFile { expected, source, observations, updates }
}

struct PolicyFile<'a, S: ?Sized> {
    expected: Option<(u64, u64)>,
    source: &'a mut S,
    observations: &'a mut Vec<Result<EvidenceIdentity, EvidenceError>>,
    updates: &'a mut Vec<Result<EvidenceIdentity, FileSourceError>>,
}
impl<S: EvidenceFile + ?Sized> EvidenceProvider for PolicyFile<'_, S> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        if !host.policy_only_file_source_required() {
            // Preserve original legacy parsing, clock calls and source checks.
            // In particular a registered FULL-context source still refuses.
            let scope = host.profile.delivery.scope;
            let members: Vec<_> = host.profile.committee.members().keys().cloned().collect();
            let full = host.file_source_required();
            let snapshot = || policy_snapshot(self.source, scope, &members, full, self.observations);
            return CurrentLearned { expected: self.expected, snapshot }.capture(host, action, clock);
        }
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let Some((attempt, revision)) = self.expected else { return Ok(Err(Error::Incomplete)); };
        if let Err(error) = acquisition_input(host, attempt, revision, action) { return Ok(Err(error)); }
        // The same read-start convention, interruption latch, persisted version
        // floor, refusal and withdrawal behavior as ordinary durable evidence.
        let captured = host.refresh_file_source(host.revision(), self.source, clock());
        self.updates.push(captured.as_ref().map(|value| value.identity()).map_err(Clone::clone));
        let captured = match captured {
            Ok(value) => { self.observations.push(Ok(value.identity())); value }
            Err(FileSourceError::Refused(error)) => {
                self.observations.push(Err(EvidenceError::Data(error))); return Ok(Err(error));
            }
            Err(FileSourceError::Read { error, withdrawal }) => {
                self.observations.push(Err(error));
                return match withdrawal {
                    Some(error) => Err(error),
                    None => Ok(Err(match error { EvidenceError::Data(error) => error,
                        EvidenceError::Io(_) => Error::Incomplete })),
                };
            }
            Err(FileSourceError::Journal(error)) => return Err(error),
        };
        // A changed or refused source invalidates ORIGINAL input/key eligibility.
        // Do not reinstate the retained sidecar or translate file contexts into it.
        // The outer driver checks the post-read clock before any permitting work.
        Ok(current_input(host, attempt, revision, action).map(|input|
            DriverEvidence { snapshot: captured.snapshot().clone(), inputs: Some(input) }))
    }
}

fn acquisition_input(host: &FileOversight, attempt: u64, revision: u64,
    action: &FrozenAction) -> Result<(), Error>
{
    if host.source_interrupted || host.machine.pending_learned_reset().is_some() {
        return Err(Error::Incomplete);
    }
    let original = host.machine.checked_learned_sidecar_source(attempt)?;
    if original.input_revision() != revision { return Err(Error::Stale); }
    if original.round().input().action() != action { return Err(Error::Binding); }
    Ok(())
}

impl FileSupervisedDriver {
    /// Freeze the existing independent human request after a fresh policy read
    /// and the original post-read clock/current-input checks. This can renew an
    /// expired policy lease, never a withdrawn learned judgment. Source and
    /// request journal operations report separately; no human approval is issued.
    pub fn request_learned_human_approval_from_policy_file<S, F>(&mut self, source: &mut S,
        key: u64, expires_at: ElapsedTick, mut clock: F) -> FileEvidenceReport<FileHumanRequest>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.reap_helpers();
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        let result = (|| {
            let job = self.job.as_ref().ok_or(Error::Missing)?;
            let mut host = self.supervisor.host_mut()?;
            job.check_owner(&host)?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            if job.phase != Phase::Ready
                || !matches!(stage(&host, job.request)?, ActionState::Reviewing | ActionState::Authorized)
            { return Err(Error::WrongState.into()); }
            observe(&mut host, clock())?;
            let captured = sample(&mut host, job,
                &mut policy_file_provider(Some((job.attempt, job.input_revision)), source,
                    &mut observations, &mut source_updates), &mut clock)?;
            observe(&mut host, clock())?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            let inputs: &CommitteeInput = captured.evidence.inputs.as_ref().ok_or(Error::Incomplete)?;
            job.check_ready(&host, Some(inputs))?;
            let revision = host.revision();
            host.request_human_approval(revision, key, job.attempt, inputs, expires_at)
        })();
        self.reap_helpers();
        FileEvidenceReport { observations, source_updates, result }
    }
}
