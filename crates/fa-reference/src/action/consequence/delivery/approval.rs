//! The consumed second key's execution window, not another live permit.
//!
//! Plan 9.11 / FA-121: an endpoint must not execute a delayed first delivery
//! after either key's deadline. The original frozen action stays unchanged.
//! Numeric reviewer/request identities are audit data, not authentication.

use crate::action::ElapsedTick;
use crate::Error;

/// Immutable, execution-bearing evidence of a separately approved key.
/// Copying this data cannot create a dispatch envelope or mint either permit.
/// The human's packet, helper votes and private review context are not exposed.
///
/// ```compile_fail
/// use fa_reference::action::consequence::delivery::DispatchApproval;
/// fn forge() -> DispatchApproval { DispatchApproval {} }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchApproval {
    request: u64,
    reviewer: u64,
    issued_at: ElapsedTick,
    expires_at: ElapsedTick,
}

impl DispatchApproval {
    pub(crate) fn new(
        request: u64, reviewer: u64, issued_at: ElapsedTick, expires_at: ElapsedTick,
    ) -> Result<Self, Error> {
        if request == 0 || reviewer == 0 || issued_at >= expires_at {
            return Err(Error::InvalidInput);
        }
        Ok(Self { request, reviewer, issued_at, expires_at })
    }

    pub fn request(&self) -> u64 { self.request }
    pub fn reviewer(&self) -> u64 { self.reviewer }
    pub fn issued_at(&self) -> ElapsedTick { self.issued_at }
    pub fn expires_at(&self) -> ElapsedTick { self.expires_at }

    pub(super) fn validate_at(&self, now: ElapsedTick, action_deadline: ElapsedTick) -> Result<(), Error> {
        if self.expires_at > action_deadline { return Err(Error::InvalidInput); }
        if now < self.issued_at || now >= self.expires_at { return Err(Error::Stale); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_window_is_half_open_and_cannot_extend_the_frozen_action() {
        let approval = DispatchApproval::new(1, 2, ElapsedTick(3), ElapsedTick(5)).unwrap();
        assert_eq!(approval.validate_at(ElapsedTick(2), ElapsedTick(10)), Err(Error::Stale));
        assert_eq!(approval.validate_at(ElapsedTick(3), ElapsedTick(10)), Ok(()));
        assert_eq!(approval.validate_at(ElapsedTick(4), ElapsedTick(5)), Ok(()));
        assert_eq!(approval.validate_at(ElapsedTick(5), ElapsedTick(10)), Err(Error::Stale));
        assert_eq!(approval.validate_at(ElapsedTick(3), ElapsedTick(4)), Err(Error::InvalidInput));
        for (request, reviewer, start, end) in [(0, 2, 3, 5), (1, 0, 3, 5), (1, 2, 5, 5), (1, 2, 6, 5)] {
            assert_eq!(DispatchApproval::new(request, reviewer, ElapsedTick(start), ElapsedTick(end)), Err(Error::InvalidInput));
        }
        let last = DispatchApproval::new(1, 2, ElapsedTick(u64::MAX - 1), ElapsedTick(u64::MAX)).unwrap();
        assert_eq!(last.validate_at(ElapsedTick(u64::MAX - 1), ElapsedTick(u64::MAX)), Ok(()));
        assert_eq!(last.validate_at(ElapsedTick(u64::MAX), ElapsedTick(u64::MAX)), Err(Error::Stale));
    }
}
