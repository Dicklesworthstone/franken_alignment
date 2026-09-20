//! A complete feed may outrun its producer without invalidating the original
//! review. Retain that observation, but never install it as current evidence.
use super::{DeliveryBroker, PublicationInputCut, PublicationInputs, Slot};
use super::super::changes::PublicationChangeStatus;
use crate::action::ActionState;
use crate::Error;

/// The result of one original withdrawal/read cycle. Installed means only that
/// witness inputs may be compared ONCE, not that comparison or authorization
/// succeeded. Deferred is a negative observation, never a validation basis.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationCaptureOutcome;
/// fn grant(observation: PublicationCaptureOutcome) -> Permit { observation }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationCaptureOutcome {
    Installed { revision: u64 },
    Deferred { revision: u64, observed: PublicationInputCut, required_through: u64 },
}
impl PublicationCaptureOutcome {
    pub fn revision(self) -> u64 {
        match self { Self::Installed { revision } | Self::Deferred { revision, .. } => revision }
    }
    pub fn deferred(self) -> bool { matches!(self, Self::Deferred { .. }) }
}

impl DeliveryBroker {
    /// Admit a cut-aware observation before dispatch. The ONLY recoverable
    /// deficiency is a nonregressing image behind selected invalidations in a
    /// COMPLETE feed. Missing/foreign/future cuts, equivocation, rollback and
    /// missing feed coverage still refuse. Legacy bindings cannot use this API.
    ///
    /// A deferred read consumes its acquisition cycle and retains exact image
    /// bytes, generation, snapshot floors and the covered prefix. It preserves
    /// the original judgment and rights, but installs no current inputs. Retry
    /// requires a new withdrawal and real read; existing one-use validation,
    /// committee/human keys, deadlines and publication checks are unchanged.
    /// This operation cannot extend a dispatched effect's execution window.
    pub fn record_captured_publication_inputs_or_defer(&mut self, attempt: u64,
        expected_revision: u64, source: u64, generation: u64, inputs: PublicationInputs,
        input_cut: PublicationInputCut) -> Result<PublicationCaptureOutcome, Error>
    {
        if !matches!(self.inspect().ledger.stages.get(&attempt),
            Some(ActionState::Reviewing | ActionState::Authorized)) {
            return Err(Error::WrongState);
        }
        let feed = self.publication_change_status()?;
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get_mut(&attempt).ok_or(Error::Missing)?;
        slot.capture_or_defer(expected_revision, source, generation, inputs, input_cut, feed)
    }
}

impl Slot {
    fn capture_or_defer(&mut self, expected_revision: u64, source: u64,
        generation: u64, inputs: PublicationInputs, supplied: PublicationInputCut,
        feed: PublicationChangeStatus) -> Result<PublicationCaptureOutcome, Error>
    {
        if self.revision != expected_revision { return Err(Error::Stale); }
        let retained = self.source.as_ref().ok_or(Error::Incomplete)?;
        if !retained.status.capture_pending || self.current.is_some() { return Err(Error::WrongState); }
        if retained.status.source != source { return Err(Error::Binding); }
        if generation < retained.status.generation { return Err(Error::Stale); }
        if generation == retained.status.generation && inputs != retained.last { return Err(Error::Binding); }
        let bound = retained.input_cut.ok_or(Error::Binding)?;
        supplied.check()?;
        if supplied.source != bound.last.source || supplied.source != feed.source { return Err(Error::Binding); }
        if generation == retained.status.generation && supplied != bound.last { return Err(Error::Binding); }
        if supplied.through < bound.last.through { return Err(Error::Stale); }
        if !feed.complete() || supplied.through > feed.through { return Err(Error::Incomplete); }
        self.check_floor(Some(&inputs))?;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let deferred = supplied.through < bound.required_through;
        let outcome = if deferred {
            PublicationCaptureOutcome::Deferred { revision, observed: supplied,
                required_through: bound.required_through }
        } else { PublicationCaptureOutcome::Installed { revision } };
        // All checks and the possible deep copy precede mutation. Deferred data
        // is retained exactly once, not duplicated into the current-input lane.
        let current = if deferred { None } else { Some(inputs.clone()) };
        let floor = super::cut(&inputs).or(self.floor);
        let retained = self.source.as_mut().expect("checked source");
        retained.last = inputs;
        retained.status.generation = generation;
        retained.status.capture_pending = false;
        retained.status.fresh = !deferred;
        retained.input_cut.as_mut().expect("checked cut binding").last = supplied;
        self.current = current;
        self.floor = floor;
        self.revision = revision;
        self.last = None;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests;
