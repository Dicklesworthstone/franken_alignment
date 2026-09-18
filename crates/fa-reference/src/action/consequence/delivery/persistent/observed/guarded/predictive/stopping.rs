//! Original consistency-stop evidence from one exactly pinned canonical image.
use super::{FileOversight, FileOversightProfile, FilePredictiveRequirements, FilePredictiveSnapshot,
    JournalError, checked_image, storage};
use crate::action::consequence::oversight::consistency::{ConsistencyStopIncident, ConsistencyStopPolicy};
use std::path::Path;

/// Historical incident and local stop, not an endpoint nonexecution receipt.
/// Absence of an incident in this image is not proof that an uncommitted gap
/// never occurred. The original snapshot layout and inspection API are unchanged.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::FilePredictiveStopSnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn authorize(report: FilePredictiveStopSnapshot) -> FilePermit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePredictiveStopSnapshot {
    pub history: FilePredictiveSnapshot,
    pub policy: Option<ConsistencyStopPolicy>,
    pub incident: Option<ConsistencyStopIncident>,
}

impl FileOversight {
    /// Read the acknowledged native contract, not a caller-asserted policy.
    pub fn consistency_stop_policy(&self) -> Result<Option<ConsistencyStopPolicy>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.consistency_stop_policy())
    }

    /// Native cause, original samples and separate local-stop result. No copied
    /// incident can replace the live process, reopen a key or settle an effect.
    pub fn consistency_stop_incident(&self) -> Result<Option<&ConsistencyStopIncident>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.consistency_stop_incident())
    }

    /// Pin every original predictive guard and history floor before returning an
    /// incident, including beside a locked/faulted owner. Full-history validation
    /// precedes observation; no writer, cleanup, fresh clock or role is acquired.
    pub fn read_predictive_stop(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FilePredictiveRequirements) -> Result<FilePredictiveStopSnapshot, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(profile, &identity, &bytes, expected)?;
        let credibility = expected.evaluation.as_ref().map(|_| machine.broker.credibility_report()).transpose()?;
        Ok(FilePredictiveStopSnapshot {
            history: FilePredictiveSnapshot { journal: machine.snapshot(events.len()),
                consistency: machine.consistency_snapshot(events.len() as u64)?, credibility },
            policy: machine.broker.consistency_stop_policy(),
            incident: machine.broker.consistency_stop_incident().cloned(),
        })
    }
}

#[cfg(test)]
mod tests;
