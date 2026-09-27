//! Compose the original learned generator with the original guarded lifecycle.
//! Independently supplied requirements never come from the journal being opened.
//! No numerical, authority, storage or role-provisioning algorithm is replaced.

use super::{BaseEvent, Event, FileCredentialRegistration, FileGuardSet, FileOversight,
    FileOversightProfile, FileOversightRoles, FileRecoveryRequirements, JournalError,
    Machine, bootstrap::PreparedGuardedBootstrap, journal, storage};
use super::super::decoder::learned::{FileLearnedConfig, bind_history};
use crate::Error;
use std::path::Path;

impl FileOversight {
    /// Publish ALL declared guards and the exact learned recipe in the FIRST
    /// canonical image. The original bootstrap resolves any credential route;
    /// caller assertions cannot enable a credential. No observation, inference,
    /// source freshness, identity success or effect key is manufactured here.
    /// Native text configurations preserve their original output-only binding.
    pub fn create_guarded_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>,
        config: FileLearnedConfig,
    ) -> Result<(Self, FileOversightRoles), JournalError> {
        check_profile(guards)?;
        let prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?
            .learned(config)?;
        let store = storage::Store::create(directory.as_ref())?;
        prepared.publish(store)
    }

    /// Recover ONE composed owner and all its separately provisioned roles.
    /// Exact learned parameters are bound before numerical replay. The original
    /// guard inventory, current policy, credential state and independent counter
    /// floors must all match before cleanup, recovery writes or role release.
    /// Success appends exactly the original fence: numerical state is paused,
    /// old keys are withdrawn, and unknown-effect liabilities are not refunded.
    ///
    /// The counter floors do not distinguish equal-counter forks. Evaluated,
    /// predictive and mediated extensions are deliberately not admitted by this
    /// base profile; the original inventory checks refuse them, not strip them.
    pub fn open_guarded_with_learned_generation(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileRecoveryRequirements,
        config: &FileLearnedConfig,
    ) -> Result<(Self, FileOversightRoles), JournalError> {
        profile.delivery.limits.check()?;
        check_profile(&expected.guards)?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        recover(store, profile, events, expected, config)
    }
}

pub(super) fn check_profile(guards: &FileGuardSet) -> Result<(), Error> {
    guards.validate()?;
    // These are distinct original engines, not interchangeable configurations.
    // Reject a contradictory bootstrap before creating a directory or running
    // either decoder. Learned text is part of FileLearnedConfig, not this slot.
    if guards.decoder.is_some() || guards.decoder_stop.is_some() {
        return Err(Error::Binding);
    }
    Ok(())
}

// Also used by the anchored entry point after exact prefix comparison. Parsed
// events remain local data. There is no public callback, hydrated configuration,
// prepared-machine accessor or role getter that could bypass these checks.
pub(super) fn recover(
    store: storage::Store,
    profile: FileOversightProfile,
    mut events: Vec<Event>,
    expected: &FileRecoveryRequirements,
    config: &FileLearnedConfig,
) -> Result<(FileOversight, FileOversightRoles), JournalError> {
    check_profile(&expected.guards)?;
    expected.guards.check_decoder_config(&events)?;
    bind_history(&mut events, config)?;
    let machine = Machine::replay(&profile, &events)?;
    if machine.learned_contract() != Some(config) {
        return Err(Error::Binding.into());
    }
    expected.check(&profile, &machine, &events)?;
    store.confirm_and_cleanup()?;
    let (mut host, human) = FileOversight::owner(profile, store, events, machine);
    host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
    let roles = FileOversightRoles::provision(&host, human);
    Ok((host, roles))
}
