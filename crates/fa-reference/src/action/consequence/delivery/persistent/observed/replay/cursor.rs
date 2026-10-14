//! Read-only, single-pass decision replay. The source is one frozen image;
//! no mutable publication owner, store, callback or permitting output escapes.
use super::{CommitteeInput, Error, Event, FileDeliverySnapshot, FileOversight,
    FileOversightProfile, FileReviewReplay, JournalError, Machine,
    ObservedDecisionArchive, ObservedReceipt, ObservedReviewAnchor, Path, Snapshot,
    journal, storage};
use super::super::Transition;
use super::{FileReviewEffectReplay, effect::EffectTrace};
use super::super::super::MAX_JOURNAL_EVENTS;
use std::cell::OnceCell;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileReviewReplayStatus {
    Replaying,
    /// All events, including the suffix after the selected round, verified.
    Ready,
    /// An original reducer did not return; its private state is not reusable.
    Interrupted,
    Failed(Error),
    Cancelled,
}

/// Logical original-event attempts, not CPU time, I/O or an authority receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileReviewReplayProgress {
    pub replayed_events: usize,
    pub attempted_events: usize,
    pub total_events: usize,
    pub status: FileReviewReplayStatus,
}

struct Completed {
    archive: ObservedDecisionArchive,
    application: Result<ObservedReceipt, Error>,
    inputs: Option<CommitteeInput>,
    snapshot: Snapshot,
    state: FileDeliverySnapshot,
}

/// Historical inspector over one acknowledged or canonical input image.
/// It does not acquire the writer lock, fence a run, clean staging, invoke
/// helpers, refresh source eligibility or provide any live role. A concurrent
/// writer may advance; this inspection still describes its original frozen cut.
/// Initial bounded reading/framing, individual events and archive verification
/// remain synchronous. Dropping/cancelling never changes storage or live work.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::FileReviewReplayCursor;
/// fn early(cursor: &FileReviewReplayCursor) { cursor.archive(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::FileReviewReplayCursor;
/// fn role(cursor: FileReviewReplayCursor) { cursor.owner(); }
/// ```
#[must_use = "finish complete historical replay, or discard it without changing live state"]
pub struct FileReviewReplayCursor {
    events: Vec<Event>,
    machine: Machine,
    round: u64,
    begin: usize,
    finish: usize,
    replayed: usize,
    attempted: usize,
    status: FileReviewReplayStatus,
    anchor: Option<ObservedReviewAnchor>,
    capture: Option<Rc<OnceCell<ObservedDecisionArchive>>>,
    completed: Option<Completed>,
    acknowledged: Option<FileDeliverySnapshot>,
    effect: EffectTrace,
}

impl fmt::Debug for FileReviewReplayCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileReviewReplayCursor").field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Freeze the original acknowledged events, retaining their immutable runtime
    /// bindings. The host remains usable between polls. A poisoned owner cannot
    /// present its stale RAM cut as acknowledged current history.
    pub fn begin_review_replay(&self, round: u64) -> Result<FileReviewReplayCursor, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        FileReviewReplayCursor::new(&self.profile, self.events.clone(), round, Some(self.inspect()))
    }

    /// Inspect the complete canonical image beside a live writer or after a
    /// crash. A visible but unacknowledged completion remains historical data.
    /// Specialized runtime recipes must be supplied through their explicit API;
    /// this entry point cannot reconstruct them from untrusted journal bytes.
    pub fn begin_read_review_replay(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        round: u64) -> Result<FileReviewReplayCursor, JournalError>
    {
        let events = read_events(directory.as_ref(), profile)?;
        FileReviewReplayCursor::new(profile, events, round, None)
    }
}

pub(super) fn read_events(directory: &Path, profile: &FileOversightProfile)
    -> Result<Vec<Event>, JournalError>
{
    super::super::super::codec::validate_profile(&profile.delivery)?;
    let identity = storage::identity(directory)?;
    let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
    Ok(journal::decode(profile, &identity, &bytes)?)
}

impl FileReviewReplayCursor {
    // Only existing acknowledged history or fully decoded canonical input enters
    // this constructor. It is not a public event/claim import or replay shortcut.
    pub(super) fn new(profile: &FileOversightProfile, events: Vec<Event>, round: u64,
        acknowledged: Option<FileDeliverySnapshot>) -> Result<Self, JournalError>
    {
        if round == 0 { return Err(Error::InvalidInput.into()); }
        if events.len() > profile.delivery.limits.events || events.len() > MAX_JOURNAL_EVENTS {
            return Err(Error::Limit.into());
        }
        let begin = events.iter().position(|event| matches!(event,
            Event::Begin(_, id, ..) if *id == round)).ok_or(Error::Missing)?;
        let finish = events.iter().position(|event| matches!(event,
            Event::Finish(id, ..) if *id == round)).ok_or(Error::Incomplete)?;
        if finish <= begin { return Err(Error::Binding.into()); }
        let Event::Begin(attempt, ..) = &events[begin] else { return Err(Error::Binding.into()); };
        let effect = EffectTrace::new(*attempt);
        let machine = Machine::new(profile)?;
        Ok(Self { events, machine, round, begin, finish, replayed: 0, attempted: 0,
            status: FileReviewReplayStatus::Replaying, anchor: None, capture: None,
            completed: None, acknowledged, effect })
    }

    pub fn progress(&self) -> FileReviewReplayProgress {
        FileReviewReplayProgress { replayed_events: self.replayed, attempted_events: self.attempted,
            total_events: self.events.len(), status: self.status }
    }

