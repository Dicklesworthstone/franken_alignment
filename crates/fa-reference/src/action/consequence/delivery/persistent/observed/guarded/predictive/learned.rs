//! Recover a pinned predictive learned owner through the original replay/fence.
//! The full guard inventory and recipe are independent inputs, never disk defaults.
use super::{FilePredictiveRequirements, FilePredictiveRoles};
use super::super::{FileOversightRoles, anchored::FileHistoryAnchor, learned::check_profile};
use super::super::super::{Event, FileOversight, FileOversightProfile, JournalError};
use super::super::super::credibility::CredibilityEvent;
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus};
use crate::Error;
use std::fmt;
use std::path::Path;

/// The original exclusive learned recovery with an independently frozen complete
/// predictive guard contract. No partial owner, observer or candidate escapes.
/// The predictor must be pinned INSIDE the supplied learned recipe. An additional
/// standalone Enable cannot replace or supplement that required configuration.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::learned::FilePredictiveLearnedRecovery;
/// fn bypass(run: FilePredictiveLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::learned::FilePredictiveLearnedRecovery;
/// fn early(run: &FilePredictiveLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "finish the original guarded recovery or drop it without changing storage"]
pub struct FilePredictiveLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FilePredictiveRequirements,
    failure: Option<Error>,
}
impl fmt::Debug for FilePredictiveLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePredictiveLearnedRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Reconstruct the original learned journal and recover the complete original
    /// role bundle. Exact recipe/predictor and evaluator inventory are checked
    /// before any numerical replay; all guards, policy, credential epoch and
    /// independent counter floors are checked before cleanup or fence writes.
    /// Saved identity/source eligibility and effect keys remain withdrawn.
    pub fn begin_open_predictive_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FilePredictiveRequirements, config: &FileLearnedConfig)
        -> Result<FilePredictiveLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, None)
    }

    /// Additionally check the original independently retained exact history
    /// prefix on the SAME locked canonical bytes before numerical replay. Counter
    /// floors alone do not distinguish forks; this does not authenticate the
    /// anchor or make an out-of-date operator anchor current.
    pub fn begin_open_predictive_guarded_anchored_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FilePredictiveRequirements,
        config: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FilePredictiveLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, Some(anchor))
    }

    /// Synchronous consumer of the SAME cooperative recovery. No second history
    /// replay, role getter or replacement forecast is introduced. Provision each
    /// returned role separately; unanswered forecasts remain permanently lost
    /// coverage even though a newly bound observer is returned.
    pub fn open_predictive_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FilePredictiveRequirements, config: &FileLearnedConfig)
        -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        let mut run = Self::begin_open_predictive_guarded_with_learned_generation(
            directory, profile, expected, config)?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            let completed = run.progress().replayed_events;
            run.advance(completed, 1)?;
        }
        run.finish()
    }
}

fn begin(directory: &Path, profile: FileOversightProfile, expected: &FilePredictiveRequirements,
    config: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FilePredictiveLearnedRecovery, JournalError>
{
    check_profile(&expected.oversight.guards)?;
    if config.required_pre_output_forecast() != Some(&expected.prediction) { return Err(Error::Binding.into()); }
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    // The independent recipe has already been bound byte-for-byte. Its single
    // learned Enable installs the predictor. The base inventory deliberately
    // requires NO standalone predictor, sampled decoder, stop mode or topology.
    // Never append a synthetic Enable or filter actual events to satisfy it.
    expected.oversight.guards.check_decoder_config(events)?;
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() {
        return Err(Error::Binding.into());
    }
    Ok(FilePredictiveLearnedRecovery { inner, expected: expected.clone(), failure: None })
}

impl FilePredictiveLearnedRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// One bounded count of ORIGINAL reducer events, including every numerical
    /// witness. Original replay errors/unwinds stay terminal. A completed replay
    /// with a mismatching guard is Failed, never Ready; requirements cannot be
    /// changed afterward. One event, final encoding and I/O remain synchronous.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = self.expected.oversight.check_predictive(profile,
                self.inner.verified_guarded_machine()?, events,
                self.expected.evaluation.as_ref(), Some(&self.expected.prediction)) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    /// Check all original guards again, acknowledge the single original fence,
    /// THEN provision the original human/identity/governance/evaluation/forecast
    /// roles. No live credential or prior approval is reconstructed. A pending
    /// forecast retains its outcome uncertainty and spent lifetime budget. The
    /// numerical owner remains paused until explicit admissible requalification
    /// and resume; returning observer custody does not bypass either requirement.
    pub fn finish(self) -> Result<(FileOversight, FilePredictiveRoles), JournalError> {
        if let Some(error) = self.failure { return Err(error.into()); }
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.oversight.check_predictive(profile, self.inner.verified_guarded_machine()?,
            events, self.expected.evaluation.as_ref(), Some(&self.expected.prediction))?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FilePredictiveRoles::provision(&host, oversight, self.expected.evaluation.is_some());
        Ok((host, roles))
    }
}

#[cfg(test)]
mod tests;
