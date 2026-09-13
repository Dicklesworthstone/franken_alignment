//! Reserve logical journal space for original fencing, stopping and draining.
//! This is admission of journal bytes, not reserved disk space or new authority.
use super::{Event, FileDelivery, JournalError, JournalLimits, MAX_JOURNAL_BYTES, MAX_JOURNAL_EVENTS};
use crate::Error;

/// The full-input format needs 6 + 30 + 14 encoded bytes for one Fence, Stop
/// and StopProgress respectively, including their record framing. The simpler
/// format needs three fewer bytes. Neither operation serializes result objects.
pub const TERMINAL_RECOVERY_EVENTS: usize = 3;
pub const TERMINAL_RECOVERY_BYTES: usize = 50;

/// Install explicitly before work. Existing unconfigured journals keep their
/// original limits/bytes. Once installed this reserve cannot shrink or disappear.
/// It is a finite logical allowance, not a promise of unlimited restart attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryReserve {
    pub events: usize,
    pub bytes: usize,
}
impl RecoveryReserve {
    /// One reopening fence, local stop and terminal sweep fit after a last
    /// ordinary admission. Storage, clock and endpoint evidence can still fail.
    pub const fn terminal() -> Self {
        Self { events: TERMINAL_RECOVERY_EVENTS, bytes: TERMINAL_RECOVERY_BYTES }
    }
    fn validate(self, limits: JournalLimits) -> Result<(), Error> {
        limits.check()?;
        if self.events < TERMINAL_RECOVERY_EVENTS || self.bytes < TERMINAL_RECOVERY_BYTES {
            return Err(Error::InvalidInput);
        }
        if self.events >= limits.events || self.bytes >= limits.bytes
            || self.events > MAX_JOURNAL_EVENTS || self.bytes > MAX_JOURNAL_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalSpace { pub events: usize, pub bytes: usize }

/// Snapshot of acknowledged logical capacity for the privileged supervisor.
/// It does not expose a permit, an endpoint receipt or a disk-free-space claim.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{FilePermit, JournalCapacity};
/// fn authorize(space: JournalCapacity) -> FilePermit { space }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalCapacity {
    limits: JournalLimits,
    reserve: Option<RecoveryReserve>,
    used: JournalSpace,
}
impl JournalCapacity {
    pub fn limits(&self) -> JournalLimits { self.limits }
    pub fn reserve(&self) -> Option<RecoveryReserve> { self.reserve }
    pub fn used(&self) -> JournalSpace { self.used }
    pub fn remaining(&self) -> JournalSpace {
        JournalSpace { events: self.limits.events.saturating_sub(self.used.events),
            bytes: self.limits.bytes.saturating_sub(self.used.bytes) }
    }
    /// Includes ALL previously acknowledged events, even earlier recovery work.
    /// Spending recovery space never replenishes ordinary admission capacity.
    pub fn ordinary_remaining(&self) -> JournalSpace {
        let reserve = self.reserve.unwrap_or(RecoveryReserve { events: 0, bytes: 0 });
        JournalSpace {
            events: (self.limits.events - reserve.events).saturating_sub(self.used.events),
            bytes: (self.limits.bytes - reserve.bytes).saturating_sub(self.used.bytes),
        }
    }
    pub fn terminal_space_remaining(&self) -> bool {
        let remaining = self.remaining();
        remaining.events >= TERMINAL_RECOVERY_EVENTS && remaining.bytes >= TERMINAL_RECOVERY_BYTES
    }
    pub(super) fn new(limits: JournalLimits, reserve: Option<RecoveryReserve>, events: usize, bytes: usize) -> Self {
        Self { limits, reserve, used: JournalSpace { events, bytes } }
    }
}

/// Only these journal transitions can spend the tail: a recovery fence, terminal
/// stop, and its endpoint sweep. New effects, policy edits, helper votes, human
/// approvals, routine clock updates and even ordinary queries cannot spend it.
#[derive(Clone, Copy)]
pub(super) enum Class { Bootstrap, Work, Install(RecoveryReserve), Recovery }
pub(super) fn class(event: &Event) -> Class {
    match event {
        Event::ReserveRecovery(reserve) => Class::Install(*reserve),
        Event::Time(_) => Class::Bootstrap,
        Event::Fence | Event::Stop(_) | Event::StopProgress(_) => Class::Recovery,
        _ => Class::Work,
    }
}

/// Shared by both canonical encoders. Decoder canonicalization runs this same
/// PREFIX check, so an imported history cannot use a recovery tail for new work.
/// The accounting includes bootstrap, record framing and every earlier event.
pub(super) struct Admission {
    limits: JournalLimits,
    reserve: Option<RecoveryReserve>,
    work_started: bool,
}
impl Admission {
    pub(super) fn new(limits: JournalLimits) -> Self { Self { limits, reserve: None, work_started: false } }
    pub(super) fn record(&mut self, class: Class, events: usize, bytes: usize) -> Result<(), Error> {
        if let Class::Install(reserve) = class {
            if self.reserve.is_some() { return Err(Error::Duplicate); }
            if self.work_started { return Err(Error::WrongState); }
            reserve.validate(self.limits)?;
            self.reserve = Some(reserve);
        }
        if matches!(class, Class::Work | Class::Recovery) { self.work_started = true; }
        let (event_limit, byte_limit) = match (self.reserve, class) {
            (Some(reserve), Class::Bootstrap | Class::Work | Class::Install(_)) =>
                (self.limits.events - reserve.events, self.limits.bytes - reserve.bytes),
            _ => (self.limits.events, self.limits.bytes),
        };
        if events > event_limit || bytes > byte_limit { return Err(Error::Limit); }
        Ok(())
    }
}

impl FileDelivery {
    /// Installation is a real durable input. The canonical encoder refuses
    /// repeated/late installation and verifies that the installation ITSELF
    /// leaves the entire declared reserve. No existing bootstrap bytes change.
    pub fn enable_recovery_reserve(&mut self, revision: u64, reserve: RecoveryReserve) -> Result<(), JournalError> {
        self.transact(revision, Event::ReserveRecovery(reserve))?;
        Ok(())
    }
    /// Bounded re-encoding of acknowledged history, not a fresh filesystem read.
    /// Ambiguous storage makes this unavailable rather than exposing old space.
    pub fn journal_capacity(&self) -> Result<JournalCapacity, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let bytes = super::codec::encode(&self.profile, self.store.identity(), &self.events)?.len();
        let reserve = self.events.iter().find_map(|event| match event {
            Event::ReserveRecovery(reserve) => Some(*reserve), _ => None,
        });
        Ok(JournalCapacity::new(self.profile.limits, reserve, self.events.len(), bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_dimensions_include_bootstrap_and_recovery_never_replenishes_work() {
        let limits = JournalLimits { events: 8, bytes: 200 };
        let mut admission = Admission::new(limits);
        admission.record(Class::Bootstrap, 1, 100).unwrap();
        admission.record(Class::Install(RecoveryReserve::terminal()), 2, 117).unwrap();
        admission.record(Class::Work, 5, 150).unwrap();
        assert_eq!(admission.record(Class::Work, 6, 151), Err(Error::Limit));
        admission.record(Class::Recovery, 6, 156).unwrap();
        assert_eq!(admission.record(Class::Bootstrap, 7, 169), Err(Error::Limit));
        admission.record(Class::Recovery, 7, 186).unwrap();
        admission.record(Class::Recovery, 8, 200).unwrap();
        assert_eq!(admission.record(Class::Recovery, 9, 201), Err(Error::Limit));
        let mut bytes = Admission::new(limits);
        bytes.record(Class::Install(RecoveryReserve::terminal()), 1, 120).unwrap();
        assert_eq!(bytes.record(Class::Work, 2, 151), Err(Error::Limit));
        bytes.record(Class::Recovery, 2, 151).unwrap();
    }
    #[test]
    fn installing_after_work_or_recovery_cannot_reinterpret_the_prefix() {
        for class in [Class::Work, Class::Recovery] {
            let mut admission = Admission::new(JournalLimits::default());
            admission.record(class, 1, 100).unwrap();
            assert_eq!(admission.record(Class::Install(RecoveryReserve::terminal()), 2, 117), Err(Error::WrongState));
        }
        let mut admission = Admission::new(JournalLimits::default());
        admission.record(Class::Install(RecoveryReserve::terminal()), 1, 100).unwrap();
        assert_eq!(admission.record(Class::Install(RecoveryReserve::terminal()), 2, 117), Err(Error::Duplicate));
    }
}
