//! Durable pre-action prediction and the ORIGINAL exact lifetime evidence gate.
//! Prediction agreement never replaces congress, human approval or publication checks.
mod config;
mod codec;
mod requests;
mod hosted;
mod deadline;
pub use config::{FileConsistencyConfig, FileConsistencyParameters};
pub(super) use codec::{read, write};

use super::{BaseEvent, Event, FileOversight, JournalError, Transition};
use crate::action::{ActionSpec, FrozenAction};
use crate::action::consequence::activation::SourceFrame;
use crate::action::consequence::activation::consistency::{LikelihoodEvidence, Prediction};
use crate::action::consequence::oversight::consistency::ConsistencyObservation;
use crate::{Error, Snapshot};
use std::rc::Rc;

#[derive(Clone)]
pub(super) enum ConsistencyEvent {
    Enable(Rc<FileConsistencyConfig>),
    Forecast(u64, u64, SourceFrame),
    ForecastRequest(u64, u64, SourceFrame),
    ForecastHosted(u64, u64),
    ForecastHostedRequest(u64, u64),
    Unavailable,
    Expire(crate::action::consequence::oversight::consistency::ConsistencyDeadline, crate::action::ElapsedTick),
}

/// Acknowledged supervisor evidence, not an effect permit or proof of calibration.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencySnapshot;
/// fn grant(snapshot: FileConsistencySnapshot) -> FilePermit { snapshot }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileConsistencySnapshot {
    pub journal_revision: u64,
    pub evidence: LikelihoodEvidence,
    pub pending_attempt: Option<u64>,
    pub coverage_lost: bool,
}

/// Custody is separate from actor access, not cryptographic observer identity.
/// No public constructor, cloning or live-owner role getter is provided.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
/// fn duplicate(role: FileConsistencyObserver) { let _ = role.clone(); }
/// ```
#[derive(Debug)]
pub struct FileConsistencyObserver { pub(super) issuer: Rc<()> }

impl FileOversight {
    pub fn enable_action_consistency(&mut self, revision: u64, config: FileConsistencyConfig)
        -> Result<FileConsistencyObserver, JournalError>
    {
        self.transact(revision, Event::Consistency(ConsistencyEvent::Enable(Rc::new(config))))?;
        Ok(FileConsistencyObserver { issuer: Rc::clone(&self.issuer) })
    }
    pub fn action_consistency_required(&self) -> bool { self.machine.consistency.is_some() }
    pub fn action_consistency_snapshot(&self) -> Result<FileConsistencySnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.consistency_snapshot(self.revision())?)
    }
    pub fn action_consistency_observation(&self, attempt: u64) -> Result<&ConsistencyObservation, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.consistency_observation(attempt)?)
    }

    /// Outer Err is unacknowledged. Inner Err is the native proposal refusal,
    /// committed together with any consumed forecast and likelihood factor.
    /// Ordinary propose also commits native refusals in this opt-in profile,
    /// then maps the inner refusal to JournalError::Contract. This explicit API
    /// distinguishes that case from an unacknowledged outer storage failure.
    /// External-key submit_request retains the native refusal in its request book.
    pub fn propose_consistent(&mut self, revision: u64, attempt: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<Result<FrozenAction, Error>, JournalError>
    {
        if !self.action_consistency_required() { return Err(Error::Incomplete.into()); }
        match self.transact(revision, Event::Core(BaseEvent::Propose(attempt, spec, snapshot)))? {
            Transition::ConsistencyProposed(result) => Ok(result),
            _ => unreachable!("consistency proposal transition"),
        }
    }
}
impl FileConsistencyObserver {
    /// The original predictor consumes exact captured values; no caller-supplied
    /// probability or matched Boolean is accepted. Source origin is still trusted.
    pub fn forecast_action(&self, host: &mut FileOversight, revision: u64, attempt: u64,
        actor_revision: u64, frame: &SourceFrame) -> Result<Result<Prediction, Error>, JournalError>
    {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        match host.transact(revision, Event::Consistency(ConsistencyEvent::Forecast(attempt, actor_revision, frame.clone())))? {
            Transition::ConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("consistency prediction transition"),
        }
    }

    /// A real missing capture permanently closes this finite profile. No
    /// subsequent recapture, checkpoint reset or recovery can re-arm its budget.
    pub fn unavailable(&self, host: &mut FileOversight, revision: u64) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        host.transact(revision, Event::Consistency(ConsistencyEvent::Unavailable))?;
        Ok(())
    }
}

#[cfg(test)]
mod storage_tests;
