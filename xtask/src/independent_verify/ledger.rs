//! Independent accounting reducer. No production state machine is imported.
use super::wire::{Effect, Event};
use super::{Accounting, ErrorKind, Location, VerifyError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Reserved,
    Dispatched,
    Unknown,
    Committed,
    Aborted,
}

struct Reservation {
    units: u64,
    epoch: u64,
    state: State,
    // Only Reserved can compare an effect again. Terminal IDs stay occupied.
    effect: Option<Box<[u8]>>,
}

pub(super) struct Ledger {
    rows: BTreeMap<u64, Reservation>,
    reserved: BTreeSet<u64>,
    accounting: Accounting,
}

fn increase(value: &mut u64, amount: u64, at: Location) -> Result<(), VerifyError> {
    *value = value.checked_add(amount).ok_or_else(|| {
        VerifyError::at(ErrorKind::ArithmeticOverflow, at, "accounting addition overflowed")
    })?;
    Ok(())
}

fn decrease(value: &mut u64, amount: u64, at: Location) -> Result<(), VerifyError> {
    *value = value.checked_sub(amount).ok_or_else(|| {
        VerifyError::at(ErrorKind::ConservationFailure, at, "accounting subtraction underflowed")
    })?;
    Ok(())
}

impl Ledger {
    pub fn new(total: u64) -> Self {
        Self {
            rows: BTreeMap::new(),
            reserved: BTreeSet::new(),
            accounting: Accounting {
                total_units: total,
                available_units: total,
                spent_units: 0,
                reserved_units: 0,
                dispatched_units: 0,
                unknown_units: 0,
                epoch: 0,
                incident_count: 0,
                reservations: 0,
                reserved_count: 0,
                dispatched_count: 0,
                unknown_count: 0,
                committed_count: 0,
                aborted_count: 0,
            },
        }
    }

    fn row(&self, id: u64, at: Location) -> Result<&Reservation, VerifyError> {
        self.rows.get(&id).ok_or_else(|| {
            VerifyError::at(ErrorKind::UnknownReservation, at, "reservation identifier has never been reserved")
        })
    }

    fn require_state(&self, id: u64, state: State, at: Location) -> Result<(), VerifyError> {
        if self.row(id, at)?.state != state {
            return Err(VerifyError::at(ErrorKind::InvalidTransition, at, "event is not allowed from this reservation state"));
        }
        Ok(())
    }

    pub fn apply(&mut self, event: Event<'_>, at: Location) -> Result<(), VerifyError> {
        match event {
            Event::Reserve { id, effect } => self.reserve(id, effect, at)?,
            Event::Dispatch { id, effect } => {
                self.require_state(id, State::Reserved, at)?;
                let row = self.row(id, at)?;
                if row.epoch != self.accounting.epoch {
                    return Err(VerifyError::at(ErrorKind::StaleEpoch, at, "dispatch reservation belongs to a revoked epoch"));
                }
                if row.effect.as_deref() != Some(effect.encoded) {
                    return Err(VerifyError::at(ErrorKind::EffectMismatch, at, "dispatch effect does not exactly match its reservation"));
                }
                self.transition(id, State::Dispatched, at)?;
            }
            Event::MarkUnknown { id } => {
                self.require_state(id, State::Dispatched, at)?;
                self.transition(id, State::Unknown, at)?;
            }
            Event::Abort { id } => {
                self.require_state(id, State::Reserved, at)?;
                self.transition(id, State::Aborted, at)?;
            }
            Event::Reconcile { id, occurred } => {
                if !matches!(self.row(id, at)?.state, State::Dispatched | State::Unknown) {
                    return Err(VerifyError::at(ErrorKind::InvalidTransition, at, "only dispatched or unknown effects can be reconciled"));
                }
                self.transition(id, if occurred { State::Committed } else { State::Aborted }, at)?;
            }
            Event::Revoke => increase(&mut self.accounting.epoch, 1, at)?,
            Event::Reset { keep } => self.reset(&keep, at)?,
        }
        self.check_conservation(at)
    }

    fn reserve(&mut self, id: u64, effect: Effect<'_>, at: Location) -> Result<(), VerifyError> {
        if self.rows.contains_key(&id) {
            return Err(VerifyError::at(ErrorKind::DuplicateReservation, at, "reservation identifiers cannot be reused, including after settlement"));
        }
        if effect.units == 0 || !effect.principal_nonempty || !effect.target_nonempty {
            return Err(VerifyError::at(ErrorKind::InvalidTransition, at, "reservation requires positive units and nonempty principal and target"));
        }
        if effect.units > self.accounting.available_units {
            return Err(VerifyError::at(ErrorKind::InsufficientUnits, at, "reservation exceeds available units"));
        }
        decrease(&mut self.accounting.available_units, effect.units, at)?;
        increase(&mut self.accounting.reserved_units, effect.units, at)?;
        increase(&mut self.accounting.reserved_count, 1, at)?;
        increase(&mut self.accounting.reservations, 1, at)?;
        self.rows.insert(id, Reservation {
            units: effect.units,
            epoch: self.accounting.epoch,
            state: State::Reserved,
            effect: Some(effect.encoded.to_vec().into_boxed_slice()),
        });
        self.reserved.insert(id);
        Ok(())
    }

