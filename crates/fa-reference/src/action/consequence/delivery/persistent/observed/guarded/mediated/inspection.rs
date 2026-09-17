//! Inspect canonical topology changes without recovering observer authority.
use super::{FileMediatedRequirements, FileOversight, FileOversightProfile, JournalError,
    Event, MediationEvent, checked_image, storage};
use super::super::super::FileDeliverySnapshot;
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::delivery::persistent::observed::mediation::{FileMediationSnapshot, FileMediationUpdate};
use crate::action::consequence::delivery::persistent::observed::consistency::FileConsistencySnapshot;
use crate::action::consequence::oversight::credibility::CredibilityReport;
use std::path::Path;

/// Native committed result, retaining the exact input and its original revision.
/// None means an already-unavailable graph made withdrawal a native no-op, not
/// that an operation was missing or that no external effect executed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTopologyUpdateRecord {
    pub journal_revision: u64,
    pub request: FileMediationUpdate,
    pub change: Option<TopologyChange>,
}

/// One completely validated historical cut. These are native reconstructed
/// results, not current source observations, an owner acknowledgment or roles.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedSnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn grant(report: FileMediatedSnapshot) -> FilePermit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediatedSnapshot {
    pub journal: FileDeliverySnapshot,
    pub topology: FileMediationSnapshot,
    pub updates: Vec<FileTopologyUpdateRecord>,
    pub consistency: Option<FileConsistencySnapshot>,
    pub credibility: Option<CredibilityReport>,
}

impl FileOversight {
    /// Resolve a selected canonical image beside a locked or faulted owner. The
    /// exact old/new graph expectation is explicit even after an ambiguous write;
    /// an unexpected topology is refused, never blessed as fresh capture. No lock,
    /// staging cleanup, fence, clock update or callback can run through this API.
    pub fn read_mediation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileMediatedRequirements) -> Result<FileMediatedSnapshot, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = checked_image(profile, &identity, &bytes, expected)?;
        let mut updates = Vec::new();
        for (position, event) in events.iter().enumerate() {
            if let Event::Mediation(MediationEvent::Update(request)) = event {
                updates.push(FileTopologyUpdateRecord { journal_revision: (position + 1) as u64,
                    request: request.clone(), change: machine.mediation_update(request.operation)?.clone() });
            }
        }
        Ok(FileMediatedSnapshot { journal: machine.snapshot(events.len()),
            topology: machine.mediation_snapshot(events.len() as u64)?, updates,
            consistency: expected.prediction.as_ref().map(|_| machine.consistency_snapshot(events.len() as u64)).transpose()?,
            credibility: expected.evaluation.as_ref().map(|_| machine.broker.credibility_report()).transpose()? })
    }
}
