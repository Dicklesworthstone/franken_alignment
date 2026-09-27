//! Buy only frozen, originally checked residuals after an original abstention.
use super::{LearnedSidecar, OversightBroker};
use crate::action::consequence::oversight::{ObservedReview, sidecar::SidecarRefinementOutcome};
use crate::Error;
use std::rc::Rc;

impl OversightBroker {
    /// A completed UNAPPLIED original round may disclose its next registered
    /// residual. No raw recapture, new inference, policy retuning or implicit
    /// permission occurs. Changed bytes require a NEW independent review.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::oversight::learned_host::sidecar::LearnedSidecar;
    /// fn enlarge(sidecar: &mut LearnedSidecar) { sidecar.budget_mut(); }
    /// ```
    pub fn refine_learned_sidecar(&mut self, sidecar: &mut LearnedSidecar, review: &ObservedReview)
        -> Result<SidecarRefinementOutcome, Error>
    {
        self.check_learned_sidecar(sidecar)?;
        if !Rc::ptr_eq(&review.issuer, &self.issuer) || review.attempt != sidecar.attempt {
            return Err(Error::Binding);
        }
        if review.revision != sidecar.input_revision { return Err(Error::Stale); }
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < review.completed_at { return Err(Error::Stale); }
        let action = sidecar.current.input().action();
        // Original planning can succeed while the broker's cumulative input cap
        // refuses. Stage BOTH planner and requirement marker before recording;
        // no purchased residual or changed frontier escapes a failed commit.
        let mut prepared = sidecar.plan.clone();
        let outcome = prepared.refine_after(review, action, &self.contracts)?;
        if let SidecarRefinementOutcome::Refined { round, .. } = &outcome {
            let published = round.clone();
            let revision = self.record_learned_sidecar_input(sidecar.attempt, sidecar.input_revision, round.input())?;
            sidecar.plan = prepared;
            sidecar.current = published;
            sidecar.input_revision = revision;
        }
        Ok(outcome)
    }
}
