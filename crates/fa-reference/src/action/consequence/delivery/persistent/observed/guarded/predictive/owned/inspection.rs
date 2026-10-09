//! Same independent guards on ONE canonical image, without acquiring authority.
use super::{Error, FileHistoryAnchor, FileLearnedConfig, FileOversight, FileOversightProfile,
    FileOwnedPredictiveRequirements, JournalError, Machine, Path,
    check_configuration, check_history, check_machine};
use super::super::super::super::{journal, storage};
use super::super::super::super::consistency::learned::{FileLearnedConsistencySnapshot,
    FileLearnedPublicationSnapshot, owned::bind_owned_history};
use crate::action::consequence::oversight::credibility::CredibilityReport;

/// Original publication/learned evidence and optional evaluator report from the
/// same historical cut. No freshness, acknowledgment, observer or permitting
/// capability is asserted by successful inspection, including after lost sync.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::owned::FileOwnedPredictiveSnapshot;
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn authorize(snapshot: FileOwnedPredictiveSnapshot) -> FilePermit { snapshot }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOwnedPredictiveSnapshot {
    pub journal: FileLearnedPublicationSnapshot,
    pub credibility: Option<CredibilityReport>,
}

impl FileOversight {
    /// Inspect beside a locked or faulted writer. Apply the same independently
    /// bound recipes, inventory and final guard/floor checks as recovery, but
    /// perform no cleanup, fence, source refresh, resume, role issuance or resend.
    pub fn read_predictive_guarded_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
        generation: &FileLearnedConfig) -> Result<FileOwnedPredictiveSnapshot, JournalError>
    {
        read(directory.as_ref(), profile, expected, generation, None)
    }

    /// Verify the externally retained exact prefix against this SAME read image,
    /// before replay. There is no separate read that could validate a different
    /// disk cut. The operator still owns the anchor's provenance and currency.
    pub fn read_predictive_guarded_anchored_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
        generation: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FileOwnedPredictiveSnapshot, JournalError>
    {
        read(directory.as_ref(), profile, expected, generation, Some(anchor))
    }
}

fn read(directory: &Path, profile: &FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
    generation: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileOwnedPredictiveSnapshot, JournalError>
{
    check_configuration(expected, generation)?;
    profile.delivery.limits.check()?;
    let identity = storage::identity(directory)?;
    let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
    let mut events = journal::decode(profile, &identity, &bytes)?;
    bind_owned_history(&mut events, generation, &expected.prediction)?;
    if let Some(anchor) = anchor { anchor.check(profile, &identity, &events)?; }
    check_history(&events, expected)?;
    let machine = Machine::replay(profile, &events)?;
    if machine.learned_contract() != Some(generation) { return Err(Error::Binding.into()); }
    check_machine(profile, &machine, &events, expected)?;
    let credibility = expected.evaluation.as_ref().map(|_| machine.broker.credibility_report()).transpose()?;
    Ok(FileOwnedPredictiveSnapshot {
        journal: FileLearnedPublicationSnapshot {
            publication: machine.snapshot(events.len()),
            consistency: FileLearnedConsistencySnapshot {
                consistency: machine.consistency_snapshot(events.len() as u64)?,
                work: machine.broker.learned_consistency_work()?,
                retained_source_bytes: machine.broker.learned_consistency_retained_source_bytes()?,
                has_unreported_work: machine.broker.learned_consistency_has_unreported_work()?,
            },
            pending_request: machine.consistency_request.map(|(request, _)| request),
        },
        credibility,
    })
}
