//! Durable supplied-source learned forecasts in the ORIGINAL two-key owner.
//! Saved bytes are equality witnesses, NEVER imported learned bounds or state.
//! Explicit recovery supplies independently source-checked captures, then reruns
//! the original predictor, lifetime accounting, proposal observations and fence.
mod config;
mod recovery;
mod outcome;
pub(in super::super) use outcome::complete_event;
pub use config::FileLearnedConsistencyConfig;
pub use recovery::FileLearnedConsistencyRequirement;

use super::{ConsistencyEvent, FileConsistencyObserver, FileConsistencySnapshot};
use super::super::{Event, FileOversight, JournalError, Transition};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::consistency::learned::LearnedForecastReport;
use crate::action::consequence::activation::monitor::learned::{LearnedMonitorBudget, LearnedMonitorWork};
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvRow, MAX_CHECKED_KV_BYTES};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::oversight::consistency::LearnedConsistencyObservation;
use crate::Error;
use std::rc::Rc;

#[derive(Clone)]
pub(in super::super) struct Configuration {
    bytes: Rc<[u8]>,
    runtime: Option<Rc<FileLearnedConsistencyConfig>>,
}
impl Configuration {
    pub(super) fn new(config: FileLearnedConsistencyConfig) -> Self {
        Self { bytes: Rc::from(config.encoded()), runtime: Some(Rc::new(config)) }
    }
    pub(in super::super) fn runtime(&self) -> Result<&FileLearnedConsistencyConfig, Error> {
        let config = self.runtime.as_ref().ok_or(Error::Incomplete)?;
        if config.encoded() != self.bytes.as_ref() { return Err(Error::Binding); }
        Ok(config)
    }
    pub(in super::super) fn write(&self, w: &mut Writer) -> Result<(), Error> { w.blob(&self.bytes) }
    pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<Self, Error> {
        let bytes = r.blob(config::MAX_CONFIG_BYTES)?;
        FileLearnedConsistencyConfig::decode(bytes)?;
        Ok(Self { bytes: Rc::from(bytes), runtime: None })
    }
}

/// No parser can manufacture CheckedLearnedKv. The private runtime is present
/// only after live typed capture or exact comparison to independent recovery input.
#[derive(Clone)]
pub(in super::super) struct Capture {
    bytes: Rc<[u8]>,
    runtime: Option<CheckedLearnedKv>,
    outcome: Option<Rc<[u8]>>,
}
impl Capture {
    fn new(source: &CheckedLearnedKv) -> Result<Self, Error> {
        Ok(Self { bytes: Rc::from(source.encode()?), runtime: Some(source.clone()), outcome: None })
    }
    pub(in super::super) fn runtime(&self) -> Result<&CheckedLearnedKv, Error> {
        self.runtime.as_ref().ok_or(Error::Incomplete)
    }
    pub(in super::super) fn write(&self, w: &mut Writer) -> Result<(), Error> {
        w.blob(&self.bytes)?; w.blob(self.outcome.as_deref().unwrap_or(&[]))
    }
    pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<Self, Error> {
        let bytes = r.blob(MAX_CHECKED_KV_BYTES)?;
        let outcome = r.blob(outcome::MAX_BYTES)?;
        if bytes.is_empty() || outcome.is_empty() { return Err(Error::Incomplete); }
        Ok(Self { bytes: Rc::from(bytes), runtime: None, outcome: Some(Rc::from(outcome)) })
    }
    pub(in super::super) fn verify_outcome(&self, result: &Result<LearnedForecastReport, Error>,
        broker: &crate::action::consequence::oversight::OversightBroker) -> Result<(), Error>
    {
        if let Some(saved) = &self.outcome {
            if outcome::witness(result, broker)?.as_slice() != saved.as_ref() { return Err(Error::Binding); }
        }
        Ok(())
    }
}

