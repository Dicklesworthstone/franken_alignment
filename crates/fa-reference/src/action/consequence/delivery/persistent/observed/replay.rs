//! Read-only extraction of a completed congress from the original durable journal.
//! No provider, helper, human endpoint, writer, or external effect is invoked.

mod anchored;
pub mod situation;
mod cursor;
mod effect;
mod learned;
pub use cursor::{FileReviewReplayCursor, FileReviewReplayProgress, FileReviewReplayStatus};
pub use effect::{AuditedAuthorization, AuditedDispatch, AuditedOutcome, AuditedReconciliation,
    FileReviewEffectReplay, ReviewEffectAudit};

use super::{FileDeliverySnapshot, FileOversight, FileOversightProfile, JournalError,
    journal::{self, Event}, machine::Machine, storage};
use super::decoder::{DecoderEvent, learned::{LearnedEvent,
    sidecar::{FileLearnedSidecarFinish, SidecarEvent}}};
use crate::action::consequence::oversight::replay::{ObservedDecisionArchive, ObservedReviewAnchor};
use crate::action::consequence::oversight::{CommitteeInput, ObservedReceipt};
use crate::action::consequence::activation::probe::learned::KvGroup;
use crate::{Error, Snapshot};
use std::path::Path;

/// One historical review, its REAL application result, and the enclosing cut.
/// A replayable Continue may have been rejected as stale; it is never relabeled
/// as an applied approval. The reconstructed anchor is not independently trusted.
/// Native learned-sidecar applications retain their original receipt and archive.
/// A refinement-only round has no application and reports Incomplete through
/// the legacy readers. Explicit completed-outcome inspection returns the distinct
/// FileReviewCompletion::Refined variant without inventing an application.
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
    pub fn begin_revision(&self) -> u64 {
        self.begin_revision
    }
    pub fn anchor(&self) -> &ObservedReviewAnchor {
        &self.anchor
    }
    pub fn archive(&self) -> &ObservedDecisionArchive {
        &self.archive
    }
    pub fn application(&self) -> &Result<ObservedReceipt, Error> {
        &self.application
    }
    pub fn application_inputs(&self) -> Option<&CommitteeInput> {
        self.application_inputs.as_ref()
    }
    pub fn application_snapshot(&self) -> &Snapshot {
        &self.application_snapshot
    }
    /// Original state immediately after the completed review's application.
    pub fn completed_snapshot(&self) -> &FileDeliverySnapshot {
        &self.completed
    }
    /// Historical state at the END of the selected canonical/acknowledged image.
    pub fn journal_snapshot(&self) -> &FileDeliverySnapshot {
        &self.journal
    }
}

/// One original completed refinement, before any new independent review.
/// The retained archive is the review of the OLD input. The exact K/V group and
/// new input revision are the original planner's refinement result, not a new
/// permission or an attempted policy application. A later review may have been
/// applied at the enclosing journal cut; it remains a different review.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::FileRefinedReviewReplay;
/// fn approve(refinement: FileRefinedReviewReplay) -> FilePermit { refinement }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRefinedReviewReplay {
    begin_revision: u64,
    anchor: ObservedReviewAnchor,
    archive: ObservedDecisionArchive,
    group: KvGroup,
    input_revision: u64,
    completed: FileDeliverySnapshot,
    journal: FileDeliverySnapshot,
}
impl FileRefinedReviewReplay {
    pub fn begin_revision(&self) -> u64 {
        self.begin_revision
    }
    pub fn anchor(&self) -> &ObservedReviewAnchor {
        &self.anchor
    }
    pub fn archive(&self) -> &ObservedDecisionArchive {
        &self.archive
    }
    /// Exact residual group acquired by the original completed refinement.
    pub fn group(&self) -> KvGroup {
        self.group
    }
    /// The original reviewed input, before the refinement.
    pub fn reviewed_input_revision(&self) -> u64 {
        self.archive.input_revision
    }
    /// The NEW original input revision, which still needs independent review.
    pub fn input_revision(&self) -> u64 {
        self.input_revision
    }
    pub fn completed_snapshot(&self) -> &FileDeliverySnapshot {
        &self.completed
    }
    pub fn journal_snapshot(&self) -> &FileDeliverySnapshot {
        &self.journal
    }
}

/// A completed original round, distinguishing application from refinement.
/// Applied retains the legacy result, including a real application refusal.
/// Refined has no application result: completion alone is never approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileReviewCompletion {
    Applied(FileReviewReplay),
    Refined(FileRefinedReviewReplay),
}
impl FileReviewCompletion {
    pub fn begin_revision(&self) -> u64 {
        match self {
            Self::Applied(review) => review.begin_revision(),
            Self::Refined(review) => review.begin_revision(),
        }
    }
    pub fn anchor(&self) -> &ObservedReviewAnchor {
        match self {
            Self::Applied(review) => review.anchor(),
            Self::Refined(review) => review.anchor(),
        }
    }
    pub fn archive(&self) -> &ObservedDecisionArchive {
        match self {
            Self::Applied(review) => review.archive(),
            Self::Refined(review) => review.archive(),
        }
    }
    pub fn completed_snapshot(&self) -> &FileDeliverySnapshot {
        match self {
            Self::Applied(review) => review.completed_snapshot(),
            Self::Refined(review) => review.completed_snapshot(),
        }
    }
    pub fn journal_snapshot(&self) -> &FileDeliverySnapshot {
        match self {
            Self::Applied(review) => review.journal_snapshot(),
            Self::Refined(review) => review.journal_snapshot(),
        }
    }
}