    /// Each quantum names its verified-event predecessor. The original machine
    /// processes each event ONCE, including Finish and its admission/postlude.
    /// Refusals returned inside Reviewed remain real historical outcomes; an
    /// invalid journal transition or interrupted reducer seals the whole audit.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileReviewReplayProgress, JournalError>
    {
        match self.status {
            FileReviewReplayStatus::Failed(error) => return Err(error.into()),
            FileReviewReplayStatus::Interrupted => return Err(JournalError::Unavailable),
            FileReviewReplayStatus::Cancelled => return Err(Error::WrongState.into()),
            _ => {}
        }
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        if self.status == FileReviewReplayStatus::Ready { return Ok(self.progress()); }
        let end = self.replayed.saturating_add(max_events).min(self.events.len());
        while self.replayed < end {
            self.status = FileReviewReplayStatus::Interrupted;
            self.attempted += 1;
            if let Err(error) = self.step() {
                self.status = FileReviewReplayStatus::Failed(error);
                return Err(error.into());
            }
            self.replayed += 1;
            self.status = FileReviewReplayStatus::Replaying;
        }
        if self.replayed == self.events.len() {
            if self.completed.is_none() || self.acknowledged.as_ref().is_some_and(|expected|
                *expected != self.machine.snapshot(self.events.len())) {
                self.status = FileReviewReplayStatus::Failed(Error::Binding);
                return Err(Error::Binding.into());
            }
            self.status = FileReviewReplayStatus::Ready;
        }
        Ok(self.progress())
    }

    fn step(&mut self) -> Result<(), Error> {
        let event = &self.events[self.replayed];
        // Finish consumes the original session. Read its attempt before apply;
        // all attribution below waits for the original transition to succeed.
        let reviewed_attempt = match event {
            Event::Finish(round, ..) => self.machine.sessions.get(round).map(|session| session.0),
            _ => None,
        };
        let transition = self.machine.apply(event)?;
        self.effect.observe(event, &transition, reviewed_attempt, (self.replayed + 1) as u64,
            &self.machine.broker)?;
        if self.replayed == self.begin {
            let session = &mut self.machine.sessions.get_mut(&self.round).ok_or(Error::Missing)?.1;
            self.anchor = Some(session.replay_anchor()?);
            self.capture = Some(session.retain_replay_archive()?);
        }
        if self.replayed == self.finish {
            let Transition::Reviewed(application) = transition else { return Err(Error::Binding); };
            // Machine removed the consumed session; only this private observer
            // still owns its one-shot output. No archive is reconstructed from
            // a caller-supplied verdict or by running Finish a second time.
            let capture = self.capture.take().ok_or(Error::Incomplete)?;
            let archive = Rc::try_unwrap(capture).map_err(|_| Error::WrongState)?
                .into_inner().ok_or(Error::Incomplete)?;
            let anchor = self.anchor.as_ref().ok_or(Error::Incomplete)?;
            archive.verify(anchor)?;
            if let Ok(receipt) = &application { archive.verify_receipt(anchor, receipt)?; }
            let Event::Finish(_, supplied, snapshot) = &self.events[self.replayed] else {
                return Err(Error::Binding);
            };
            let inputs = supplied.as_ref().map(|views| CommitteeInput::capture(
                archive.inputs.action(), self.machine.broker.contracts(), views.clone())).transpose()?;
            self.completed = Some(Completed { archive, application, inputs, snapshot: snapshot.clone(),
                state: self.machine.snapshot(self.replayed + 1) });
        }
        Ok(())
    }

    /// Export only after the ENTIRE selected cut verified. An archive captured
    /// earlier cannot escape an invalid or unprocessed suffix. No fence is added.
    pub fn finish(self) -> Result<FileReviewReplay, JournalError> {
        Ok(self.finish_with_effects()?.review)
    }

    /// Return the same original review plus the exact selected-attempt effect
    /// chain. No outcome or consumed-approval link escapes an unverified suffix.
    /// This never obtains an endpoint receipt from outside the RAM-only replay.
    pub fn finish_with_effects(self) -> Result<FileReviewEffectReplay, JournalError> {
        match self.status {
            FileReviewReplayStatus::Ready => {}
            FileReviewReplayStatus::Failed(error) => return Err(error.into()),
            FileReviewReplayStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        let completed = self.completed.ok_or(Error::Incomplete)?;
        let journal = self.machine.snapshot(self.events.len());
        let effect = self.effect.finish(self.round, &completed.state, &journal)?;
        let review = FileReviewReplay { begin_revision: (self.begin + 1) as u64,
            anchor: self.anchor.ok_or(Error::Incomplete)?, archive: completed.archive,
            application: completed.application, application_inputs: completed.inputs,
            application_snapshot: completed.snapshot, completed: completed.state, journal };
        Ok(FileReviewEffectReplay { review, effect })
    }

    /// Discard private reconstruction and report its work, not a refund. A prior
    /// failure/interruption remains visible rather than being relabeled cancelled.
    pub fn cancel(self) -> FileReviewReplayProgress {
        let mut progress = self.progress();
        if !matches!(progress.status, FileReviewReplayStatus::Failed(_) | FileReviewReplayStatus::Interrupted) {
            progress.status = FileReviewReplayStatus::Cancelled;
        }
        progress
    }

    pub(super) fn complete(mut self) -> Result<FileReviewReplay, JournalError> {
        while self.status == FileReviewReplayStatus::Replaying {
            self.advance(self.replayed, MAX_JOURNAL_EVENTS)?;
        }
        self.finish()
    }
}
