//! Current original learned evidence at every existing publication boundary.
//! Callers supply policy snapshots, never helper-input bytes or a replacement
//! source. The original driver still owns both keys and final endpoint checks.
mod computed;
mod policy_file;
pub(in super::super) use policy_file::policy_file_provider;

use super::super::{FileSupervisedDriver, FileDriverEvent, FileHumanPermit,
    FileHumanRequest, FileCredentialPermit, FileOversight, FrozenAction,
    CommitteeInput, DriverEvidence, JournalError, Phase, provider::EvidenceProvider};
use crate::action::{ElapsedTick, Scope};
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceFile, EvidenceIdentity};
use crate::{Error, Snapshot};

impl FileSupervisedDriver {
    /// Same review/dispatch/publication/reconciliation state machine as the
    /// callback driver, but each evidence capture resolves the CURRENT original
    /// learned plan. The snapshot callback cannot replace its helper inputs.
    /// Recovery reconciliation needs neither a plan nor a snapshot callback.
    pub fn step_with_learned_evidence<F, P>(&mut self, clock: F, snapshot: P,
        human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        self.step_with_provider(clock, &mut CurrentLearned { expected, snapshot }, human, None)
    }

    /// A credential is consulted only by the original first-publication gate.
    /// Supplying it never replaces congress, human review or currentness checks.
    pub fn step_with_learned_evidence_and_credential<F, P>(&mut self, clock: F,
        snapshot: P, human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        self.step_with_provider(clock, &mut CurrentLearned { expected, snapshot }, human, Some(credential))
    }

    /// The ordinary request operation still samples the supplied current time
    /// and performs the original human-key checks. This resolves evidence only;
    /// the independent reviewer capability is not acquired or stored here.
    pub fn request_learned_human_approval(&mut self, key: u64,
        expires_at: ElapsedTick, now: ElapsedTick) -> Result<FileHumanRequest, JournalError>
    {
        let input = {
            let job = self.job.as_ref().ok_or(Error::Missing)?;
            if job.phase != Phase::Ready { return Err(Error::WrongState.into()); }
            let host = self.supervisor.host()?;
            job.check_owner(&host)?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            current_input(&host, job.attempt, job.input_revision, &job.action)?
        };
        self.request_human_approval(key, &input, expires_at, now)
    }
}

struct CurrentLearned<P> {
    expected: Option<(u64, u64)>,
    snapshot: P,
}
impl<P> EvidenceProvider for CurrentLearned<P>
where P: FnMut() -> Result<Snapshot, Error> {
    fn capture<F>(&mut self, host: &mut FileOversight, action: &FrozenAction, _clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let Some((attempt, revision)) = self.expected else { return Ok(Err(Error::Incomplete)); };
        // Resolve before callback entry, and again after it. The same concrete
        // machine owns source/plan checks; retained packet bytes cannot stand in
        // for a live source. The outer driver brackets capture with its clock.
        if let Err(error) = check_current(host, attempt, revision, action) { return Ok(Err(error)); }
        let snapshot = match (self.snapshot)() {
            Ok(snapshot) => snapshot,
            Err(error) => return Ok(Err(error)),
        };
        Ok(current_input(host, attempt, revision, action)
            .map(|input| DriverEvidence { snapshot, inputs: Some(input) }))
    }
}

fn check_current(host: &FileOversight, attempt: u64, revision: u64,
    action: &FrozenAction) -> Result<(), Error>
{
    // These live-owner latches are not part of a historical Machine projection.
    // In particular an interrupted reset may not reuse an older quiet packet.
    if host.source_interrupted || host.machine.pending_learned_reset().is_some() {
        return Err(Error::Incomplete);
    }
    let original = host.machine.checked_learned_sidecar(attempt)?;
    if original.input_revision() != revision { return Err(Error::Stale); }
    if original.round().input().action() != action { return Err(Error::Binding); }
    Ok(())
}
fn current_input(host: &FileOversight, attempt: u64, revision: u64,
    action: &FrozenAction) -> Result<CommitteeInput, Error>
{
    check_current(host, attempt, revision, action)?;
    Ok(host.machine.checked_learned_sidecar(attempt)?.round().input().clone())
}

// Original concrete evidence, shared only inside the supervising driver. This
// does not open a public provider extension or export a mutable learned source.
pub(in super::super) fn current_provider<P>(expected: Option<(u64, u64)>, snapshot: P)
    -> impl EvidenceProvider
where P: FnMut() -> Result<Snapshot, Error> {
    CurrentLearned { expected, snapshot }
}

// The SAME policy-only file admission for probe and native-model congress. All
// nonempty helper context is refused, never omitted from the actual judged input.
pub(in super::super) fn policy_snapshot<S: EvidenceFile + ?Sized>(source: &mut S,
    scope: Scope, members: &[String], full_context_source: bool,
    observations: &mut Vec<Result<EvidenceIdentity, EvidenceError>>) -> Result<Snapshot, Error>
{
    if full_context_source { return Err(Error::Binding); }
    let captured = source.read_evidence();
    let result = captured.and_then(|captured| {
        if captured.identity().scope != scope
            || !captured.contexts().keys().eq(members.iter())
            || captured.contexts().values().any(|context| !context.is_empty()) {
            return Err(EvidenceError::Data(Error::Binding));
        }
        if !captured.snapshot().complete { return Err(EvidenceError::Data(Error::Incomplete)); }
        Ok(captured)
    });
    observations.push(result.as_ref().map(|capture| capture.identity()).map_err(|error| *error));
    result.map(|capture| capture.snapshot().clone()).map_err(|error| match error {
        EvidenceError::Data(error) => error, EvidenceError::Io(_) => Error::Incomplete,
    })
}
