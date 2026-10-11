//! Read-only cooperative screening of the owner's ORIGINAL retained requirements.
use super::{DeliveryBroker, PublicationBasis, PublicationInputs};
use crate::action::consequence::oversight::publication::PublicationRefinement;
use crate::Error;

impl DeliveryBroker {
    /// Begin a comparison of retained evidence at an exact input predecessor.
    /// Neither a substitute action nor caller-supplied judgment is accepted.
    /// The original configured validation allowance is the lifetime ceiling;
    /// per-poll budgets can only subdivide it, not increase it.
    ///
    /// This read-only precheck does NOT claim fresh source capture, a live feed
    /// lease or current authority. It does not consume/refresh a capture, record
    /// an approval, alter last-validation diagnostics, or mint either key.
    /// Authorize, dispatch and first publication still run their original final
    /// checks. A historical match cannot survive new evidence by being replayed.
    /// The borrow prevents the retained cut from being edited during comparison.
    ///
    /// ```compile_fail,E0502
    /// use fa_reference::action::consequence::delivery::DeliveryBroker;
    /// use fa_reference::witness::refinement::RefinementBudget;
    /// fn no_mutable_cut(owner: &mut DeliveryBroker, attempt: u64, revision: u64) {
    ///     let mut check = owner.begin_publication_precheck(attempt, revision).unwrap();
    ///     owner.record_publication_inputs(attempt, revision, None).unwrap();
    ///     let _ = check.advance(RefinementBudget::default());
    /// }
    /// ```
    pub fn begin_publication_precheck(&self, attempt: u64, expected_revision: u64)
        -> Result<PublicationRefinement<'_>, Error>
    {
        let gate = self.publication.as_ref().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get(&attempt).ok_or(Error::Missing)?;
        if slot.revision != expected_revision { return Err(Error::Stale); }
        let judgment = slot.judgment.as_ref().ok_or(Error::Incomplete)?;
        let basis = slot.current.as_ref().map_or_else(PublicationBasis::default, PublicationInputs::basis);
        Ok(judgment.begin_refinement(basis, gate.limits.validation))
    }
}
