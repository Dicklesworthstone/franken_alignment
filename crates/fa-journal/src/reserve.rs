//! An independently pinned logical tail for withdrawal and settlement records.
//! This reserves journal capacity, not effect rights or physical storage.
use crate::{Error, Event, Head, Identity, Journal, Limits};
use crate::replay::Replay;
use std::io::Read;

pub(crate) const MAGIC: &[u8; 8] = b"FARJ\0\0\0\x02";
pub(crate) const HEADER_LEN: usize = 120;

/// Fixed at construction and included with ALL lifetime limits in the v2 header.
/// No live setter, widening on reopen, migration or inferred disk configuration.
/// Both dimensions apply independently; provision actual record sizes, not only
/// a record count. A RevokeEpoch occupies 21 bytes, a Reconcile occupies 30.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryReserve {
    pub records: u64,
    pub bytes: u64,
}

impl RecoveryReserve {
    pub(crate) fn check(self, limits: Limits) -> Result<(), Error> {
        if self.records == 0 || self.records > limits.max_records
            || self.records.checked_mul(21).is_none_or(|minimum| self.bytes < minimum)
            || (HEADER_LEN as u64).checked_add(self.bytes)
                .is_none_or(|minimum| minimum > limits.max_total_bytes)
        {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Remaining logical space, not a promise that any particular event is lawful.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemainingCapacity {
    pub records: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalCapacity {
    pub ordinary: RemainingCapacity,
    /// Total remaining space available to the fixed recovery-event allowlist.
    /// Includes remaining ordinary space; these are NOT additive balances.
    pub recovery: RemainingCapacity,
}

impl Journal {
    /// Explicit v2 profile. The first header binds identity, all lifetime limits
    /// and reserve. Legacy new/replay remain v1, with identical original bytes.
    /// Recovery must supply these exact values independently of the archive.
    pub fn new_with_recovery_reserve(identity: Identity, limits: Limits, reserve: RecoveryReserve)
        -> Result<Self, Error>
    {
        Self::configured(identity, limits, Some(reserve))
    }

    pub fn recovery_reserve(&self) -> Option<RecoveryReserve> {
        self.recovery_reserve
    }

    /// No balance can refill on revocation, cancellation, reset or settlement.
    /// All records, including recovery, monotonically consume the same total.
    pub fn capacity(&self) -> JournalCapacity {
        let reserve = self.recovery_reserve.unwrap_or(RecoveryReserve { records: 0, bytes: 0 });
        JournalCapacity {
            ordinary: RemainingCapacity {
                records: (self.limits.max_records - reserve.records).saturating_sub(self.head.sequence),
                bytes: (self.limits.max_total_bytes - reserve.bytes).saturating_sub(self.encoded_len),
            },
            recovery: RemainingCapacity {
                records: self.limits.max_records - self.head.sequence,
                bytes: self.limits.max_total_bytes - self.encoded_len,
            },
        }
    }

    /// Same strict framing/reducer as legacy replay, with the exact independently
    /// supplied v2 header. A header mismatch refuses before any event is applied.
    pub fn replay_with_recovery_reserve(reader: impl Read, identity: Identity, minimum: Head,
        limits: Limits, reserve: RecoveryReserve) -> Result<Self, Error>
    {
        Self::new_with_recovery_reserve(identity, limits, reserve)?.replay_from(reader, minimum)
    }

    /// Same private cooperative cursor; no partial ledger or larger replay cap.
    pub fn begin_replay_with_recovery_reserve<R: Read>(reader: R, identity: Identity, minimum: Head,
        limits: Limits, reserve: RecoveryReserve) -> Result<Replay<R>, Error>
    {
        Self::new_with_recovery_reserve(identity, limits, reserve)?.replay_cursor(reader, minimum)
    }

    pub(crate) fn remaining_for(&self, event: &Event) -> RemainingCapacity {
        let capacity = self.capacity();
        match event {
            Event::RevokeEpoch | Event::MarkUnknown { .. } | Event::AbortBeforeDispatch { .. }
                | Event::Reconcile { .. } => capacity.recovery,
            // Reset is not a tail operation. It keeps its ordinary bounds and
            // original reducer; no checkpoint can manufacture new journal space.
            Event::Reserve { .. } | Event::Dispatch { .. } | Event::ResetToCheckpoint { .. } => capacity.ordinary,
        }
    }

    // Used by single/grouped append AND both replay decoders after event decode.
    // Global framing/size limits are checked before payload allocation on replay.
    pub(crate) fn check_event_growth(&self, event: &Event, body_len: usize) -> Result<u64, Error> {
        let encoded_len = self.check_growth(body_len)?;
        let remaining = self.remaining_for(event);
        if remaining.records == 0 || encoded_len - self.encoded_len > remaining.bytes {
            return Err(Error::Limit);
        }
        Ok(encoded_len)
    }
}
