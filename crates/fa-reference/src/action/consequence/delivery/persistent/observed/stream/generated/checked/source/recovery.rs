//! Cooperative reconstruction of the original source-bound native text owner.
//! Each event uses the original reducer; no partial owner or role escapes.
use super::{
    BaseEvent, ByteBpe, Event, FileDecoderConfig, FileHumanReviewer, FileOversight,
    FileOversightProfile, GeneratedSourceProfile, JournalError, MAX_FILE_TOKENIZER_BYTES, Machine,
    Path, journal, storage,
};
use crate::Error;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::{
    MAX_JOURNAL_EVENTS,
    observed::{
        decoder::text::{check_text_config, check_text_machine},
        machine::Transition,
        shutdown::{
            FileShutdownDrain,
            recovery::{FileAdmissionStoppedHistory, FileStopRecovery},
        },
    },
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileGeneratedSourceRecoveryStatus {
    Replaying,
    /// Complete original history and selected configuration have passed. No
    /// recovery Fence, Stop, storage cleanup, live owner or role exists yet.
    Ready,
    Interrupted,
    Failed(Error),
    Cancelled,
}

/// Original event calls, not a CPU, physical-work or elapsed-time receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileGeneratedSourceRecoveryProgress {
    pub replayed_events: usize,
    pub attempted_events: usize,
    pub total_events: usize,
    pub status: FileGeneratedSourceRecoveryStatus,
}

/// One exclusively locked canonical image and its private native reconstruction.
/// The same source, stream, reserve, publication, joint-policy, decoder and
/// tokenizer bindings as the synchronous source opener are mandatory. This does
/// not add independent latest-head, guard-role or anti-rollback assurance.
///
/// Creation reads and decodes the original bounded image but executes no event.
/// Advance bounds event calls; one event, final encoding and persistence remain
/// synchronous. Dropping or cancelling preserves canonical AND pending files.
/// Only a fully checked finish may release a live owner after its original Fence,
/// or transfer the same reconstruction to the authenticated terminal Stop path.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::source::FileGeneratedSourceRecovery;
/// fn early_owner(run: &FileGeneratedSourceRecovery) { run.owner(); }
/// ```
/// ```compile_fail,E0382
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::source::FileGeneratedSourceRecovery;
/// fn cancelled(mut run: FileGeneratedSourceRecovery) { run.cancel(); run.advance(0, 1); }
/// ```
#[must_use = "complete recovery or drop it without exposing a live owner"]
pub struct FileGeneratedSourceRecovery {
    profile: FileOversightProfile,
    selected: GeneratedSourceProfile,
    decoder: FileDecoderConfig,
    tokenizer: Vec<u8>,
    store: storage::Store,
    canonical: Vec<u8>,
    events: Vec<Event>,
    machine: Machine,
    last_drain: Option<FileShutdownDrain>,
    replayed: usize,
    attempted: usize,
    status: FileGeneratedSourceRecoveryStatus,
}

impl fmt::Debug for FileGeneratedSourceRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGeneratedSourceRecovery")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Freeze exactly the original source/native-text opener's inputs before
    /// historical numerical work, cleanup, fencing or role issuance. Retain one
    /// exclusive store across every quantum and either finalizer. This reads no
    /// live source, samples no clock and executes no model or tokenizer event.
    pub fn begin_open_generated_text_stream_from_source(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
    ) -> Result<FileGeneratedSourceRecovery, JournalError> {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) {
            return Err(Error::Binding.into());
        }
        let tokenizer = tokenizer.to_bytes()?;
        if tokenizer.len() > MAX_FILE_TOKENIZER_BYTES {
            return Err(Error::Limit.into());
        }
        let store = storage::Store::open(directory.as_ref())?;
        FileGeneratedSourceRecovery::begin_store(store, profile, decoder, tokenizer, selected)
    }
}

