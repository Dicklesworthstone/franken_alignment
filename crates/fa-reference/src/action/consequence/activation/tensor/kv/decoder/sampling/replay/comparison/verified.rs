//! A saved baseline must pass the original replay before a pair is observable.
//!
//! Policy and paired-work admission freeze before verification starts. Parsed
//! archive words remain expectations, never installed state. Reconstruction and
//! paired recomputation have separate work rows and no executable-owner escape.
use super::{ComparisonLimits, ComparisonLineage, PolicyComparison};
use super::super::{GenerationCheckpoint, GenerationReplay, ReplayBudget, ReplayReceipt, ReplayStatus,
    archive::GenerationArchive};
use super::super::super::monitored::{GenerationTelemetryWork, GenerationWork};
use super::super::super::super::monitoring::LearnedDecoderPolicy;
use crate::Error;
use std::fmt;

/// Reported work is not a complete accounting of an errored/unwound verifier.
/// In particular, a failed source check may have spent work without returning
/// telemetry. Incomplete verification is never a successful experiment result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparationReport {
    pub status: ReplayStatus,
    pub compared_positions: usize,
    pub reserved_and_accepted_work: GenerationWork,
    pub reported_telemetry: GenerationTelemetryWork,
    pub verification_complete: bool,
}

/// No paired outputs, mutable verifier, executable generation or candidate
/// decision is exposed before the ORIGINAL checkpoint verifier succeeds.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::verified::ComparisonPreparation;
/// fn bypass(preparation: &mut ComparisonPreparation) { preparation.comparison_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::{PolicyComparison, verified::ComparisonPreparation};
/// fn trust_partial(preparation: ComparisonPreparation) -> PolicyComparison { preparation }
/// ```
pub struct ComparisonPreparation {
    replay: GenerationReplay,
    pair: PolicyComparison,
}
impl fmt::Debug for ComparisonPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComparisonPreparation").field("report", &self.report()).finish_non_exhaustive()
    }
}

impl GenerationCheckpoint {
    /// Frozen candidate policy is for an isolated experiment, never a replacement
    /// for the baseline's original monitoring during checkpoint verification.
    pub fn begin_policy_comparison(&self, candidate: LearnedDecoderPolicy,
        replay_budget: ReplayBudget, comparison_limits: ComparisonLimits)
        -> Result<ComparisonPreparation, Error>
    {
        self.begin_replay(replay_budget)?.prepare_policy_comparison(candidate, comparison_limits)
    }
}
impl GenerationArchive {
    /// Import already checked the complete intended recipe. This still requires
    /// original numerical/learned replay; parsing alone never releases a pair.
    pub fn begin_policy_comparison(&self, candidate: LearnedDecoderPolicy,
        replay_budget: ReplayBudget, comparison_limits: ComparisonLimits)
        -> Result<ComparisonPreparation, Error>
    {
        self.begin_replay(replay_budget)?.prepare_policy_comparison(candidate, comparison_limits)
    }
}
impl GenerationReplay {
    /// Adopt only an unadvanced verifier. This freezes the candidate and paired
    /// limits before any baseline computation or final state comparison. A
    /// nonempty zero-step call does not compute a token and remains admissible.
    pub fn prepare_policy_comparison(self, candidate: LearnedDecoderPolicy, limits: ComparisonLimits)
        -> Result<ComparisonPreparation, Error>
    {
        if !matches!(self.status, ReplayStatus::Pending { compared: 0, .. }) || self.compared != 0 {
            return Err(Error::WrongState);
        }
        // start performs BOTH-arm numerical admission and both original policy
        // constructor checks, without executing inference. The pair remains
        // private until replay.finish verifies the entire saved state.
        let checkpoint = &self.checkpoint;
        let pair = PolicyComparison::start(&checkpoint.recipe, candidate, limits, ComparisonLineage {
            stream: checkpoint.recipe.stream,
            evaluation_origin: checkpoint.recipe.evaluation_origin,
            source_position: checkpoint.positions() as u64,
            source_status: checkpoint.status(),
        })?;
        Ok(ComparisonPreparation { replay: self, pair })
    }
}
impl ComparisonPreparation {
    pub fn status(&self) -> ReplayStatus { self.replay.status() }
    pub fn receipt(&self) -> Option<&ReplayReceipt> { self.replay.receipt() }
    pub fn report(&self) -> PreparationReport {
        PreparationReport { status: self.replay.status(), compared_positions: self.replay.compared,
            reserved_and_accepted_work: self.replay.candidate.work(),
            reported_telemetry: self.replay.candidate.telemetry_work(),
            verification_complete: self.replay.status() == ReplayStatus::Verified }
    }

    /// Delegate all state comparison, original-probe execution, resource guards
    /// and failure/unwind latching to the existing verifier, not a second replay.
    pub fn advance(&mut self, positions: usize) -> Result<ReplayStatus, Error> {
        self.replay.advance(positions)
    }

    /// Successful baseline verification releases ONLY an experiment from zero.
    /// The reconstructed original owner is dropped, not returned or promoted.
    /// This deliberately costs a baseline replay plus the two experimental arms;
    /// it is not a constant-time fork or a direct cache-state restart.
    pub fn finish(self) -> Result<PolicyComparison, Error> {
        let Self { replay, mut pair } = self;
        let (_verified_source, receipt) = replay.finish()?;
        pair.baseline_replay = Some(receipt);
        Ok(pair)
    }
}
