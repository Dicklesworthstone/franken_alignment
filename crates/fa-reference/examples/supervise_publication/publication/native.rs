//! Project the existing explicit witness profile into the atomic native owner.
//! Parsing and receipt recovery never open a capture, producer or feed file.
use super::{PublicationProfile, FileOversightProfile, JournalError, Error};
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::{
    GeneratedPublicationProfile, GeneratedPublicationFeed,
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;

impl PublicationProfile {
    pub(crate) fn generated_profile(&self, profile: &FileOversightProfile, stream: StreamProfile,
        reserve: Option<RecoveryReserve>) -> Result<GeneratedPublicationProfile, JournalError>
    {
        self.check_scope(profile)?;
        // Joint held-out qualification has additional role/bootstrap semantics.
        // Never silently drop it when using this expressly non-joint composition.
        if self.joint.is_some() || (self.snapshot_fallback && self.feed.is_none()) {
            return Err(Error::Binding.into());
        }
        Ok(GeneratedPublicationProfile { stream, reserve, limits: self.limits,
            feed: self.feed.as_ref().map(|feed| GeneratedPublicationFeed {
                changes: feed.changes, freshness: feed.freshness, snapshot_fallback: self.snapshot_fallback,
            }) })
    }
}
