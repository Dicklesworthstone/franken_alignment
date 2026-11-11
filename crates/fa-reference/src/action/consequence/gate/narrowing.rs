//! Explicit supervisor restriction through the original consequence authority.
//! No synthetic congress round, checkpoint, policy replacement or new issuer.
use super::{ConsequenceAuthority, TargetCeiling, undispatched};
use crate::action::consequence::Consequence;
use crate::action::{ActionState, Scope};
use crate::{Error, State};

/// The original authority's exact monotone transition. Dispatched liabilities
/// and retained-target reservations stay owned by the same authority. A new
/// human approval may still be needed for the new control predecessor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetNarrowReceipt {
    pub scope: Scope,
    pub previous_sequence: u64,
    pub sequence: u64,
    pub authority_epoch: u64,
    pub previous_ceiling: Option<TargetCeiling>,
    pub ceiling: TargetCeiling,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

impl ConsequenceAuthority {
    pub(crate) fn narrow_targets(
        &mut self,
        expected_control_sequence: u64,
        expected_authority_epoch: u64,
        retained: TargetCeiling,
    ) -> Result<TargetNarrowReceipt, Error> {
        if self.suspended { return Err(Error::WrongState); }
        if self.sequence != expected_control_sequence
            || self.authority.rights.epoch() != expected_authority_epoch {
            return Err(Error::Stale);
        }
        self.check_record_capacity()?;
        self.narrow_receipts.try_reserve(1).map_err(|_| Error::Limit)?;
        let sequence = self.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let ceiling = self.ceiling.as_ref().map_or_else(
            || retained.clone(), |current| current.intersect(&retained));
        let cancelled: Vec<_> = self.authority.attempts.iter().filter_map(|(id, attempt)| {
            (undispatched(attempt.stage)
                && !attempt.action.spec().target.is_some_and(|target| ceiling.contains(target)))
                .then_some(*id)
        }).collect();
        // Stage accounting before changing the original authority. No epoch is
        // minted: in-ceiling automatic permits retain their original reservation.
        let mut rights = self.authority.rights.clone();
        if !rights.conserved() { return Err(Error::WrongState); }
        let available = rights.available();
        for id in &cancelled {
            if self.authority.attempts[id].stage == ActionState::Authorized {
                if rights.state(*id)? != State::Reserved { return Err(Error::WrongState); }
                rights.abort_before_dispatch(*id)?;
            }
        }
        if !rights.conserved() { return Err(Error::WrongState); }
        let refunded_units = rights.available().checked_sub(available).ok_or(Error::WrongState)?;
        let receipt = TargetNarrowReceipt {
            scope: self.authority.scope, previous_sequence: self.sequence, sequence,
            authority_epoch: expected_authority_epoch, previous_ceiling: self.ceiling.clone(),
            ceiling: ceiling.clone(), cancelled: cancelled.clone(), refunded_units,
        };
        self.authority.rights = rights;
        for id in cancelled {
            self.authority.attempts.get_mut(&id).expect("retained attempt").stage = ActionState::Cancelled;
            self.decisions.insert(id, Consequence::Deny);
        }
        self.ceiling = Some(ceiling);
        self.sequence = sequence;
        self.narrow_receipts.push(receipt.clone());
        Ok(receipt)
    }
}
