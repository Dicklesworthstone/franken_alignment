//! Exact, independently retained history, not a signature or an authority grant.
use super::Replay;
use crate::{Error, Head, Identity, Journal, Limits, RecoveryReserve};
use std::io::Read;
use std::sync::Arc;

/// A complete canonical prefix checked with the original decoder and reducer.
///
/// The caller must obtain these bytes independently of the journal being
/// recovered. Import validates structure and context, NOT who supplied them or
/// whether they are the newest acknowledged history. Joint rollback of both
/// copies remains undetectable. This evidence is freely cloneable; no rights or
/// mutable bytes are exposed. Storage is bounded by the selected journal limits.
#[derive(Clone, Debug)]
pub struct HistoryAnchor {
    bytes: Arc<[u8]>,
    head: Head,
    limits: Limits,
}

impl HistoryAnchor {
    /// Import a v1 prefix; reject partial records and invalid accounting before
    /// it can be selected for recovery. No file is opened or changed.
    pub fn from_bytes(bytes: Vec<u8>, identity: Identity, limits: Limits) -> Result<Self, Error> {
        Self::from_journal(bytes, Journal::new(identity, limits)?)
    }

    /// Import only the explicitly selected v2 profile. Never infer limits or a
    /// recovery reserve from the archive, or reinterpret a v1 header as v2.
    pub fn from_bytes_with_recovery_reserve(
        bytes: Vec<u8>,
        identity: Identity,
        limits: Limits,
        reserve: RecoveryReserve,
    ) -> Result<Self, Error> {
        Self::from_journal(bytes, Journal::new_with_recovery_reserve(identity, limits, reserve)?)
    }

    pub fn head(&self) -> Head {
        self.head
    }

    pub fn encoded_len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Original canonical bytes, not a serialization of reconstructed state.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn from_journal(bytes: Vec<u8>, journal: Journal) -> Result<Self, Error> {
        Self::checked(bytes, journal, None)
    }

    #[cfg(unix)]
    pub(crate) fn from_acknowledged(bytes: Vec<u8>, owner: &Journal) -> Result<Self, Error> {
        // Start at the original header, never from a clone of the live ledger.
        let initial = Journal::configured(owner.identity, owner.limits, owner.recovery_reserve)?;
        Self::checked(bytes, initial, Some(owner))
    }

    fn checked(bytes: Vec<u8>, journal: Journal, owner: Option<&Journal>) -> Result<Self, Error> {
        if bytes.len() as u64 > journal.limits.max_total_bytes {
            return Err(Error::Limit);
        }
        let minimum = journal.head();
        let checked = journal.replay_from(bytes.as_slice(), minimum)?;
        if owner.is_some_and(|owner| {
            checked.head() != owner.head()
                || checked.encoded_len() != owner.encoded_len()
                || checked.rights() != owner.rights()
        }) {
            return Err(Error::Binding);
        }
        Ok(Self { bytes: bytes.into(), head: checked.head(), limits: checked.limits })
    }

    pub(crate) fn check_context(&self, journal: &Journal) -> Result<(), Error> {
        // v1 does not encode lifetime limits; pin those independently as well.
        if self.limits.max_frame_bytes != journal.limits.max_frame_bytes
            || self.limits.max_records != journal.limits.max_records
            || self.limits.max_total_bytes != journal.limits.max_total_bytes
            || !self.bytes.starts_with(&journal.header())
        {
            return Err(Error::Binding);
        }
        Ok(())
    }

    pub(super) fn check_read(&self, offset: u64, bytes: &[u8]) -> Result<(), Error> {
        if offset >= self.encoded_len() {
            return Ok(());
        }
        // The comparison above proves offset fits usize, even on 32-bit hosts.
        let start = offset as usize;
        let count = bytes.len().min(self.bytes.len() - start);
        if bytes[..count] != self.bytes[start..start + count] {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

impl Journal {
    /// Replay the original history from byte zero, comparing the independently
    /// retained prefix in the SAME budgeted read path. A matching CRC/head alone
    /// cannot admit different bytes. Valid suffixes still replay in full.
    ///
    /// Context refusal occurs before the reader is called. Prefix storage is
    /// retained in addition to the ordinary one-frame replay buffer. Import work
    /// is separate; per-poll read and reducer accounting remains unchanged.
    pub fn begin_replay_anchored<R: Read>(
        reader: R,
        identity: Identity,
        anchor: HistoryAnchor,
        limits: Limits,
    ) -> Result<Replay<R>, Error> {
        Self::new(identity, limits)?.replay_cursor_anchored(reader, anchor)
    }

    pub fn begin_replay_anchored_with_recovery_reserve<R: Read>(
        reader: R,
        identity: Identity,
        anchor: HistoryAnchor,
        limits: Limits,
        reserve: RecoveryReserve,
    ) -> Result<Replay<R>, Error> {
        Self::new_with_recovery_reserve(identity, limits, reserve)?
            .replay_cursor_anchored(reader, anchor)
    }

    pub(crate) fn replay_cursor_anchored<R: Read>(
        self,
        reader: R,
        anchor: HistoryAnchor,
    ) -> Result<Replay<R>, Error> {
        anchor.check_context(&self)?;
        let mut replay = self.replay_cursor(reader, anchor.head())?;
        replay.exact_anchor = Some(anchor);
        Ok(replay)
    }
}
