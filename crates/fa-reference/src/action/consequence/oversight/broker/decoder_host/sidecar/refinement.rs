//! Coarse-to-exact evidence purchases stay at the same live proposal/source cut.
use super::{HostedSidecar, OversightBroker};
use crate::action::consequence::oversight::{ObservedReview, ObservedSession, ReviewWindow,
    sidecar::SidecarRefinementOutcome};
use crate::{Error, Snapshot};
use std::rc::Rc;

impl OversightBroker {
    /// Start the ORIGINAL independent review over this handle's current input.
    /// The caller cannot switch the attempt, source or helper-visible packet.
    pub fn begin_hosted_sidecar_review(&mut self, sidecar: &HostedSidecar, round: u64,
        root: [u8; 32], window: ReviewWindow, snapshot: &Snapshot) -> Result<ObservedSession, Error>
    {
        self.check_hosted_sidecar(sidecar)?;
        self.begin_review(sidecar.attempt, round, root, window, snapshot)
    }

    /// A completed, unapplied original review may buy the next frozen residual.
    /// This does NOT apply a consequence or grant approval. Only a distinct round
    /// over the newly recorded complete input can support subsequent permission.
    /// Missing workers, missing residuals and exhausted budgets remain explicit.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::oversight::decoder_host::sidecar::HostedSidecar;
    /// fn retune(sidecar: &mut HostedSidecar) { sidecar.plan_mut(); }
    /// ```
    pub fn refine_hosted_sidecar(&mut self, sidecar: &mut HostedSidecar, review: &ObservedReview)
        -> Result<SidecarRefinementOutcome, Error>
    {
        self.check_hosted_sidecar(sidecar)?;
        if !Rc::ptr_eq(&review.issuer, &self.issuer) || review.attempt != sidecar.attempt {
            return Err(Error::Binding);
        }
        if review.revision != sidecar.input_revision { return Err(Error::Stale); }
        let action = sidecar.current.input().action();
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < review.completed_at || now >= action.spec().deadline { return Err(Error::Stale); }
        // Prepare against a private copy; a failure of the ORIGINAL cumulative
        // input-byte admission must not consume a residual or advance the public
        // handle without recording its matching whole-input version.
        let mut prepared = sidecar.plan.clone();
        let outcome = prepared.refine_after(review, action, &self.contracts)?;
        if let SidecarRefinementOutcome::Refined { round, .. } = &outcome {
            let published = round.clone();
            let revision = self.record_inputs(sidecar.attempt, sidecar.input_revision, round.input().clone())?;
            // All cloning, allocation, encoding and admission preceded the input
            // commit; only infallible owner-field moves follow it. Old snapshots
            // and original completed reviews are not changed or reclassified.
            sidecar.plan = prepared;
            sidecar.current = published;
            sidecar.input_revision = revision;
        }
        Ok(outcome)
    }
}