impl FileGeneratedSourceRecovery {
    fn begin_store(
        store: storage::Store,
        profile: FileOversightProfile,
        decoder: &FileDecoderConfig,
        tokenizer: Vec<u8>,
        selected: GeneratedSourceProfile,
    ) -> Result<Self, JournalError> {
        let canonical = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &canonical)?;
        selected.check(&events)?;
        check_text_config(decoder, &tokenizer, &events)?;
        let machine = Machine::new(&profile)?;
        Ok(Self {
            profile,
            selected,
            decoder: decoder.clone(),
            tokenizer,
            store,
            canonical,
            events,
            machine,
            last_drain: None,
            replayed: 0,
            attempted: 0,
            status: FileGeneratedSourceRecoveryStatus::Replaying,
        })
    }

    pub fn progress(&self) -> FileGeneratedSourceRecoveryProgress {
        FileGeneratedSourceRecoveryProgress {
            replayed_events: self.replayed,
            attempted_events: self.attempted,
            total_events: self.events.len(),
            status: self.status,
        }
    }

    /// Execute at most max_events ORIGINAL transitions from the exact verified
    /// predecessor. Invalid/stale polls do no work. Numerical state, draws and
    /// lifetime limits remain with the same reconstruction through every call.
    /// Failure or unwind latches before a partial machine could be reused.
    pub fn advance(
        &mut self,
        expected_events: usize,
        max_events: usize,
    ) -> Result<FileGeneratedSourceRecoveryProgress, JournalError> {
        self.advance_with(expected_events, max_events, Machine::apply)
    }

    fn advance_with(
        &mut self,
        expected_events: usize,
        max_events: usize,
        mut apply: impl FnMut(&mut Machine, &Event) -> Result<Transition, Error>,
    ) -> Result<FileGeneratedSourceRecoveryProgress, JournalError> {
        use FileGeneratedSourceRecoveryStatus::*;
        match self.status {
            Failed(error) => return Err(error.into()),
            Interrupted => return Err(JournalError::Unavailable),
            Cancelled => return Err(Error::WrongState.into()),
            _ => {}
        }
        if expected_events != self.replayed {
            return Err(Error::Stale.into());
        }
        if max_events == 0 {
            return Err(Error::InvalidInput.into());
        }
        if max_events > MAX_JOURNAL_EVENTS {
            return Err(Error::Limit.into());
        }
        if self.status == Ready {
            return Ok(self.progress());
        }
        let end = self
            .replayed
            .saturating_add(max_events)
            .min(self.events.len());
        while self.replayed < end {
            self.status = Interrupted;
            self.attempted += 1; // Bounded by the decoded original event count.
            let transition = match apply(&mut self.machine, &self.events[self.replayed]) {
                Ok(transition) => transition,
                Err(error) => {
                    self.status = Failed(error);
                    return Err(error.into());
                }
            };
            self.replayed += 1;
            if let Transition::StopProgressed(sweep) = transition {
                self.last_drain = Some(FileShutdownDrain {
                    journal_revision: self.replayed as u64,
                    sweep,
                });
            }
        }
        self.status = Interrupted;
        if self.replayed == self.events.len() {
            if let Err(error) = self.check_machine() {
                self.status = Failed(error);
                return Err(error.into());
            }
            self.status = Ready;
        } else {
            self.status = Replaying;
        }
        Ok(self.progress())
    }

    /// Consume only private replay state; cancellation does not fence, stop,
    /// clean staging or alter the durable original history.
    pub fn cancel(self) -> FileGeneratedSourceRecoveryProgress {
        let mut progress = self.progress();
        if matches!(
            progress.status,
            FileGeneratedSourceRecoveryStatus::Replaying | FileGeneratedSourceRecoveryStatus::Ready
        ) {
            progress.status = FileGeneratedSourceRecoveryStatus::Cancelled;
        }
        progress
    }

    fn check_machine(&self) -> Result<(), Error> {
        self.selected.check_machine(&self.machine)?;
        check_text_machine(&self.decoder, &self.tokenizer, &self.machine)
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        match self.status {
            FileGeneratedSourceRecoveryStatus::Ready => {}
            FileGeneratedSourceRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGeneratedSourceRecoveryStatus::Interrupted => {
                return Err(JournalError::Unavailable);
            }
            _ => return Err(Error::Incomplete.into()),
        }
        self.check_machine()?;
        Ok(())
    }

    /// Publish one original recovery Fence over the ALREADY VERIFIED machine.
    /// No historical numerical event is rerun. Old source/time eligibility and
    /// approvals withdraw; acknowledged numerical work, stream state and charged
    /// unknown effects remain. New work still needs fresh source/time and resume.
    ///
    /// Capacity, complete selected bindings and canonical equality precede
    /// staging cleanup. A failed replacement returns neither owner nor reviewer,
    /// even when the candidate replacement may already be visible on disk.
    pub fn finish(self) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        self.check_ready()?;
        let Self {
            profile,
            store,
            canonical,
            events,
            mut machine,
            ..
        } = self;
        if events.len() >= profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        let event = Event::Core(BaseEvent::Fence);
        let bytes = journal::encode_appended(&profile, store.identity(), &events, &event)?;
        machine.preflight_consistency(&event)?;
        let result = machine.apply(&event)?;
        let placeholder = Machine::new(&profile)?;
        if store.read(profile.delivery.limits.bytes)? != canonical {
            return Err(Error::Binding.into());
        }
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = FileOversight::owner(profile, store, events, placeholder);
        host.check_source_admission(&event)?;
        host.persist_candidate(event, bytes, machine, result)?;
        Ok((host, reviewer))
    }

    /// Select a Stop predecessor only after complete original reconstruction.
    /// An existing Stop retains its exact request; another operation cannot
    /// replace it. Selection reads no clock and writes no journal event.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        if operation == 0 {
            return Err(Error::InvalidInput.into());
        }
        let snapshot = self.machine.snapshot(self.events.len());
        if let Some(receipt) = snapshot.stop {
            let request = receipt.request();
            if request.operation != operation {
                return Err(Error::Duplicate.into());
            }
            return Ok(request);
        }
        Ok(StopRequest {
            operation,
            expected_control_sequence: snapshot.control.sequence,
            expected_authority_epoch: snapshot.control.ledger.epoch,
        })
    }

    /// The privileged host's explicit no-clock Stop over the same verified
    /// history. No ordinary recovery Fence precedes the native Stop/Fence pair,
    /// and no live owner, numerical continuation or effect settlement is issued.
    /// Unknown charges remain liabilities; acknowledgment does not imply drain.
    pub fn finish_admission_stop(
        self,
        request: StopRequest,
    ) -> Result<FileAdmissionStoppedHistory, JournalError> {
        self.into_terminal()?.finish_admission_stop(request)
    }

    fn into_terminal(self) -> Result<FileStopRecovery, JournalError> {
        self.check_ready()?;
        let Self {
            profile,
            store,
            canonical,
            events,
            machine,
            last_drain,
            ..
        } = self;
        Ok(FileStopRecovery::from_verified_terminal(
            profile, store, canonical, events, machine, last_drain,
        ))
    }

    /// Transfer the same complete reconstruction into the original authenticated
    /// Stop consumer, with no ordinary Fence or live owner first. A retained
    /// terminal reserve is mandatory and is never installed or replenished here.
    /// Only the peer's complete exact nonce-bound request authorizes the Stop.
    /// Exact historical retries retain the original operation and native cut.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(self, operation: u64) -> Result<
        crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop,
        JournalError,
    >{
        use crate::action::consequence::delivery::persistent::observed::reviewer::{
            client::ReviewerExpectation, peer::control::FileRecoveryStop,
        };
        let request = self.stop_request(operation)?;
        if self.selected.publication.reserve().is_none() {
            return Err(Error::Incomplete.into());
        }
        let expected = ReviewerExpectation {
            reviewer: self.profile.human.reviewer_id,
            scope: self.profile.delivery.scope,
            clock_domain: self.profile.delivery.clock_domain,
        };
        let recovery = self.into_terminal()?;
        Ok(FileRecoveryStop::from_verified(recovery, expected, request))
    }

    pub(super) fn finish_synchronously(
        mut self,
    ) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        while self.status == FileGeneratedSourceRecoveryStatus::Replaying {
            self.advance(self.replayed, MAX_JOURNAL_EVENTS)?;
        }
        self.finish()
    }
}

#[cfg(test)]
mod tests;
