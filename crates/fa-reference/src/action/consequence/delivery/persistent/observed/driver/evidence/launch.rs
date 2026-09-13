//! Review launch captures through the same live source gate as later steps.
use super::{FileReviewLaunch, FileSourceReviewError, durable_capture};
use super::super::{FileDriverLaunch, FileSupervisedDriver, admitted};
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::oversight::{CommitteeInput, evidence_source::{EvidenceFile, EvidenceIdentity, EvidenceSnapshot}};
use crate::action::consequence::oversight::helper_processes::HelperProgram;
use crate::action::consequence::delivery::persistent::JournalError;
use crate::Error;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

struct PreparedInput {
    capture: Rc<EvidenceSnapshot>,
    inputs: CommitteeInput,
    input_revision: u64,
}

impl FileSupervisedDriver {
    /// Validate the caller's exact predecessor BEFORE reading. With a registered
    /// source, persist the actual read through the native gate before launching
    /// workers. A changed observation can withdraw previous inputs; only revisions
    /// caused by THIS exclusively owned capture are adopted for the new review.
    /// No previously completed review, approval or reservation is reused here.
    pub fn start_file_review<S, F>(&mut self, source: &mut S,
        launch: FileReviewLaunch<UnixStream>, mut clock: F) -> Result<EvidenceIdentity, FileSourceReviewError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let prepared = self.file_review_inputs(source, &launch, &mut clock)?;
        let observation = prepared.capture.identity();
        self.start_review(FileDriverLaunch {
            request: launch.request, round: launch.round, evidence_root: prepared.capture.reference_root(),
            window: launch.window, expected_input_revision: prepared.input_revision,
            inputs: prepared.inputs, workers: launch.workers, limits: launch.limits,
        }, prepared.capture.snapshot().clone(), clock)
            .map_err(|error| FileSourceReviewError::Sockets { observation, error })?;
        Ok(observation)
    }

    /// Capture and commit before executable-helper admission or process creation.
    /// The original launcher retains every started child on partial failure and
    /// checks fresh time after spawn. No source read failure can launch a helper.
    pub fn start_file_process_review<S, F>(&mut self, source: &mut S,
        launch: FileReviewLaunch<HelperProgram>, mut clock: F) -> Result<EvidenceIdentity, FileSourceReviewError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        let prepared = self.file_review_inputs(source, &launch, &mut clock)?;
        let observation = prepared.capture.identity();
        self.start_process_review(FileDriverLaunch {
            request: launch.request, round: launch.round, evidence_root: prepared.capture.reference_root(),
            window: launch.window, expected_input_revision: prepared.input_revision,
            inputs: prepared.inputs, workers: launch.workers, limits: launch.limits,
        }, prepared.capture.snapshot().clone(), clock)
            .map_err(|error| FileSourceReviewError::Processes { observation, error })?;
        Ok(observation)
    }

    fn file_review_inputs<S, W, F>(&mut self, source: &mut S, launch: &FileReviewLaunch<W>, clock: &mut F)
        -> Result<PreparedInput, FileSourceReviewError>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        if self.job.is_some() { return Err(Error::WrongState.into()); }
        self.ensure_child_slot()?;
        let mut host = self.supervisor.host_mut()?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable.into()); }
        let (attempt, state) = admitted(host.request_status(launch.request)?)?;
        if state != ActionState::Reviewing { return Err(Error::WrongState.into()); }
        if host.input_revision(attempt)? != launch.expected_input_revision { return Err(Error::Stale.into()); }
        if !launch.workers.keys().eq(host.profile.committee.members().keys()) { return Err(Error::Binding.into()); }
        let action = host.request_action(launch.request)?.clone();
        // Retain the exclusive original owner across the read. A stale caller
        // cannot hide behind a subsequent source-driven input revision change.
        let captured = if host.file_source_required() {
            Ok(durable_capture(&mut host, source, clock).map_err(FileSourceReviewError::DurableSource)?)
        } else {
            // The explicit legacy profile preserves its original clock calls.
            source.read_evidence()
        };
        let captured = captured.and_then(|capture| {
            if !capture.snapshot().complete { return Err(Error::Incomplete.into()); }
            let inputs = capture.inputs_for(&action, &host.profile.committee)?;
            Ok((capture, inputs))
        });
        match captured {
            Ok((capture, inputs)) => Ok(PreparedInput {
                capture, inputs, input_revision: host.input_revision(attempt)?,
            }),
            Err(error) => {
                let revision = host.revision();
                let expected = host.input_revision(attempt)?;
                let withdrawal = host.inputs_unavailable(revision, attempt, expected).err();
                Err(FileSourceReviewError::Source { error, withdrawal })
            }
        }
    }
}
