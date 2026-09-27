//! Independently anchored recovery and read-only inspection of learned owners.
//! Anchors bind history; the original recipe and control checks remain separate.

use super::{FileHistoryAnchor, FileOversight, FileOversightProfile, FileOversightRoles,
    FileRecoveryRequirements, JournalError, journal, storage};
use super::super::learned::{check_profile, reconstruct, recover};
use super::super::super::decoder::learned::FileLearnedConfig;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use std::path::Path;

impl FileOversight {
    /// Compare the ORIGINAL canonical history prefix against an independently
    /// retained anchor before any numerical replay, cleanup, fence or role release.
    /// Genuine append-only successors are accepted; equal-counter forks are not.
    /// The same read image supplies the prefix and all suffix verification.
    /// Original guard/recipe/floor checks and exactly one original fence still
    /// apply. An anchor does not restore freshness, old keys or spent allowances.
    pub fn open_guarded_anchored_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileRecoveryRequirements,
        config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<(Self, FileOversightRoles), JournalError> {
        profile.delivery.limits.check()?;
        check_profile(&expected.guards)?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        anchor.check(&profile, store.identity(), &events)?;
        recover(store, profile, events, expected, config)
    }

    /// Verify one immutable canonical image without taking ownership, cleaning
    /// staging files, appending a fence, updating the anchor or issuing any role.
    /// This can inspect a journal while its cooperating writer is alive. It is
    /// historical data at the read cut, not a live clock/source eligibility claim.
    /// Neither an anchored prefix nor a parseable suffix bypasses original replay.
    ///
    /// The path and storage remain operator-controlled assumptions. Replacing
    /// both the journal and its independent anchor defeats rollback protection.
    /// No remote effect runs during the original RAM-only reconstruction.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::Permit;
    /// use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
    /// fn authorize_history(snapshot: FileDeliverySnapshot) -> Permit { snapshot }
    /// ```
    pub fn read_guarded_anchored_learned_publication(
        directory: impl AsRef<Path>,
        profile: &FileOversightProfile,
        expected: &FileRecoveryRequirements,
        config: &FileLearnedConfig,
        anchor: &FileHistoryAnchor,
    ) -> Result<FileDeliverySnapshot, JournalError> {
        super::super::super::super::codec::validate_profile(&profile.delivery)?;
        check_profile(&expected.guards)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        anchor.check(profile, &identity, &events)?;
        let machine = reconstruct(profile, &mut events, expected, config)?;
        Ok(machine.snapshot(events.len()))
    }
}
