//! Original guarded recovery with an independently pinned OWNED K/V predictor.
//! No supplied capture map, imported fidelity bounds or second replay engine.
mod inspection;
pub use inspection::FileOwnedPredictiveSnapshot;
use super::{EvaluationProtocol, FilePredictiveRoles, FileRecoveryRequirements};
use super::super::{FileOversightRoles, anchored::FileHistoryAnchor, learned::check_profile};
use super::super::super::{Event, FileOversight, FileOversightProfile, JournalError, Machine};
use super::super::super::consistency::learned::FileLearnedConsistencyConfig;
use super::super::super::credibility::CredibilityEvent;
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use std::fmt;
use std::path::Path;

/// Independent recovery inputs. None in the original guard/evaluation inventory
/// requires absence. The learned prediction includes source mode, timing policy,
/// per-job/lifetime work and inventory limits, not just its raw probability table.
/// Keep these inputs outside the rollbackable journal being recovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOwnedPredictiveRequirements {
    pub oversight: FileRecoveryRequirements,
    pub prediction: FileLearnedConsistencyConfig,
    pub evaluation: Option<EvaluationProtocol>,
}

/// The SAME exclusive numerical recovery, with mandatory final guard checks.
/// Readiness includes exact guard, policy, credential and counter agreement.
/// Neither a partially reconstructed machine nor an unchecked role can escape.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::owned::FileOwnedPredictiveRecovery;
/// fn bypass(run: FileOwnedPredictiveRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::owned::FileOwnedPredictiveRecovery;
/// fn premature(run: &FileOwnedPredictiveRecovery) { run.roles(); }
/// ```
#[must_use = "finish the original guarded recovery or drop it without changing storage"]
pub struct FileOwnedPredictiveRecovery {
    inner: FileLearnedRecovery,
    expected: FileOwnedPredictiveRequirements,
    failure: Option<Error>,
}
impl fmt::Debug for FileOwnedPredictiveRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileOwnedPredictiveRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Bind both independent configurations before the first original numerical
    /// replay. The complete guard inventory, effective policy, credential epoch
    /// and counter floors must agree before any cleanup, fence or role issuance.
    /// Raw-predictive recovery and supplied-capture hydration are not fallbacks.
    pub fn begin_open_predictive_guarded_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
        generation: &FileLearnedConfig) -> Result<FileOwnedPredictiveRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, generation, None)
    }

    /// Additionally require the original exact externally retained history
    /// prefix, checked against the SAME locked image before replay. Counter
    /// floors alone cannot distinguish equal-counter forks or a stale anchor.
    pub fn begin_open_predictive_guarded_anchored_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
        generation: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FileOwnedPredictiveRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, generation, Some(anchor))
    }

    /// Synchronous consumer of the SAME cooperative replay and single original
    /// fence. Returned roles are new custody, not restored approval, fresh source
    /// evidence or a refill of spent numerical/statistical allowances.
    pub fn open_predictive_guarded_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
        generation: &FileLearnedConfig) -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        let mut recovery = Self::begin_open_predictive_guarded_with_owned_learned_consistency(
            directory, profile, expected, generation)?;
        while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
            recovery.advance(recovery.progress().replayed_events, 1)?;
        }
        recovery.finish()
    }
}

fn begin(directory: &Path, profile: FileOversightProfile, expected: &FileOwnedPredictiveRequirements,
    generation: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileOwnedPredictiveRecovery, JournalError>
{
    check_configuration(expected, generation)?;
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, generation)?
        .bind_owned_prediction(&expected.prediction)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    check_history(events, expected)?;
    Ok(FileOwnedPredictiveRecovery { inner, expected: expected.clone(), failure: None })
}

fn check_configuration(expected: &FileOwnedPredictiveRequirements, generation: &FileLearnedConfig)
    -> Result<(), Error>
{
    check_profile(&expected.oversight.guards)?;
    if !expected.prediction.uses_owned_generation() || generation.required_pre_output_forecast().is_some() {
        // An owned-code predictor is not the recipe's separately pinned raw
        // residual predictor. Never strip the latter to admit a second mode.
        return Err(Error::Binding);
    }
    Ok(())
}

fn check_history(events: &[Event], expected: &FileOwnedPredictiveRequirements) -> Result<(), Error> {
    // The original binder has already matched the sole learned configuration
    // and rejected all supplied captures. Reject additional raw/decoder/topology
    // bootstrap here, before executing any of the independently bound numerics.
    expected.oversight.guards.check_decoder_config(events)?;
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() {
        return Err(Error::Binding);
    }
    Ok(())
}

fn check_machine(profile: &FileOversightProfile, machine: &Machine, events: &[Event],
    expected: &FileOwnedPredictiveRequirements) -> Result<(), Error>
{
    if machine.learned_consistency.as_deref() != Some(&expected.prediction)
        || !machine.broker.owned_learned_consistency_required() { return Err(Error::Binding); }
    // Original guard/policy/counter validator, in addition to the complete
    // learned representation check above. No raw evidence is manufactured.
    expected.oversight.check_predictive(profile, machine, events, expected.evaluation.as_ref(),
        Some(expected.prediction.consistency()))
}

impl FileOwnedPredictiveRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// At most max_events original reductions. All numerical/outcome witnesses
    /// are checked by their original implementations. A failed final inventory
    /// check is sticky and never exposes Ready. An event is not a time bound.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = check_machine(profile, self.inner.verified_guarded_machine()?, events, &self.expected) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    /// Recheck the original guards, commit exactly the original recovery fence,
    /// THEN provision the separate human/identity/governance/evaluation/forecast
    /// roles. Numerical history is not replayed a second time at this boundary.
    /// Unanswered forecasts stay lost coverage; unknown effects remain charged;
    /// resumed generation still requires original requalification and fresh time.
    pub fn finish(self) -> Result<(FileOversight, FilePredictiveRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FilePredictiveRoles::provision(&host, oversight, self.expected.evaluation.is_some());
        Ok((host, roles))
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure { return Err(error.into()); }
        let (profile, events, _) = self.inner.guarded_history();
        check_machine(profile, self.inner.verified_guarded_machine()?, events, &self.expected)?;
        Ok(())
    }

    /// Complete only the independently selected original pending reset, keeping
    /// the owned source mode, pre-output timing, numerical/lifetime ceilings and
    /// optional evaluator fixed. The full contract is checked around the SAME
    /// private reset before its completion and fence are published atomically.
    /// No role escapes an unacknowledged write, and an exact completed retry
    /// cannot reset, audit or count an incident again. Pending forecast loss and
    /// spent learned work are retained by the original fence, never re-armed.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FilePredictiveRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }
}

#[cfg(test)]
mod tests;