/// The original completed round and the same fully verified effect audit.
/// A later application and dispatch of the attempt is not a dispatch of the
/// selected refinement. The effect audit preserves that distinction and never
/// returns a live key or an independently authenticated external receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReviewCompletionAudit {
    pub(super) review: FileReviewCompletion,
    pub(super) effect: ReviewEffectAudit,
}
impl FileReviewCompletionAudit {
    pub fn review(&self) -> &FileReviewCompletion {
        &self.review
    }
    pub fn effect(&self) -> &ReviewEffectAudit {
        &self.effect
    }
}

// Both variants consume the ORIGINAL observed session. This selects a candidate
// completion only; the original machine must still validate its entire event.
fn finish_round(event: &Event) -> Option<u64> {
    match event {
        Event::Finish(round, ..) => Some(*round),
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
            SidecarEvent::Finish { round, .. }))) => Some(*round),
        _ => None,
    }
}

impl FileOversight {
    /// Supervisor-only pre-vote anchor. Reading never commits or opens a phase.
    /// Also works for an original helper-pool round before its first pump. Once
    /// the round is consumed, use review_replay for historical material instead.
    pub fn review_anchor(&self, round: u64) -> Result<ObservedReviewAnchor, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        Ok(self
            .machine
            .sessions
            .get(&round)
            .ok_or(Error::Missing)?
            .1
            .replay_anchor()?)
    }

    /// Export one explicit completed round from the last ACKNOWLEDGED journal.
    /// Reconstruct in RAM; the live broker, keys, source leases and clock do not
    /// change. Unfinished/abandoned rounds report Incomplete, never a fake verdict.
    /// A native refinement without an application also reports Incomplete.
    pub fn review_replay(&self, round: u64) -> Result<FileReviewReplay, JournalError> {
        self.begin_review_replay(round)?.complete()
    }

    /// Read one canonical image beside a live owner or after a crash. Validate
    /// the ENTIRE image, including its suffix, against the independent bootstrap
    /// before extracting. No lock, fence, staging cleanup or clock refresh occurs.
    /// A visible but unacknowledged completion is historical data, not live keys.
    pub fn read_review_replay(
        directory: impl AsRef<Path>,
        profile: &FileOversightProfile,
        round: u64,
    ) -> Result<FileReviewReplay, JournalError> {
        Self::begin_read_review_replay(directory, profile, round)?.complete()
    }
}

// Retain the previous extraction path as an independent test-only oracle.
#[cfg(test)]
fn extract(
    profile: &FileOversightProfile,
    events: &[Event],
    round: u64,
    at_end: FileDeliverySnapshot,
) -> Result<FileReviewReplay, JournalError> {
    if round == 0 {
        return Err(Error::InvalidInput.into());
    }
    let begin = events
        .iter()
        .position(|event| matches!(event, Event::Begin(_, id, ..) if *id == round))
        .ok_or(Error::Missing)?;
    let finish = events
        .iter()
        .position(|event| matches!(event, Event::Finish(id, ..) if *id == round))
        .ok_or(Error::Incomplete)?;
    if finish <= begin {
        return Err(Error::Binding.into());
    }

    let mut machine = Machine::new(profile)?;
    let mut anchor = None;
    for (position, event) in events[..finish].iter().enumerate() {
        machine.apply(event)?;
        if position == begin {
            anchor = Some(
                machine
                    .sessions
                    .get(&round)
                    .ok_or(Error::Missing)?
                    .1
                    .replay_anchor()?,
            );
        }
    }
    let anchor = anchor.ok_or(Error::Incomplete)?;
    let Event::Finish(_, supplied, snapshot) = &events[finish] else {
        unreachable!("selected completed review");
    };
    let attempt = machine.sessions.get(&round).ok_or(Error::Missing)?.0;
    // Reconstruct the exact supplied FINISH input too: it may differ from the
    // original reviewed input and explain a committed Stale/Incomplete refusal.
    let current = supplied
        .as_ref()
        .map(|views| {
            CommitteeInput::capture(
                machine.actions.get(&attempt).ok_or(Error::Missing)?,
                machine.broker.contracts(),
                views.clone(),
            )
        })
        .transpose()?;
    let now = machine
        .broker
        .inspect()
        .ledger
        .elapsed
        .ok_or(Error::Incomplete)?;
    let review = machine
        .sessions
        .get_mut(&round)
        .ok_or(Error::Missing)?
        .1
        .finish(now)?;
    machine.sessions.remove(&round);
    let archive = review.replay_archive();
    archive.verify(&anchor)?;
    // The SAME original broker applies the reconstructed review. No alternative
    // policy, consequence, source-eligibility or rights algorithm is introduced.
    let application = machine
        .broker
        .apply_review(review, current.as_ref(), snapshot);
    if let Ok(receipt) = &application {
        archive.verify_receipt(&anchor, receipt)?;
    }
    let completed = machine.snapshot(finish + 1);
    Ok(FileReviewReplay {
        begin_revision: (begin + 1) as u64,
        anchor,
        archive,
        application,
        application_inputs: current,
        application_snapshot: snapshot.clone(),
        completed,
        journal: at_end,
    })
}

#[cfg(test)]
mod tests;
