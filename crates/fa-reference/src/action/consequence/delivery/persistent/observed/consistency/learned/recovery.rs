//! Bind exact independent learned inputs BEFORE numerical replay or disk cleanup.
use super::*;
use super::super::super::{BaseEvent, FileHumanReviewer, FileOversightProfile, Machine, journal, storage};
use std::collections::BTreeMap;
use std::path::Path;

/// Historical input requirements, not a replayed judgment or source certificate.
/// `journal_revision` is the ONE-BASED revision that recorded this forecast,
/// including refused calls. Attempt IDs alone are not unique acquisition IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedConsistencyRequirement {
    pub journal_revision: u64,
    pub attempt_or_request: u64,
    pub external_request: bool,
    pub actor_revision: u64,
    pub row: KvRow,
    pub encoded_source_bytes: usize,
}

impl FileOversight {
    /// Inspect only the framed historical input inventory. This does not rerun a
    /// predictor, certify an observation, expose a role, or modify the directory.
    pub fn learned_consistency_recovery_requirements(directory: impl AsRef<Path>,
        profile: &FileOversightProfile) -> Result<Vec<FileLearnedConsistencyRequirement>, JournalError>
    {
        let events = read_events(directory.as_ref(), profile)?;
        let mut requirements = Vec::new();
        for (index, event) in events.iter().enumerate() {
            let (key, revision, row, source, external_request) = match event {
                Event::Consistency(ConsistencyEvent::ForecastLearned(key, revision, row, source)) =>
                    (key, revision, row, source, false),
                Event::Consistency(ConsistencyEvent::ForecastLearnedRequest(key, revision, row, source)) =>
                    (key, revision, row, source, true),
                _ => continue,
            };
            requirements.try_reserve(1).map_err(|_| Error::Limit)?;
            requirements.push(FileLearnedConsistencyRequirement { journal_revision: ordinal(index)?,
                attempt_or_request: *key, external_request, actor_revision: *revision,
                row: *row, encoded_source_bytes: source.bytes.len() });
        }
        Ok(requirements)
    }

    /// An independently retained configuration and EXACTLY the canonical
    /// forecast-event source inventory are required. Recreate these checked
    /// captures through the original source checker, not from archived bounds.
    /// Ordinary open deliberately cannot hydrate this profile.
    ///
    /// After matching EVERY input, execute the original machine and its normal
    /// recovery fence before exposing fresh reviewer/observer roles. Pending
    /// forecasts lose coverage; consumed evidence and spent budgets never reset.
    /// Recovery repeats physical numerical work; it restores logical accounting,
    /// not an exactly-once computation guarantee or authenticated source origin.
    pub fn open_with_learned_action_consistency(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileLearnedConsistencyConfig,
        sources: &BTreeMap<u64, CheckedLearnedKv>)
        -> Result<(Self, FileHumanReviewer, FileConsistencyObserver), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let mut events = journal::decode(&profile, store.identity(), &bytes)?;
        bind_events(&mut events, expected, sources)?;
        let machine = Machine::replay(&profile, &events)?;
        // Missing/wrong/unbound inputs cannot trigger cleanup, a fence, keys or
        // publication. The original exclusive Store remains the only sink.
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let observer = FileConsistencyObserver { issuer: Rc::clone(&host.issuer) };
        Ok((host, reviewer, observer))
    }

    /// Historical canonical costs and likelihood only. A visible replacement
    /// may have lost its acknowledgment. No live authority, current clock,
    /// fresh capture, or recovery fence is returned or installed.
    pub fn read_learned_action_consistency(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileLearnedConsistencyConfig, sources: &BTreeMap<u64, CheckedLearnedKv>)
        -> Result<FileLearnedConsistencySnapshot, JournalError>
    {
        Ok(Self::read_publication_with_learned_action_consistency(directory, profile, expected, sources)?.consistency)
    }

    /// Reconstruct the original publication outcome AND learned evidence from
    /// the same canonical bytes. Separate reads could observe different cuts.
    /// This is available while a failed writable owner still holds its lock.
    /// No cleanup, fence, key issuance, resend or current-time observation occurs.
    /// Missing evidence refuses rather than falling back to an unchecked payload.
    pub fn read_publication_with_learned_action_consistency(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, expected: &FileLearnedConsistencyConfig,
        sources: &BTreeMap<u64, CheckedLearnedKv>) -> Result<FileLearnedPublicationSnapshot, JournalError>
    {
        let mut events = read_events(directory.as_ref(), profile)?;
        bind_events(&mut events, expected, sources)?;
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

fn read_events(directory: &Path, profile: &FileOversightProfile) -> Result<Vec<Event>, JournalError> {
    super::super::super::super::codec::validate_profile(&profile.delivery)?;
    let identity = storage::identity(directory)?;
    let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
    Ok(journal::decode(profile, &identity, &bytes)?)
}
fn ordinal(index: usize) -> Result<u64, Error> {
    u64::try_from(index).map_err(|_| Error::Overflow)?.checked_add(1).ok_or(Error::Overflow)
}

pub(super) fn bind_events(events: &mut [Event], expected: &FileLearnedConsistencyConfig,
    sources: &BTreeMap<u64, CheckedLearnedKv>) -> Result<(), Error>
{
    let mut configured = false;
    let mut captures = 0_usize;
    for (index, event) in events.iter_mut().enumerate() {
        match event {
            Event::Consistency(ConsistencyEvent::EnableLearned(config)) => {
                if configured { return Err(Error::Duplicate); }
                if config.bytes.as_ref() != expected.encoded() { return Err(Error::Binding); }
                config.runtime = Some(Rc::new(expected.clone()));
                configured = true;
            }
            Event::Consistency(ConsistencyEvent::ForecastLearned(_, _, _, capture)
                | ConsistencyEvent::ForecastLearnedRequest(_, _, _, capture)) => {
                if !configured { return Err(Error::Incomplete); }
                let source = sources.get(&ordinal(index)?).ok_or(Error::Missing)?;
                if source.encode()?.as_slice() != capture.bytes.as_ref() { return Err(Error::Binding); }
                capture.runtime = Some(source.clone());
                captures = captures.checked_add(1).ok_or(Error::Overflow)?;
            }
            _ => {}
        }
    }
    if !configured { return Err(Error::Incomplete); }
    if captures != sources.len() { return Err(Error::Binding); }
    Ok(())
}
