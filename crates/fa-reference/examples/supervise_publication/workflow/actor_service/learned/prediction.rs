//! Supervisor custody for one original request, outside the source-only actor.
//! The finite reference registration is loaded independently, not calibrated by
//! this driver. Native raw and owned predictions keep their distinct evidence.
use super::{FileOversight, GenerationStatus, GenerationStop, debug};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyObserver,
    pre_output::{FilePreOutputStepper, FileOwnedPreOutputStepper, FilePreOutputQuantum,
        FileOwnedPreOutputStart},
};

// This is a historical reducer-call quantum, not a time/CPU guarantee.
// The ordinary stop, deadline and policy-source loop runs after EVERY quantum.
pub(super) const MAX_REPLAY_EVENTS_PER_QUANTUM: usize = 32;

enum Message {
    Raw(Box<FilePreOutputStepper>),
    Owned(Box<FileOwnedPreOutputStepper>),
}

/// Custody is not returned to the actor, cloned, or reconstructed from a saved
/// pending forecast. Recovery must finish the original independently guarded
/// replay before issuing a new observer; it preserves any lost coverage.
pub(super) struct Session {
    request: u64,
    observer: FileConsistencyObserver,
    message: Option<Message>,
}

pub(super) fn check(host: &FileOversight, session: Option<&Session>) -> Result<(), String> {
    if host.action_consistency_required() != session.is_some() {
        return Err("required learned forecast and separately held observer custody do not match".into());
    }
    if session.is_some() {
        let evidence = host.action_consistency_snapshot().map_err(debug)?;
        if evidence.coverage_lost || evidence.evidence.crossed() {
            return Err("required learned forecast has lost coverage or crossed its lifetime evidence boundary".into());
        }
    }
    Ok(())
}

impl Session {
    pub(super) fn new(request: u64, observer: FileConsistencyObserver) -> Self {
        Self { request, observer, message: None }
    }

    pub(super) fn has_message(&self) -> bool { self.message.is_some() }

    /// Select a new independently provisioned request only after this message's
    /// original forecast was consumed and its publication has an Executed
    /// receipt. Keep the SAME observer and original lifetime evidence owner.
    pub(super) fn select_request(&mut self, host: &FileOversight, request: u64) -> Result<(), String> {
        use fa_reference::action::ActionState;
        use fa_reference::action::consequence::delivery::{EndpointOutcome,
            persistent::{JournalError, requests::FileRequestDisposition}};
        use fa_reference::Error;
        if request == 0 || request == self.request {
            return Err("forecast successor requires a distinct nonzero request".into());
        }
        self.complete_message(host)?;
        let FileRequestDisposition::Admitted { attempt, stage: ActionState::Confirmed, .. } =
            host.request_status(self.request).map_err(debug)?.disposition
            else { return Err("forecast predecessor has no confirmed request".into()); };
        if !matches!(host.request_resolution(self.request).map_err(debug)?,
            Some(EndpointOutcome::Executed { .. }))
            || host.action_consistency_snapshot().map_err(debug)?.pending_attempt.is_some() {
            return Err("forecast successor requires a consumed prediction and Executed receipt".into());
        }
        // Each predictor retains its own native consumed record. An owned
        // learned observation cannot be relabeled as a raw forecast.
        if host.owned_learned_action_consistency_required() {
            host.learned_action_consistency_observation(attempt).map_err(debug)?;
        } else {
            host.action_consistency_observation(attempt).map_err(debug)?;
        }
        match host.request_status(request) {
            Err(JournalError::Contract(Error::Missing)) => {},
            Ok(_) => return Err("forecast successor request is already recorded".into()),
            Err(error) => return Err(debug(error)),
        }
        self.request = request;
        self.message = None;
        Ok(())
    }