    fn transition(&mut self, id: u64, next: State, at: Location) -> Result<(), VerifyError> {
        let row = self.row(id, at)?;
        let (previous, units) = (row.state, row.units);
        let stats = &mut self.accounting;
        match previous {
            State::Reserved => {
                decrease(&mut stats.reserved_units, units, at)?;
                decrease(&mut stats.reserved_count, 1, at)?;
            }
            State::Dispatched => {
                decrease(&mut stats.dispatched_units, units, at)?;
                decrease(&mut stats.dispatched_count, 1, at)?;
            }
            State::Unknown => {
                decrease(&mut stats.unknown_units, units, at)?;
                decrease(&mut stats.unknown_count, 1, at)?;
            }
            State::Committed | State::Aborted => {
                return Err(VerifyError::at(ErrorKind::InvalidTransition, at, "settled reservations cannot transition"));
            }
        }
        match next {
            State::Dispatched => {
                increase(&mut stats.dispatched_units, units, at)?;
                increase(&mut stats.dispatched_count, 1, at)?;
            }
            State::Unknown => {
                increase(&mut stats.unknown_units, units, at)?;
                increase(&mut stats.unknown_count, 1, at)?;
            }
            State::Committed => {
                increase(&mut stats.spent_units, units, at)?;
                increase(&mut stats.committed_count, 1, at)?;
            }
            State::Aborted => {
                increase(&mut stats.available_units, units, at)?;
                increase(&mut stats.aborted_count, 1, at)?;
            }
            State::Reserved => {
                return Err(VerifyError::at(ErrorKind::InvalidTransition, at, "an existing identifier cannot become newly reserved"));
            }
        }
        let row = self.rows.get_mut(&id).ok_or_else(|| {
            VerifyError::at(ErrorKind::ConservationFailure, at, "reservation disappeared during accounting")
        })?;
        row.state = next;
        row.effect = None;
        if previous == State::Reserved && !self.reserved.remove(&id) {
            return Err(VerifyError::at(ErrorKind::ConservationFailure, at, "reserved index disagrees with accounting"));
        }
        Ok(())
    }

    fn reset(&mut self, keep: &[u64], at: Location) -> Result<(), VerifyError> {
        // Native journal semantics require *every* kept ID to exist, including
        // IDs that do not currently hold units.
        for &id in keep {
            self.row(id, at)?;
        }
        let mut refund = 0_u64;
        let mut count = 0_u64;
        // Work is charged to IDs removed once or IDs explicitly encoded in keep.
        // Settled/dispatched/unknown history is never scanned on reset.
        for &id in &self.reserved {
            if keep.binary_search(&id).is_err() {
                increase(&mut refund, self.row(id, at)?.units, at)?;
                increase(&mut count, 1, at)?;
            }
        }
        let stats = &mut self.accounting;
        increase(&mut stats.available_units, refund, at)?;
        decrease(&mut stats.reserved_units, refund, at)?;
        decrease(&mut stats.reserved_count, count, at)?;
        increase(&mut stats.aborted_count, count, at)?;
        increase(&mut stats.incident_count, 1, at)?;
        let rows = &mut self.rows;
        let mut consistent = true;
        self.reserved.retain(|id| {
            if keep.binary_search(id).is_ok() {
                return true;
            }
            match rows.get_mut(id) {
                Some(row) if row.state == State::Reserved => {
                    row.state = State::Aborted;
                    row.effect = None;
                }
                _ => consistent = false,
            }
            false
        });
        if !consistent {
            return Err(VerifyError::at(ErrorKind::ConservationFailure, at, "reserved index disagrees with reset accounting"));
        }
        // Reset does not change epoch or any dispatched/unknown liability.
        Ok(())
    }

    fn check_conservation(&self, at: Location) -> Result<(), VerifyError> {
        let stats = &self.accounting;
        let total = [
            stats.available_units, stats.spent_units, stats.reserved_units,
            stats.dispatched_units, stats.unknown_units,
        ].into_iter().try_fold(0_u64, |sum, value| sum.checked_add(value));
        let count = [
            stats.reserved_count, stats.dispatched_count, stats.unknown_count,
            stats.committed_count, stats.aborted_count,
        ].into_iter().try_fold(0_u64, |sum, value| sum.checked_add(value));
        if total != Some(stats.total_units)
            || count != Some(stats.reservations)
            || stats.reservations != self.rows.len() as u64
            || stats.reserved_count != self.reserved.len() as u64
        {
            return Err(VerifyError::at(ErrorKind::ConservationFailure, at, "rights units or reservation counts are not conserved"));
        }
        Ok(())
    }

    pub fn accounting(self) -> Accounting {
        self.accounting
    }

    #[cfg(test)]
    pub fn retained_effect_bytes(&self) -> usize {
        self.rows.values().filter_map(|row| row.effect.as_ref()).map(|bytes| bytes.len()).sum()
    }
}
