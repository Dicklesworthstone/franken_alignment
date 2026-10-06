//! Historical projections beside a locked or faulted predictive learned owner.
//! One canonical image, original exact recipe binding and original semantic replay.
use super::{check_configuration, check_history, FileLearnedConfig, FileOversight,
    FileOversightProfile, FilePredictiveRequirements, JournalError};
use super::super::FilePredictiveSnapshot;
use crate::action::consequence::delivery::persistent::{codec,
    observed::{journal, storage, machine::Machine, decoder::learned::bind_history}};
use std::path::Path;

impl FileOversight {
    /// Inspect the SAME predictive learned profile without acquiring a writer,
    /// cleaning storage, fencing, resuming inference or provisioning any role.
    /// Exact pinned predictor/recipe, full guard inventory, effective policy,
    /// credential epoch and independent floors must match, as during recovery.
    /// Unanswered forecasts remain historically pending, not silently expired.
    /// This is not a live source observation, an identity check or a send outcome.
    ///
    /// One complete canonical image is read and reconstructed. All original
    /// numerical steps and witness checks execute during that historical replay;
    /// no saved tensor, likelihood state or approval is imported. Work remains
    /// synchronous. A concurrently replaced newer image may differ from the
    /// live owner's RAM, especially after an ambiguous storage acknowledgment.
    pub fn read_predictive_consistency_with_learned_generation(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, expected: &FilePredictiveRequirements,
        config: &FileLearnedConfig) -> Result<FilePredictiveSnapshot, JournalError>
    {
        check_configuration(expected, config)?;
        codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        bind_history(&mut events, config)?;
        check_history(&events, expected)?;
        let machine = Machine::replay(profile, &events)?;
        expected.oversight.check_predictive(profile, &machine, &events,
            expected.evaluation.as_ref(), Some(&expected.prediction))?;
        let credibility = expected.evaluation.as_ref().map(|_| machine.broker.credibility_report()).transpose()?;
        Ok(FilePredictiveSnapshot { journal: machine.snapshot(events.len()),
            consistency: machine.consistency_snapshot(events.len() as u64)?, credibility })
    }
}
