//! Operator-owned, crash-recoverable publication of the existing evidence format.
//!
//! Reuse the original private-directory lock and replace/fsync protocol. This
//! store is an evidence source, NOT a second effect ledger. Readers use the
//! unchanged sealed FileEvidenceSource and the original durable source gate.

use super::super::super::{JournalError, storage};
use crate::action::{Purpose, Scope};
use crate::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
};
use crate::Error;
use std::fmt;
use std::path::Path;

/// Independently supplied operator contract. The minimum generation must come
/// from outside this directory to detect rollback of an otherwise valid image.
/// It is not a cryptographic anti-rollback mechanism or a persisted authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidencePublisherProfile {
    pub source: u64,
    pub scope: Scope,
    pub minimum_generation: u64,
    pub max_bytes: usize,
}

impl EvidencePublisherProfile {
    fn check(self) -> Result<(), Error> {
        self.scope.validate()?;
        if self.scope.purpose != Purpose::Effect {
            return Err(Error::Binding);
        }
        if self.source == 0 || self.minimum_generation == 0 || self.max_bytes == 0 {
            return Err(Error::InvalidInput);
        }
        if self.max_bytes > MAX_EVIDENCE_FILE_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }

    fn check_snapshot(self, snapshot: &EvidenceSnapshot) -> Result<(), Error> {
        let identity = snapshot.identity();
        if identity.scope != self.scope || identity.source != self.source {
            return Err(Error::Binding);
        }
        if identity.generation < self.minimum_generation {
            return Err(Error::Stale);
        }
        Ok(())
    }

    fn encode(self, snapshot: &EvidenceSnapshot) -> Result<Vec<u8>, Error> {
        self.check_snapshot(snapshot)?;
        let bytes = snapshot.encode();
        if bytes.len() > self.max_bytes {
            return Err(Error::Limit);
        }
        Ok(bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidencePublicationKind {
    Created,
    Replaced,
    AlreadyCurrent,
}

/// Acknowledged local storage result, never an effect permit, observation lease,
/// authenticated source statement or assertion that helpers accepted the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidencePublication {
    pub identity: EvidenceIdentity,
    pub semantic_epoch: u64,
    pub encoded_bytes: usize,
    pub kind: EvidencePublicationKind,
}

/// One cooperating producer of one original-format evidence file. Mutation
/// requires the exact predecessor and next generation; an exact retry does not
/// create a new version. Every I/O failure poisons this owner until reopen.
///
/// The operator must protect the directory and ancestors from noncooperating
/// writers. The lock is advisory. No full-host compromise or universal filesystem
/// atomicity claim is made. Readers still verify identity, bounds and versions.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::source::publisher::FileEvidencePublisher;
/// fn duplicate(owner: FileEvidencePublisher) { let _ = owner.clone(); }
/// ```
pub struct FileEvidencePublisher {
    profile: EvidencePublisherProfile,
    store: storage::Store,
    current: EvidenceSnapshot,
    current_bytes: Vec<u8>,
    fault: Option<JournalError>,
}

impl fmt::Debug for FileEvidencePublisher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileEvidencePublisher")
            .field("identity", &self.current.identity())
            .field("fault", &self.fault)
            .finish_non_exhaustive()
    }
}

impl FileEvidencePublisher {
    /// Validate the entire initial image before creating anything. The directory
    /// must not exist; this cannot overwrite or adopt an existing producer.
    pub fn create(
        directory: impl AsRef<Path>,
        profile: EvidencePublisherProfile,
        initial: EvidenceSnapshot,
    ) -> Result<(Self, EvidencePublication), JournalError> {
        profile.check()?;
        let bytes = profile.encode(&initial)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&bytes)?;
        let owner = Self { profile, store, current: initial, current_bytes: bytes, fault: None };
        let result = owner.publication(EvidencePublicationKind::Created);
        Ok((owner, result))
    }

    /// Acquire the original lock, validate the canonical image against the
    /// independently supplied source/scope/floor, then confirm durability and
    /// discard staged leftovers. Pending bytes are NEVER promoted as evidence.
    /// A malformed/missing/rolled-back canonical image fails before cleanup.
    pub fn open(
        directory: impl AsRef<Path>, profile: EvidencePublisherProfile,
    ) -> Result<Self, JournalError> {
        profile.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let current_bytes = store.read(profile.max_bytes)?;
        let current = EvidenceSnapshot::decode(&current_bytes)?;
        profile.check_snapshot(&current)?;
        store.confirm_and_cleanup()?;
        Ok(Self { profile, store, current, current_bytes, fault: None })
    }

    /// Last acknowledged (or explicitly reopened) data, not a fresh file read.
    /// After an ambiguous failure it may differ from the currently visible file.
    pub fn snapshot(&self) -> &EvidenceSnapshot { &self.current }
    pub fn failure(&self) -> Option<&JournalError> { self.fault.as_ref() }

    /// The existing concrete reader reopens the canonical path on EVERY read.
    /// This neither transfers the producer lock nor grants an observer authority.
    pub fn reader(&self) -> Result<FileEvidenceSource, JournalError> {
        self.ensure_live()?;
        Ok(FileEvidenceSource::new(
            self.store.identity().join(storage::CANONICAL),
            self.profile.source, self.profile.scope, self.profile.max_bytes,
        )?)
    }

    /// Persist a whole immutable version. Incomplete snapshots are valid source
    /// observations: the ORIGINAL consumer holds them, never fills their gaps.
    /// A metadata result is returned only after file and directory barriers.
    pub fn publish(
        &mut self, expected_generation: u64, next: EvidenceSnapshot,
    ) -> Result<EvidencePublication, JournalError> {
        self.ensure_live()?;
        self.profile.check_snapshot(&next)?;
        let next_generation = expected_generation.checked_add(1).ok_or(Error::Overflow)?;
        if next.identity().generation != next_generation {
            return Err(Error::Stale.into());
        }
        let retry = next_generation == self.current.identity().generation;
        if retry && next != self.current {
            return Err(Error::Binding.into());
        }
        if !retry && expected_generation != self.current.identity().generation {
            return Err(Error::Stale.into());
        }
        if next.snapshot().semantic_epoch < self.current.snapshot().semantic_epoch {
            return Err(Error::Stale.into());
        }
        let bytes = self.profile.encode(&next)?;
        // Detect observed out-of-band replacement instead of overwriting it or
        // acknowledging a retry from our stale in-memory image. This read is not
        // a defense against a hostile writer racing inside a private directory.
        let observed = match self.store.read(self.profile.max_bytes) {
            Ok(bytes) => bytes,
            Err(error) => return self.fail(error),
        };
        if observed != self.current_bytes {
            return self.fail(Error::Binding.into());
        }
        if retry {
            return Ok(self.publication(EvidencePublicationKind::AlreadyCurrent));
        }
        if let Err(error) = self.store.replace(&bytes) {
            return self.fail(error);
        }
        self.current = next;
        self.current_bytes = bytes;
        Ok(self.publication(EvidencePublicationKind::Replaced))
    }

    fn ensure_live(&self) -> Result<(), JournalError> {
        if self.fault.is_some() { Err(JournalError::Unavailable) } else { Ok(()) }
    }

    fn fail<T>(&mut self, error: JournalError) -> Result<T, JournalError> {
        self.fault = Some(error.clone());
        Err(error)
    }

    fn publication(&self, kind: EvidencePublicationKind) -> EvidencePublication {
        EvidencePublication { identity: self.current.identity(),
            semantic_epoch: self.current.snapshot().semantic_epoch,
            encoded_bytes: self.current_bytes.len(), kind }
    }
}

#[cfg(test)]
mod tests;