    /// The original fixed prompt completes first. The library admits only its
    /// original output boundary or this request's retained finite successor.
    /// A late, wrong-request, or unanswered forecast cannot acquire a handle.
    pub(super) fn begin_message(&mut self, host: &mut FileOversight) -> Result<(), String> {
        check(host, Some(self))?;
        if self.message.is_some() { return Err("learned request already has its original forecast".into()); }
        let current = host.learned_generation_inspection().map_err(debug)?.numerical;
        let revision = host.revision();
        let message = if host.owned_learned_action_consistency_required() {
            match self.observer.begin_owned_pre_output_request(host, revision,
                self.request, current.actor_revision, current.position).map_err(debug)? {
                FileOwnedPreOutputStart::Ready(handle) => Message::Owned(Box::new((*handle).into_cooperative())),
                FileOwnedPreOutputStart::Inconclusive(report) => {
                    return Err(format!("original owned learned forecast is inconclusive: {report:?}"));
                }
                FileOwnedPreOutputStart::Refused(error) => return Err(debug(error)),
            }
        } else {
            Message::Raw(Box::new(self.observer.begin_pre_output_request(host, revision,
                self.request, current.actor_revision, current.position).map_err(debug)?.map_err(debug)?.into_cooperative()))
        };
        self.message = Some(message);
        Ok(())
    }

    /// One bounded original replay slice, or one original intent/completion.
    /// Return to the normal stop/deadline/source loop even when no token is ready.
    /// The original forecast checks its full numerical predecessor and pending
    /// request before replay and again before either durable finalization.
    pub(super) fn advance_quantum(&mut self, host: &mut FileOversight) -> Result<(), String> {
        if let FilePreOutputQuantum::Completed { result, .. } = self.quantum(host)? {
            result.map_err(debug)?;
        }
        Ok(())
    }

    fn quantum(&mut self, host: &mut FileOversight) -> Result<FilePreOutputQuantum, String> {
        check(host, Some(self))?;
        let revision = host.revision();
        match self.message.as_mut().ok_or("required learned message forecast is missing")? {
            Message::Raw(handle) => handle.advance_quantum(host, revision, MAX_REPLAY_EVENTS_PER_QUANTUM),
            Message::Owned(handle) => handle.advance_quantum(host, revision, MAX_REPLAY_EVENTS_PER_QUANTUM),
        }.map_err(debug)
    }

    // Preserve the existing tests' one-call/one-sample contract without placing
    // a draining loop in the production operator. The actual command uses only
    // advance_quantum; every existing per-sample assertion remains unchanged.
    #[cfg(test)]
    pub(super) fn advance(&mut self, host: &mut FileOversight) -> Result<(), String> {
        loop {
            if let FilePreOutputQuantum::Completed { result, .. } = self.quantum(host)? {
                return result.map(|_| ()).map_err(debug);
            }
        }
    }

    pub(super) fn complete_message(&self, host: &FileOversight) -> Result<(), String> {
        check(host, Some(self))?;
        let current = host.learned_generation_inspection().map_err(debug)?.numerical;
        let original = match self.message.as_ref().ok_or("completed learned output has no current request forecast")? {
            Message::Raw(handle) => handle.numerical(),
            Message::Owned(handle) => handle.numerical(),
        };
        if &current != original { return Err("learned output no longer matches its original forecast continuation".into()); }
        Ok(())
    }

    /// Finish is its own forecast at the confirmed message cut. No generation
    /// handle, numerical operation, text replay, or recycled message observation
    /// participates. Original admission still classifies and consumes it once.
    pub(super) fn finish_request(&mut self, host: &mut FileOversight) -> Result<(), String> {
        check(host, Some(self))?;
        if self.message.is_some() { return Err("a message forecast cannot be reused for stream finish".into()); }
        let current = host.learned_generation_inspection().map_err(debug)?.numerical;
        if !matches!(current.status, GenerationStatus::Finished(GenerationStop::StopToken(_))) {
            return Err("learned finish forecast requires the original completed message boundary".into());
        }
        let revision = host.revision();
        if host.owned_learned_action_consistency_required() {
            let report = self.observer.forecast_owned_learned_request(host, revision,
                self.request, current.actor_revision).map_err(debug)?.map_err(debug)?;
            report.prediction().map_err(|error|
                format!("original owned learned finish forecast is inconclusive: {error:?}; {report:?}"))?;
        } else {
            self.observer.forecast_hosted_request(host, revision,
                self.request, current.actor_revision).map_err(debug)?.map_err(debug)?;
        }
        Ok(())
    }
}
