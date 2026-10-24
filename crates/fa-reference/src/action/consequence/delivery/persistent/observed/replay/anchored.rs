//! Audit a completed decision only when independently retained history covers it.
//! The same canonical image supplies exact-prefix checks and every replay event.
use super::{Event, FileOversight, FileOversightProfile, FileReviewReplayCursor,
    JournalError, Path, journal, storage};
use super::super::guarded::anchored::FileHistoryAnchor;
use super::super::decoder::learned::{FileLearnedConfig, bind_history};
use super::super::consistency::learned::{FileLearnedConsistencyConfig, owned::bind_owned_history};
use crate::Error;

impl FileOversight {
    /// Require an independently retained exact history prefix covering the
    /// selected round's COMPLETE Finish, not merely its opening or vote phase.
    /// The original anchor checks path/profile and all canonical prefix bytes.
    /// Every unanchored suffix event must still pass original semantic replay.
    ///
    /// This is read-only and can run beside the original live writer. It does
    /// not recover an owner, renew eligibility, update the supplied anchor or
    /// authenticate its custodian. Rolling back both inputs defeats the anchor;
    /// matching it does not establish the truth of the original observations.
    pub fn begin_read_review_replay_anchored(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, round: u64, anchor: &FileHistoryAnchor)
        -> Result<FileReviewReplayCursor, JournalError>
    {
        let events = checked_history(directory.as_ref(), profile, round, anchor)?;
        FileReviewReplayCursor::new(profile, events, round, None)
    }

    /// Bind the independently selected learned recipe AFTER exact history
    /// admission and BEFORE numerical replay. Recipe-owned raw and owned-K/V
    /// predictors keep their original binder and witness checks. No model or
    /// forecast is inferred from the archive and no generic fallback is tried.
    pub fn begin_read_review_replay_anchored_with_learned_generation(
        directory: impl AsRef<Path>, profile: &FileOversightProfile,
        generation: &FileLearnedConfig, round: u64, anchor: &FileHistoryAnchor)
        -> Result<FileReviewReplayCursor, JournalError>
    {
        let mut events = checked_history(directory.as_ref(), profile, round, anchor)?;
        bind_history(&mut events, generation)?;
        FileReviewReplayCursor::new(profile, events, round, None)
    }

    /// Require exact independently retained history AND both original runtime
    /// recipes. A history match never permits substituting a standalone owned
    /// predictor or importing caller-provided captures as numerical evidence.
    pub fn begin_read_review_replay_anchored_with_owned_learned_consistency(
        directory: impl AsRef<Path>, profile: &FileOversightProfile,
        generation: &FileLearnedConfig, predictor: &FileLearnedConsistencyConfig,
        round: u64, anchor: &FileHistoryAnchor)
        -> Result<FileReviewReplayCursor, JournalError>
    {
        let mut events = checked_history(directory.as_ref(), profile, round, anchor)?;
        bind_owned_history(&mut events, generation, predictor)?;
        FileReviewReplayCursor::new(profile, events, round, None)
    }
}

fn checked_history(directory: &Path, profile: &FileOversightProfile, round: u64,
    anchor: &FileHistoryAnchor) -> Result<Vec<Event>, JournalError>
{
    if round == 0 { return Err(Error::InvalidInput.into()); }
    super::super::super::codec::validate_profile(&profile.delivery)?;
    let identity = storage::identity(directory)?;
    let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
    let events = journal::decode(profile, &identity, &bytes)?;
    anchor.check(profile, &identity, &events)?;
    let finish = events.iter().position(|event| super::finish_round(event) == Some(round))
        .ok_or(Error::Incomplete)?;
    let completed = u64::try_from(finish + 1).map_err(|_| Error::Limit)?;
    if anchor.revision() < completed { return Err(Error::Incomplete.into()); }
    // The cursor independently checks Begin/Finish ordering and subsequently
    // applies every original transition, even after the anchored review.
    Ok(events)
}
