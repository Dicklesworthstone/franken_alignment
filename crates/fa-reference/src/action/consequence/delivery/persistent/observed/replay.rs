//! Read-only extraction of a completed congress from the original durable journal.
//! No provider, helper, human endpoint, writer, or external effect is invoked.

use super::{FileDeliverySnapshot, FileOversight, FileOversightProfile, JournalError,
    journal::{self, Event}, machine::Machine, storage};
use crate::action::consequence::oversight::{CommitteeInput, ObservedReceipt};
use crate::action::consequence::oversight::replay::{ObservedDecisionArchive, ObservedReviewAnchor};
use crate::{Error, Snapshot};
use std::path::Path;

/// One historical review, its REAL application result, and the enclosing cut.
/// A replayable Continue may have been rejected as stale; it is never relabeled
/// as an applied approval. The reconstructed anchor is not independently trusted.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::FileReviewReplay;
/// fn recover_key(record: FileReviewReplay) -> FilePermit { record }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReviewReplay {
    begin_revision: u64,
    anchor: ObservedReviewAnchor,
    archive: ObservedDecisionArchive,
    application: Result<ObservedReceipt, Error>,
    application_inputs: Option<CommitteeInput>,
    application_snapshot: Snapshot,
    completed: FileDeliverySnapshot,
    journal: FileDeliverySnapshot,
}
impl FileReviewReplay {
    pub fn begin_revision(&self) -> u64 { self.begin_revision }
    pub fn anchor(&self) -> &ObservedReviewAnchor { &self.anchor }
    pub fn archive(&self) -> &ObservedDecisionArchive { &self.archive }
    pub fn application(&self) -> &Result<ObservedReceipt, Error> { &self.application }
    pub fn application_inputs(&self) -> Option<&CommitteeInput> { self.application_inputs.as_ref() }
    pub fn application_snapshot(&self) -> &Snapshot { &self.application_snapshot }
    /// Original state immediately after the completed review's application.
    pub fn completed_snapshot(&self) -> &FileDeliverySnapshot { &self.completed }
    /// Historical state at the END of the selected canonical/acknowledged image.
    pub fn journal_snapshot(&self) -> &FileDeliverySnapshot { &self.journal }
}

impl FileOversight {
    /// Supervisor-only pre-vote anchor. Reading never commits or opens a phase.
    /// Also works for an original helper-pool round before its first pump. Once
    /// the round is consumed, use review_replay for historical material instead.
    pub fn review_anchor(&self, round: u64) -> Result<ObservedReviewAnchor, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.sessions.get(&round).ok_or(Error::Missing)?.1.replay_anchor()?)
    }

    /// Export one explicit completed round from the last ACKNOWLEDGED journal.
    /// Reconstruct in RAM; the live broker, keys, source leases and clock do not
    /// change. Unfinished/abandoned rounds report Incomplete, never a fake verdict.
    pub fn review_replay(&self, round: u64) -> Result<FileReviewReplay, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        extract(&self.profile, &self.events, round, self.inspect())
    }

    /// Read one canonical image beside a live owner or after a crash. Validate
    /// the ENTIRE image, including its suffix, against the independent bootstrap
    /// before extracting. No lock, fence, staging cleanup or clock refresh occurs.
    /// A visible but unacknowledged completion is historical data, not live keys.
    pub fn read_review_replay(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        round: u64) -> Result<FileReviewReplay, JournalError>
    {
        super::super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        let at_end = Machine::replay(profile, &events)?.snapshot(events.len());
        extract(profile, &events, round, at_end)
    }
}

fn extract(profile: &FileOversightProfile, events: &[Event], round: u64,
    at_end: FileDeliverySnapshot) -> Result<FileReviewReplay, JournalError>
{
    if round == 0 { return Err(Error::InvalidInput.into()); }
    let begin = events.iter().position(|event| matches!(event, Event::Begin(_, id, ..) if *id == round))
        .ok_or(Error::Missing)?;
    let finish = events.iter().position(|event| matches!(event, Event::Finish(id, ..) if *id == round))
        .ok_or(Error::Incomplete)?;
    if finish <= begin { return Err(Error::Binding.into()); }

    let mut machine = Machine::new(profile)?;
    let mut anchor = None;
    for (position, event) in events[..finish].iter().enumerate() {
        machine.apply(event)?;
        if position == begin {
            anchor = Some(machine.sessions.get(&round).ok_or(Error::Missing)?.1.replay_anchor()?);
        }
    }
    let anchor = anchor.ok_or(Error::Incomplete)?;
    let Event::Finish(_, supplied, snapshot) = &events[finish] else {
        unreachable!("selected completed review");
    };
    let attempt = machine.sessions.get(&round).ok_or(Error::Missing)?.0;
    // Reconstruct the exact supplied FINISH input too: it may differ from the
    // original reviewed input and explain a committed Stale/Incomplete refusal.
    let current = supplied.as_ref().map(|views| {
        CommitteeInput::capture(machine.actions.get(&attempt).ok_or(Error::Missing)?,
            machine.broker.contracts(), views.clone())
    }).transpose()?;
    let now = machine.broker.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
    let review = machine.sessions.get_mut(&round).ok_or(Error::Missing)?.1.finish(now)?;
    machine.sessions.remove(&round);
    let archive = review.replay_archive();
    archive.verify(&anchor)?;
    // The SAME original broker applies the reconstructed review. No alternative
    // policy, consequence, source-eligibility or rights algorithm is introduced.
    let application = machine.broker.apply_review(review, current.as_ref(), snapshot);
    if let Ok(receipt) = &application { archive.verify_receipt(&anchor, receipt)?; }
    let completed = machine.snapshot(finish + 1);
    Ok(FileReviewReplay { begin_revision: (begin + 1) as u64, anchor, archive, application,
        application_inputs: current, application_snapshot: snapshot.clone(), completed, journal: at_end })
}

#[cfg(test)]
mod tests;
