//! Bind the existing witness profile to the original learned owner. Current
//! learned inputs remain native; publication capture cannot replace them.
use super::{PublicationProfile, PreparedPublication, FileOversightProfile,
    FileSupervisedDriver, FileEvidenceSource, FileHumanPermit, FileDriverEvent,
    ElapsedTick, debug};
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedPublicationProfile,
    stream::generated::checked::GeneratedPublicationFeed,
    driver::{native_learned::{FileNativeSupervisedDriver, FileNativeDriverEvent},
        process_learned::{FileProcessSupervisedDriver, FileProcessDriverEvent}},
};

impl PublicationProfile {
    /// Only the immutable original validation/producer contract enters the
    /// learned recipe. Capture paths and witness requests belong to each actual
    /// request. Historical loading reads this profile, never those live files.
    pub(crate) fn learned_profile(&self, profile: &FileOversightProfile)
        -> Result<FileLearnedPublicationProfile, String>
    {
        self.check_scope(profile).map_err(debug)?;
        if self.wait.is_some() {
            return Err("learned checked publication does not support finite-wait profiles".into());
        }
        if self.joint.is_some() {
            return Err("learned joint qualification must be selected by the learned joint recipe, not a witness-profile wrapper".into());
        }
        if self.snapshot_fallback && self.feed.is_none() {
            return Err("learned snapshot publication requires its original producer feed".into());
        }
        if self.feed.as_ref().is_some_and(|feed|
            feed.freshness.clock_domain != profile.delivery.clock_domain) {
            return Err("learned publication feed uses a different clock domain".into());
        }
        Ok(FileLearnedPublicationProfile { limits: self.limits,
            feed: self.feed.as_ref().map(|feed| GeneratedPublicationFeed {
                changes: feed.changes, freshness: feed.freshness,
                snapshot_fallback: self.snapshot_fallback,
            }) })
    }
}

impl PreparedPublication<'_> {
    /// The same computed driver owns authorization, dispatch and final sealing.
    /// Every capture resolves the original learned sidecar and policy-only source
    /// alongside fresh witness/producer data; no caller substitutes helper bytes.
    pub(crate) fn step_learned<F>(&self, driver: &mut FileSupervisedDriver,
        evidence: &mut FileEvidenceSource, time: &mut F, human: Option<&FileHumanPermit>)
        -> Result<FileDriverEvent, String>
    where F: FnMut() -> ElapsedTick {
        match self.feed {
            Some(feed) => driver.step_computed_from_policy_file_with_publication_feed(
                evidence, self.current.reader(), &feed.reader, time, human, None)
                .publication.evidence.result.map_err(debug),
            None => driver.step_computed_from_policy_file_with_publication_source(
                evidence, self.current.reader(), time, human, None).evidence.result.map_err(debug),
        }
    }

    /// Native numerical reviewers retain their original current-input checks and
    /// cooperative work accounting while observing the same publication sources.
    pub(crate) fn step_native_learned<F>(&self, driver: &mut FileNativeSupervisedDriver,
        evidence: &mut FileEvidenceSource, time: &mut F, human: Option<&FileHumanPermit>)
        -> Result<FileNativeDriverEvent, String>
    where F: FnMut() -> ElapsedTick {
        match self.feed {
            Some(feed) => driver.step_from_policy_file_with_publication_feed(
                evidence, self.current.reader(), &feed.reader, time, human, None)
                .publication.evidence.result.map_err(debug),
            None => driver.step_from_policy_file_with_publication_source(
                evidence, self.current.reader(), time, human, None).evidence.result.map_err(debug),
        }
    }

    /// Process replies still enter through the original source-bound protocol;
    /// publication acquisition creates no alternate vote or process launch path.
    pub(crate) fn step_process_learned<F>(&self, driver: &mut FileProcessSupervisedDriver,
        evidence: &mut FileEvidenceSource, time: &mut F, human: Option<&FileHumanPermit>)
        -> Result<FileProcessDriverEvent, String>
    where F: FnMut() -> ElapsedTick {
        match self.feed {
            Some(feed) => driver.step_from_policy_file_with_publication_feed(
                evidence, self.current.reader(), &feed.reader, time, human, None)
                .publication.evidence.result.map_err(debug),
            None => driver.step_from_policy_file_with_publication_source(
                evidence, self.current.reader(), time, human, None).evidence.result.map_err(debug),
        }
    }
}
