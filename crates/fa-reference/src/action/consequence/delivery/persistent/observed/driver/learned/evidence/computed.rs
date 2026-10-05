//! Original-source driving: policy callbacks/files cannot replace helper input.
use super::{CurrentLearned, policy_snapshot};
use super::super::super::{FileDriverEvent, FileHumanPermit, FileSupervisedDriver,
    FileCredentialPermit, JournalError};
use super::super::super::evidence::FileEvidenceReport;
use crate::action::consequence::oversight::evidence_source::EvidenceFile;
use crate::action::ElapsedTick;
use crate::{Error, Snapshot};

impl FileSupervisedDriver {
    /// Complete computed congress and publication without caller-supplied helper
    /// packets. Resolve the actual original source at each capture, including
    /// every refinement and both captures around automatic reservation. The
    /// snapshot callback supplies deterministic-policy observations only.
    pub fn step_computed<F, P>(&mut self, clock: F, snapshot: P,
        human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        self.step_computed_with_provider(clock, &mut CurrentLearned { expected, snapshot }, human, None)
    }

    /// Same original-source loop with the existing first-publication credential.
    /// The credential supplies no helper vote, current evidence or human key.
    pub fn step_computed_with_credential<F, P>(&mut self, clock: F, snapshot: P,
        human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: FnMut() -> Result<Snapshot, Error> {
        let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
        self.step_computed_with_provider(clock, &mut CurrentLearned { expected, snapshot }, human, Some(credential))
    }

    /// Reread an operator-selected POLICY-ONLY file at every original observation
    /// boundary. The source must carry exactly the committee's member names with
    /// EMPTY contexts: nonempty helper content must use a full-context profile,
    /// never be silently discarded while numerical helpers judge different input.
    /// Policy values feed the original deterministic gate, NOT the helpers.
    ///
    /// The configured durable full-context file-source profile is intentionally
    /// incompatible. This adapter cannot override its original input-equality
    /// contract. It uses the sealed reader's live version floor; it introduces no
    /// durable producer floor, authentication, provider event log or new gate.
    /// Current learned source checks and all original effect keys still apply.
    pub fn step_computed_from_policy_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>) -> FileEvidenceReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_computed_policy_file(source, clock, human, None)
    }

    /// Policy-only file observations with the SAME original credential guard.
    pub fn step_computed_from_policy_file_with_credential<S, F>(&mut self,
        source: &mut S, clock: F, human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> FileEvidenceReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.step_computed_policy_file(source, clock, human, Some(credential))
    }

    fn step_computed_policy_file<S, F>(&mut self, source: &mut S, clock: F,
        human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> FileEvidenceReport<FileDriverEvent>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let mut observations = Vec::with_capacity(2);
        let result = (|| {
            let (scope, members, full_context_source) = {
                let host = self.supervisor.host()?;
                (host.profile.delivery.scope, host.profile.committee.members().keys().cloned().collect::<Vec<_>>(),
                    host.file_source_required())
            };
            let expected = self.job.as_ref().map(|job| (job.attempt, job.input_revision));
            let snapshot = || policy_snapshot(source, scope, &members, full_context_source, &mut observations);
            // CurrentLearned resolves before AND after the actual file read;
            // no filename, returned context or journal observation installs a
            // learned source. Reconciliation never invokes this closure at all.
            self.step_computed_with_provider(clock, &mut CurrentLearned { expected, snapshot }, human, credential)
        })();
        // There was no durable full-context source refresh. Do not mislabel this
        // policy read as an acknowledged producer-floor update in the journal.
        FileEvidenceReport { observations, source_updates: Vec::new(), result }
    }
}