/// Logical acquisition costs for the inspected cut, not physical replay or RSS.
/// The live accessor exposes only acknowledged cuts. Historical reads can inspect
/// a visible replacement whose acknowledgment was lost. Original fitting, source
/// checking and repeated replay are additional costs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedConsistencySnapshot {
    pub consistency: FileConsistencySnapshot,
    pub work: LearnedMonitorWork,
    pub retained_source_bytes: usize,
    pub has_unreported_work: bool,
}
impl FileOversight {
    pub fn enable_learned_action_consistency(&mut self, revision: u64,
        config: FileLearnedConsistencyConfig) -> Result<FileConsistencyObserver, JournalError>
    {
        self.transact(revision, Event::Consistency(ConsistencyEvent::EnableLearned(Configuration::new(config))))?;
        Ok(FileConsistencyObserver { issuer: Rc::clone(&self.issuer) })
    }
    pub fn learned_action_consistency_required(&self) -> bool {
        self.machine.broker.learned_action_consistency_required()
    }
    pub fn learned_action_consistency_snapshot(&self) -> Result<FileLearnedConsistencySnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileLearnedConsistencySnapshot { consistency: self.action_consistency_snapshot()?,
            work: self.machine.broker.learned_consistency_work()?,
            retained_source_bytes: self.machine.broker.learned_consistency_retained_source_bytes()?,
            has_unreported_work: self.machine.broker.learned_consistency_has_unreported_work()? })
    }
    pub fn learned_action_consistency_report(&self, attempt: u64) -> Result<&LearnedForecastReport, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.learned_consistency_report(attempt)?)
    }
    pub fn learned_action_consistency_observation(&self, attempt: u64)
        -> Result<&LearnedConsistencyObservation, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.learned_consistency_observation(attempt)?)
    }
}
impl FileConsistencyObserver {
    /// Outer Err is unacknowledged: no speculative report escapes storage failure.
    /// Inner Err is an acknowledged native error. An acknowledged report can ALSO
    /// refuse prediction; its actual partial work and coverage loss remain durable.
    pub fn forecast_learned_action(&self, host: &mut FileOversight, revision: u64,
        attempt: u64, actor_revision: u64, source: &CheckedLearnedKv, row: KvRow)
        -> Result<Result<LearnedForecastReport, Error>, JournalError>
    {
        self.check_learned_owner(host, revision)?;
        let event = ConsistencyEvent::ForecastLearned(attempt, actor_revision, row, Capture::new(source)?);
        match host.transact(revision, Event::Consistency(event))? {
            Transition::LearnedConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("learned consistency forecast transition"),
        }
    }

    /// Bind to the ORIGINAL next-attempt allocator without granting effect rights.
    /// A report that refuses certification never establishes a request binding.
    pub fn forecast_learned_request(&self, host: &mut FileOversight, revision: u64,
        request: u64, actor_revision: u64, source: &CheckedLearnedKv, row: KvRow)
        -> Result<Result<LearnedForecastReport, Error>, JournalError>
    {
        self.check_learned_owner(host, revision)?;
        let event = ConsistencyEvent::ForecastLearnedRequest(request, actor_revision, row, Capture::new(source)?);
        match host.transact(revision, Event::Consistency(event))? {
            Transition::LearnedConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("learned request forecast transition"),
        }
    }
    fn check_learned_owner(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != host.revision() { return Err(Error::Stale.into()); }
        if !host.learned_action_consistency_required() { return Err(Error::Binding.into()); }
        Ok(())
    }
}

pub(in super::super) fn write_row(w: &mut Writer, row: KvRow) -> Result<(), Error> {
    w.u64(row.layer)?; write_side(w, row.side)?; w.u64(row.position)
}
pub(in super::super) fn read_row(r: &mut Reader<'_>) -> Result<KvRow, Error> {
    Ok(KvRow { layer: r.u64()?, side: read_side(r)?, position: r.u64()? })
}
fn write_side(w: &mut Writer, side: KvSide) -> Result<(), Error> {
    w.u8(match side { KvSide::Key => 0, KvSide::Value => 1 })
}
fn read_side(r: &mut Reader<'_>) -> Result<KvSide, Error> {
    match r.u8()? { 0 => Ok(KvSide::Key), 1 => Ok(KvSide::Value), _ => Err(Error::InvalidInput) }
}

#[cfg(test)]
mod tests;
