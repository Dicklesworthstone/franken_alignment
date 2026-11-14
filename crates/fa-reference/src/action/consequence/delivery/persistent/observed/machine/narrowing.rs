use super::{Machine, Transition};
use super::super::narrowing::{
    FileAuthorityNarrowReceipt, FileAuthorityNarrowRequest, MAX_AUTHORITY_NARROWINGS,
};
use crate::Error;

impl Machine {
    pub(in super::super) fn authority_narrowing_supported(&self) -> bool {
        // Every original owner uses the same exact target lattice. Restriction
        // changes no numerical predecessor, forecast or audience boundary. The
        // caller's separate replay allowance governs numerical reconstruction;
        // the immutable original recipes still bound that reconstruction.
        true
    }

    pub(in super::super) fn authority_narrowing_requires_topology(&self) -> bool {
        self.mediation.is_some()
    }

    pub(in super::super) fn narrowing_retry(&self, request: &FileAuthorityNarrowRequest)
        -> Result<Option<FileAuthorityNarrowReceipt>, Error>
    {
        match self.narrowings.get(&request.operation()) {
            Some(receipt) if receipt.request() == request => Ok(Some(receipt.clone())),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    pub(in super::super) fn check_authority_narrow(&self, request: &FileAuthorityNarrowRequest)
        -> Result<(), Error>
    {
        request.ceiling()?;
        if !self.authority_narrowing_supported() { return Err(Error::Incomplete); }
        let state = self.broker.inspect();
        if self.broker.stop_receipt().is_some() || state.suspended { return Err(Error::WrongState); }
        if request.expected_control_sequence() != state.sequence
            || request.expected_authority_epoch() != state.ledger.epoch { return Err(Error::Stale); }
        if self.narrowings.contains_key(&request.operation()) { return Err(Error::Duplicate); }
        if self.narrowings.len() >= MAX_AUTHORITY_NARROWINGS { return Err(Error::Limit); }
        Ok(())
    }

    pub(super) fn apply_authority_narrow(&mut self, request: &FileAuthorityNarrowRequest)
        -> Result<Transition, Error>
    {
        self.check_authority_narrow(request)?;
        // Intersecting the exact target ceiling changes neither this original
        // graph nor its authority epoch. Existing graph/cut custody stays with
        // the broker; source loss and replacement retain their revocation law.
        let change = self.broker.narrow_targets(request.expected_control_sequence(),
            request.expected_authority_epoch(), request.ceiling()?)?;
        for id in &change.cancelled { self.automatic.remove(id); }
        self.sessions.retain(|_, (attempt, _)| !change.cancelled.contains(attempt));
        // Human requests bind the complete old control sequence, even for a
        // retained target. Record their original withdrawal; a fresh independent
        // human approval is required, and its old work budget is never refunded.
        self.withdraw_keys()?;
        let receipt = FileAuthorityNarrowReceipt { request: request.clone(), change };
        self.narrowings.insert(request.operation(), receipt.clone());
        Ok(Transition::AuthorityNarrowed(receipt))
    }
}
