//! Publish the complete configured guard set in the FIRST canonical image.
//! No intermediate unguarded owner, callback, effect, or new journal format.

use super::{FileGuardSet, FileOversightRoles};
use super::super::credential::FileCredentialPolicy;
use super::super::decoder::DecoderEvent;
use super::super::governance::campaigns::CampaignEvent;
use super::super::identity::IdentityEvent;
use super::super::source::SourceEvent;
use super::super::{Event, FileOversight, FileOversightProfile, JournalError, Machine, journal, storage};
use crate::action::consequence::delivery::credential_broker::BrokerRouteBinding;
use crate::perimeter_inventory::LoadedPerimeterInventory;
use crate::Error;
use std::path::Path;
use std::rc::Rc;

/// Independently loaded perimeter data. The ORIGINAL credential resolver must
/// approve this route; a FileCredentialPolicy assertion alone cannot enable it.
/// This borrows nonsecret registration data, not a broker/provider credential.
pub struct FileCredentialRegistration<'a> {
    pub inventory: &'a LoadedPerimeterInventory,
    pub binding: &'a BrokerRouteBinding,
}

// Kept inside the guarded implementation for actual Store fault tests. There is
// no public prepared-image constructor or caller-supplied replay projection.
pub(super) struct PreparedGuardedBootstrap {
    profile: FileOversightProfile,
    events: Vec<Event>,
    machine: Machine,
}

impl PreparedGuardedBootstrap {
    pub(super) fn prepare(profile: FileOversightProfile, guards: &FileGuardSet,
        registration: Option<FileCredentialRegistration<'_>>) -> Result<Self, JournalError>
    {
        profile.delivery.limits.check()?;
        guards.validate()?;
        let credential = match (&guards.credential, registration) {
            (None, None) => None,
            (Some(expected), Some(registration)) => {
                let actual = FileCredentialPolicy::resolve(registration.inventory, registration.binding,
                    profile.delivery.scope, profile.delivery.target)?;
                if &actual != expected { return Err(Error::Binding.into()); }
                Some(actual)
            }
            _ => return Err(Error::InvalidInput.into()),
        };
        // Stream bootstrap must be first. It itself enables publication checks;
        // ordinary publication receives the same original guard explicitly.
        let mut events = vec![match guards.stream {
            Some(stream) => Event::StreamBootstrap(stream),
            None => Event::PublicationGuard,
        }];
        if let Some(source) = guards.source { events.push(Event::Source(SourceEvent::Enable(source))); }
        if let Some(identity) = &guards.identity {
            events.push(Event::Identity(IdentityEvent::Enable(Rc::new(identity.passport.clone()), identity.policy)));
        }
        if let Some(campaigns) = guards.campaigns {
            events.push(Event::Campaign(CampaignEvent::Enable(campaigns.limits, campaigns.max_campaigns)));
        }
        if let Some(credential) = credential { events.push(Event::CredentialGuard(credential)); }
        if let Some(config) = &guards.decoder {
            events.push(Event::Decoder(DecoderEvent::Enable(Rc::new(config.clone()))));
        }
        if let Some(policy) = guards.decoder_stop {
            events.push(Event::Decoder(DecoderEvent::StopPolicy(policy)));
        }
        // Validate the SAME native transition sequence before creating storage.
        // No profile is silently dropped, reordered after work, or downgraded.
        let machine = Machine::replay(&profile, &events)?;
        guards.check(&machine, &events)?;
        Ok(Self { profile, events, machine })
    }

    pub(super) fn publish(self, store: storage::Store)
        -> Result<(FileOversight, FileOversightRoles), JournalError>
    {
        let bytes = journal::encode(&self.profile, store.identity(), &self.events)?;
        // The only canonical replacement. A failed or ambiguous write returns
        // neither owner nor roles. Recovery validates the actual canonical image.
        store.replace(&bytes)?;
        let (host, human) = FileOversight::owner(self.profile, store, self.events, self.machine);
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }
}

impl FileOversight {
    /// Validate every configured native gate, then publish them together before
    /// exposing ANY role or owner. No current source, clock, identity match,
    /// campaign approval, effect key or provider secret is restored or invented.
    /// Use the original interfaces to supply them separately after bootstrap.
    /// Existing directories are refused; this is not an in-place upgrade API.
    pub fn create_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        guards: &FileGuardSet, registration: Option<FileCredentialRegistration<'_>>)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        let prepared = PreparedGuardedBootstrap::prepare(profile, guards, registration)?;
        let store = storage::Store::create(directory.as_ref())?;
        prepared.publish(store)
    }
}
