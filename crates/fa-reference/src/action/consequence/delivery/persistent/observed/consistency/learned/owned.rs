//! Durable forecast intents whose source is derived by the original generator.
//! Replay reconstructs original token/audit history; saved results only compare.
use super::{Error, Event, FileConsistencyObserver, FileLearnedConsistencyConfig,
    FileLearnedConsistencySnapshot, FileLearnedPublicationSnapshot, FileOversight,
    JournalError, LearnedForecastReport, Reader, Rc, Transition, ConsistencyEvent, outcome};
use super::super::super::{BaseEvent, FileHumanReviewer, FileOversightProfile, Machine, journal, storage};
use super::super::super::decoder::learned::{FileLearnedConfig, bind_history};
use crate::action::consequence::oversight::OversightBroker;
use std::collections::BTreeMap;
use std::path::Path;

impl FileOversight {
    pub fn owned_learned_action_consistency_required(&self) -> bool {
        self.machine.broker.owned_learned_consistency_required()
    }

    /// Bind BOTH independently retained configurations before replay, cleanup or
    /// new role issuance. Forecast source objects are rebuilt by original token
    /// execution and source checking, not supplied in a capture map or decoded
    /// from archived fidelity claims. The original recovery fence remains final.
    pub fn open_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: FileOversightProfile, generation: &FileLearnedConfig,
        predictor: &FileLearnedConsistencyConfig)
        -> Result<(Self, FileHumanReviewer, FileConsistencyObserver), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let canonical = store.read(profile.delivery.limits.bytes)?;
        let mut events = journal::decode(&profile, store.identity(), &canonical)?;
        bind_owned_history(&mut events, generation, predictor)?;
        let machine = Machine::replay(&profile, &events)?;
        if store.read(profile.delivery.limits.bytes)? != canonical { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let observer = FileConsistencyObserver { issuer: Rc::clone(&host.issuer) };
        Ok((host, reviewer, observer))
    }

    /// A single canonical image supplies publication, likelihood, accounting and
    /// pending-request identity. Historical inspection neither acquires the writer
    /// nor fences, resumes numerical work, refreshes policy or grants any key.
    /// Original replay DOES repeat physical inference, compression and monitoring.
    pub fn read_publication_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, generation: &FileLearnedConfig,
        predictor: &FileLearnedConsistencyConfig) -> Result<FileLearnedPublicationSnapshot, JournalError>
    {
        super::super::super::super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        bind_owned_history(&mut events, generation, predictor)?;
        let machine = Machine::replay(profile, &events)?;
        Ok(FileLearnedPublicationSnapshot {
            publication: machine.snapshot(events.len()),
            consistency: FileLearnedConsistencySnapshot {
                consistency: machine.consistency_snapshot(events.len() as u64)?,
                work: machine.broker.learned_consistency_work()?,
                retained_source_bytes: machine.broker.learned_consistency_retained_source_bytes()?,
                has_unreported_work: machine.broker.learned_consistency_has_unreported_work()?,
            },
            pending_request: machine.consistency_request.map(|(request, _)| request),
        })
    }
}

impl FileConsistencyObserver {
    /// The observer selects only an attempt and current actor revision. The
    /// exact owned audit/row is selected inside the original broker, on live
    /// execution AND replay. Return levels match the supplied-source path:
    /// unacknowledged JournalError, acknowledged native Error, or an acknowledged
    /// report whose prediction() may still refuse. No speculative report escapes.
    pub fn forecast_owned_learned_action(&self, host: &mut FileOversight, revision: u64,
        attempt: u64, actor_revision: u64) -> Result<Result<LearnedForecastReport, Error>, JournalError>
    {
        self.check_owned_forecast_owner(host, revision)?;
        match host.transact(revision, Event::Consistency(
            ConsistencyEvent::ForecastOwnedLearned(attempt, actor_revision, None)))? {
            Transition::LearnedConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("original owned learned forecast"),
        }
    }

    /// Bind the original external request allocator before accepting its payload.
    /// Neither a refused report nor a storage failure grants a pending request.
    pub fn forecast_owned_learned_request(&self, host: &mut FileOversight, revision: u64,
        request: u64, actor_revision: u64) -> Result<Result<LearnedForecastReport, Error>, JournalError>
    {
        self.check_owned_forecast_owner(host, revision)?;
        match host.transact(revision, Event::Consistency(
            ConsistencyEvent::ForecastOwnedLearnedRequest(request, actor_revision, None)))? {
            Transition::LearnedConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("original keyed owned learned forecast"),
        }
    }

    fn check_owned_forecast_owner(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != host.revision() { return Err(Error::Stale.into()); }
        if !host.owned_learned_action_consistency_required() { return Err(Error::Binding.into()); }
        Ok(())
    }
}

pub(in super::super::super) fn read_witness(r: &mut Reader<'_>) -> Result<Option<Rc<[u8]>>, Error> {
    let bytes = r.blob(outcome::MAX_BYTES)?;
    if bytes.is_empty() { return Err(Error::Incomplete); }
    Ok(Some(Rc::from(bytes)))
}

pub(in super::super::super) fn verify_witness(saved: Option<&[u8]>,
    result: &Result<LearnedForecastReport, Error>, broker: &OversightBroker) -> Result<(), Error>
{
    if let Some(saved) = saved {
        if outcome::witness(result, broker)?.as_slice() != saved { return Err(Error::Binding); }
    }
    Ok(())
}

fn bind_owned_history(events: &mut [Event], generation: &FileLearnedConfig,
    predictor: &FileLearnedConsistencyConfig) -> Result<(), Error>
{
    if !predictor.uses_owned_generation() { return Err(Error::Binding); }
    // Reject all supplied-capture records. There is no archived-source import or
    // alternate hydration route for this mode. Both comparisons precede replay.
    super::recovery::bind_events(events, predictor, &BTreeMap::new())?;
    bind_history(events, generation)
}

#[cfg(test)]
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationEvent;
#[cfg(test)]
mod tests;
