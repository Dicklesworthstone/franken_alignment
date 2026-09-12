//! Freshness of a complete captured prefix in the controller's elapsed domain.
//! Observation time is asserted by the separately provisioned adapter, not by
//! the actor. A closure cannot be renewed by changing its timestamp or marker.

use super::{CapturedSnapshot, PolicyStateCapture, PolicyStateWriter, StateFrontier,
    StateLimits, StateSource, capture_closed, close_state};
use crate::action::ElapsedTick;
use crate::{Error, Snapshot};

/// Immutable bootstrap policy. No setter can widen it on a live source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateFreshness { max_age_ticks: u64 }

impl StateFreshness {
    pub fn new(max_age_ticks: u64) -> Result<Self, Error> {
        if max_age_ticks == 0 { return Err(Error::InvalidInput); }
        Ok(Self { max_age_ticks })
    }
    pub fn max_age_ticks(self) -> u64 { self.max_age_ticks }
}

/// Immutable evidence of one adapter closure's time bound, never a permit.
/// It remains historical after expiry, withdrawal, replacement or writer loss.
///
/// ```compile_fail,E0451
/// use fa_reference::action::consequence::oversight::policy_state::ObservationLease;
/// fn forge() -> ObservationLease { ObservationLease {} }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationLease {
    frontier: StateFrontier,
    observed_at: ElapsedTick,
    expires_at: ElapsedTick,
}

impl ObservationLease {
    pub fn frontier(self) -> StateFrontier { self.frontier }
    pub fn observed_at(self) -> ElapsedTick { self.observed_at }
    pub fn expires_at(self) -> ElapsedTick { self.expires_at }
    pub fn validate_at(self, now: ElapsedTick) -> Result<(), Error> {
        if now < self.observed_at || now >= self.expires_at { return Err(Error::Stale); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FreshnessState {
    policy: StateFreshness,
    observed_floor: Option<ElapsedTick>,
    checked_floor: Option<ElapsedTick>,
    last_grant: Option<ObservationLease>,
}

impl PolicyStateCapture {
    pub fn new_with_freshness(
        source: StateSource, limits: StateLimits, policy: StateFreshness,
    ) -> Result<(Self, PolicyStateWriter), Error> {
        let (capture, writer) = Self::new(source, limits)?;
        capture.state.borrow_mut().freshness = Some(FreshnessState {
            policy, observed_floor: None, checked_floor: None, last_grant: None,
        });
        Ok((capture, writer))
    }

    pub fn freshness_policy(&self) -> Option<StateFreshness> {
        self.state.borrow().freshness.map(|state| state.policy)
    }

    /// Check the LIVE source and half-open lease interval. The consumer's time
    /// floor advances even on expiry or unavailable evidence; a later backdated
    /// read cannot resurrect a formerly usable cut. Untimed sources are unchanged.
    pub fn capture_at(&self, now: ElapsedTick) -> Result<CapturedSnapshot, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if let Some(freshness) = &mut state.freshness {
            if freshness.checked_floor.is_some_and(|floor| now < floor) { return Err(Error::Stale); }
            freshness.checked_floor = Some(now);
        }
        let captured = capture_closed(&state)?;
        if state.freshness.is_some() {
            captured.lease.ok_or(Error::Incomplete)?.validate_at(now)?;
        }
        Ok(captured)
    }

    pub fn validate_at(&self, supplied: &Snapshot, now: ElapsedTick) -> Result<CapturedSnapshot, Error> {
        let cut = self.capture_at(now)?;
        if cut.snapshot() != supplied { return Err(Error::Binding); }
        Ok(cut)
    }

    /// Used only by the original broker's governed source replacement. Preserve
    /// both time floors and the fixed age policy, but not any old eligibility.
    pub(crate) fn replacement(
        &self, source: StateSource, limits: StateLimits,
    ) -> Result<(Self, PolicyStateWriter), Error> {
        let freshness = self.state.try_borrow().map_err(|_| Error::WrongState)?.freshness;
        let (capture, writer) = Self::new(source, limits)?;
        capture.state.borrow_mut().freshness = freshness.map(|old| FreshnessState {
            last_grant: None, ..old
        });
        Ok((capture, writer))
    }
}

impl PolicyStateWriter {
    /// Close a newly observed complete prefix under the fixed bootstrap age.
    /// The host supplies the time of the underlying observation, not completion
    /// of unrelated processing. Every renewal needs a NEW sequence and marker;
    /// re-reading unchanged state may be represented by a new full Snapshot.
    /// Source authentication and fidelity of that observation remain host duties.
    pub fn close_observed(
        &self, through: u64, marker_generation: u64, observed_at: ElapsedTick,
    ) -> Result<StateFrontier, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if state.fault.is_some() { return Err(Error::WrongState); }
        let freshness = state.freshness.ok_or(Error::Incomplete)?;
        let frontier = StateFrontier { source: state.source, through, marker_generation };
        let lease = ObservationLease {
            frontier, observed_at,
            expires_at: ElapsedTick(observed_at.0.checked_add(freshness.policy.max_age_ticks)
                .ok_or(Error::Overflow)?),
        };
        if let Some(previous) = state.closed.as_ref().filter(|cut| cut.frontier == frontier) {
            return if previous.lease == Some(lease) { Ok(frontier) } else { Err(Error::Binding) };
        }
        if freshness.observed_floor.is_some_and(|floor| observed_at < floor)
            || freshness.last_grant.is_some_and(|previous| through <= previous.frontier.through)
        { return Err(Error::Stale); }
        let result = close_state(&mut state, through, marker_generation, Some(lease))?;
        let freshness = state.freshness.as_mut().expect("configured freshness");
        freshness.observed_floor = Some(observed_at);
        freshness.last_grant = Some(lease);
        Ok(result)
    }
}
