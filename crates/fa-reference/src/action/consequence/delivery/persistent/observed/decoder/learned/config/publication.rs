//! Bind original final-publication witnesses to the independent learned recipe.
use super::{FileLearnedConfig, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use crate::action::consequence::delivery::persistent::observed::{
    publication::witness_gate::{self, WitnessEvent, freshness::FreshnessEvent},
    stream::generated::checked::GeneratedPublicationFeed,
};
use crate::Error;

/// Exact original witness and optional change-tail/freshness contract. The
/// existing learned recipe already binds its stream; the owning constructor
/// independently selects its terminal reserve. Neither is duplicated here.
/// Bootstrap validates these fields through the original publication reducers.
/// This registration supplies no capture, heartbeat, review result or effect key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedPublicationProfile {
    pub limits: PublicationLimits,
    pub feed: Option<GeneratedPublicationFeed>,
}

impl FileLearnedPublicationProfile {
    pub(in super::super::super::super) fn events(self) -> Vec<WitnessEvent> {
        let mut events = vec![WitnessEvent::Enable(self.limits)];
        if let Some(feed) = self.feed {
            events.push(WitnessEvent::ChangeProfile(feed.changes));
            events.push(WitnessEvent::Freshness(FreshnessEvent::Enable(feed.freshness)));
            if feed.snapshot_fallback {
                events.push(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback));
            }
        }
        events
    }
}

impl FileLearnedConfig {
    /// Require the ORIGINAL final-publication witness gate, with its complete
    /// change-tail, producer-clock and exact-snapshot selection when configured.
    /// The same native Learned Enable installs every selected guard before a
    /// numerical owner can escape. A conflicting preinstalled witness profile
    /// refuses instead of silently selecting one contract or installing twice.
    ///
    /// Every learned recovery, receipt and audit reader already compares the
    /// complete independent recipe before numerical replay. Changed or omitted
    /// witness requirements therefore cannot select the same historical owner.
    /// No live evidence is recovered; fresh acquisition and original validation
    /// remain mandatory at authorization, dispatch and final publication.
    ///
    /// Existing configurations keep their exact bytes. This disjoint wrapper
    /// retains all identity, credibility, source, predictor and sidecar choices;
    /// complete wrapper order remains part of recipe identity.
    pub fn with_required_publication_validation(mut self, profile: FileLearnedPublicationProfile)
        -> Result<Self, Error>
    {
        if self.publication.is_some() { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?; writer.u64(0)?; writer.raw(b"FALPUB\0\x01")?;
        writer.blob(&self.bytes)?;
        let events = profile.events();
        writer.count(events.len())?;
        for event in &events { witness_gate::write(&mut writer, event)?; }
        self.bytes = writer.finish().into();
        self.publication = Some(profile);
        Ok(self)
    }

    /// Frozen original validation requirements, never a claim that evidence is
    /// present, fresh, complete or sufficient for a particular publication.
    pub fn required_publication_validation(&self) -> Option<FileLearnedPublicationProfile> {
        self.publication
    }
}
