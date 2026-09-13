//! Durable whole-input congress review AND mandatory two-key publication.
//!
//! The original OversightBroker owns all judgments, approvals and accounting.
//! Its endpoint is memory-only during replay; the existing locked Store owns the
//! sole externally visible canonical replacement. There is no one-key fallback.
//! Operator storage, helper provenance, human identity and clock remain trusted.
mod journal;
mod machine;
mod human;
mod views;
pub mod helpers;
pub mod driver;
pub mod reviewer;
#[cfg(test)]
mod tests;
pub use human::{FileHumanPermit, FileHumanRequest, FileHumanReviewer};

use super::{FileDeliveryProfile, FileDeliverySnapshot, FilePermit, FileStopSweep,
    JournalError, JournalFailure, JournalIo, Reconciliation, Event as BaseEvent, storage};
use super::super::{EndpointOutcome, StopProgress, StopReceipt, StopRequest};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, ObservedReceipt, ReviewWindow};
use crate::action::consequence::oversight::human::{HumanReviewPolicy, HumanStatus};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction};
use crate::evidence_view::EvidenceViewManifest;
use crate::round::{Digest, Verdict};
use crate::{Error, Snapshot};
use journal::Event;
use machine::{Machine, Transition};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::Path;
use std::rc::Rc;

/// Independent bootstrap authority. The complete original publication profile,
/// effective helper contracts and mandatory reviewer policy are bound to disk.
/// This profile deliberately does not enable model/identity/learning subprofiles.
#[derive(Clone, Debug)]
pub struct FileOversightProfile {
    pub delivery: FileDeliveryProfile,
    pub committee: CommitteeContract,
    pub human: HumanReviewPolicy,
}

/// Exclusive, bounded owner of one full-input/two-key publication domain.
/// The separately returned reviewer role must not be handed to the actor.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
/// fn bypass(host: &mut FileOversight) { let _ = host.broker_mut(); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
/// fn recover_reviewer(host: &FileOversight) { let _ = host.reviewer(); }
/// ```
pub struct FileOversight {
    profile: FileOversightProfile,
    store: storage::Store,
    events: Vec<Event>,
    machine: Machine,
    issuer: Rc<()>,
    fault: Option<JournalFailure>,
    // A worker round can never switch to manual votes in this live owner.
    // Recovery discards every native session before returning a new owner.
    worker_rounds: BTreeSet<u64>,
}
impl fmt::Debug for FileOversight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileOversight").field("revision", &self.revision())
            .field("clock_ready", &self.clock_ready()).field("fault", &self.fault).finish_non_exhaustive()
    }
}

impl FileOversight {
    pub fn create(directory: impl AsRef<Path>, profile: FileOversightProfile) -> Result<(Self, FileHumanReviewer), JournalError> {
        let machine = Machine::new(&profile)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &[])?)?;
        Ok(Self::owner(profile, store, Vec::new(), machine))
    }

    /// Reconstruct the original broker, then durably withdraw ALL old human
    /// keys, cancel only undispatched reservations and fence the endpoint BEFORE
    /// exposing an owner or fresh reviewer role. Old dispatches are query-only.
    /// Saved time is never current; a fresh explicit observation is still needed.
    pub fn open(directory: impl AsRef<Path>, profile: FileOversightProfile) -> Result<(Self, FileHumanReviewer), JournalError> {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }
    fn owner(profile: FileOversightProfile, store: storage::Store, events: Vec<Event>, machine: Machine) -> (Self, FileHumanReviewer) {
        let issuer = Rc::new(());
        let reviewer = FileHumanReviewer { issuer: Rc::clone(&issuer), reviewer: profile.human.reviewer_id };
        (Self { profile, store, events, machine, issuer, fault: None, worker_rounds: BTreeSet::new() }, reviewer)
    }

    /// Historical data only. No live broker, helper session or approval key is
    /// returned by this pure read, including after an unacknowledged replacement.
    pub fn read_publication(directory: impl AsRef<Path>, profile: &FileOversightProfile) -> Result<FileDeliverySnapshot, JournalError> {
        super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        Ok(Machine::replay(profile, &events)?.snapshot(events.len()))
    }
    pub fn revision(&self) -> u64 { self.events.len() as u64 }
    pub fn inspect(&self) -> FileDeliverySnapshot { self.machine.snapshot(self.events.len()) }
    pub fn clock_ready(&self) -> bool { self.fault.is_none() && self.machine.clock_ready }
    pub fn storage_failure(&self) -> Option<&JournalFailure> { self.fault.as_ref() }
    pub fn input_revision(&self, attempt: u64) -> Result<u64, JournalError> { Ok(self.machine.broker.input_revision(attempt)?) }
    pub fn human_status(&self, request: u64) -> Result<HumanStatus, JournalError> { Ok(self.machine.broker.human_status(request)?) }

    pub fn observe_time(&mut self, revision: u64, tick: ElapsedTick) -> Result<(), JournalError> {
        self.transact(revision, Event::Core(BaseEvent::Time(tick)))?; Ok(())
    }
    pub fn propose(&mut self, revision: u64, attempt: u64, action: ActionSpec, snapshot: Snapshot) -> Result<FrozenAction, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::Propose(attempt, action, snapshot)))? {
            Transition::Proposed(action) => Ok(action), _ => unreachable!("proposal transition"),
        }
    }
    pub fn record_inputs(&mut self, revision: u64, attempt: u64, expected: u64, inputs: CommitteeInput) -> Result<u64, JournalError> {
        inputs.validate_for(self.machine.actions.get(&attempt).ok_or(Error::Missing)?, self.machine.broker.contracts())?;
        match self.transact(revision, Event::Inputs(attempt, expected, inputs.views().clone()))? {
            Transition::Inputs(revision) => Ok(revision), _ => unreachable!("input transition"),
        }
    }
    pub fn inputs_unavailable(&mut self, revision: u64, attempt: u64, expected: u64) -> Result<u64, JournalError> {
        match self.transact(revision, Event::InputsUnavailable(attempt, expected))? {
            Transition::Inputs(revision) => Ok(revision), _ => unreachable!("input withdrawal transition"),
        }
    }
    pub fn begin_review(&mut self, revision: u64, attempt: u64, round: u64, evidence_root: [u8; 32],
        window: ReviewWindow, snapshot: Snapshot) -> Result<(), JournalError>
    {
        self.transact(revision, Event::Begin(attempt, round, evidence_root, window, snapshot))?; Ok(())
    }
    /// Exactly the bytes and provenance frozen for THIS member. Read-only data,
    /// not a worker credential, current-source claim or automatic review result.
    pub fn review_input(&self, round: u64, member: &str) -> Result<&EvidenceViewManifest, JournalError> {
        Ok(self.machine.sessions.get(&round).ok_or(Error::Missing)?.1.input(member)?)
    }
    /// Trusted worker-channel observation, through the original reference digest
    /// importer. The configured roster is fixed; this does not authenticate a peer.
    pub fn commit_review(&mut self, revision: u64, round: u64, member: &str, digest: Digest) -> Result<(), JournalError> {
        self.check_manual_round(round)?;
        self.review_input(round, member)?;
        self.transact(revision, Event::Commit(round, member.to_owned(), digest))?; Ok(())
    }
    pub fn open_reveals(&mut self, revision: u64, round: u64) -> Result<(), JournalError> {
        self.check_manual_round(round)?;
        self.transact(revision, Event::OpenReveals(round))?; Ok(())
    }
    pub fn reveal_review(&mut self, revision: u64, round: u64, member: &str, verdict: Verdict, salt: Vec<u8>) -> Result<(), JournalError> {
        self.check_manual_round(round)?;
        self.review_input(round, member)?;
        self.transact(revision, Event::Reveal(round, member.to_owned(), verdict, salt))?; Ok(())
    }

    /// Outer Err: no acknowledged journal transaction. Inner Err: the ORIGINAL
    /// completed review was consumed and its application refusal was COMMITTED.
    /// That completed round can never be repaired, reused or applied twice.
    /// Restrictive original decisions may still apply with current=None.
    pub fn finish_review(&mut self, revision: u64, round: u64, current: Option<&CommitteeInput>,
        snapshot: Snapshot) -> Result<Result<ObservedReceipt, Error>, JournalError>
    {
        self.check_manual_round(round)?;
        let attempt = self.machine.sessions.get(&round).ok_or(Error::Missing)?.0;
        if let Some(input) = current { self.check_action(attempt, input.action())?; }
        let supplied = current.map(|input| input.views().clone());
        match self.transact(revision, Event::Finish(round, supplied, snapshot))? {
            Transition::Reviewed(result) => Ok(result), _ => unreachable!("completed review transition"),
        }
    }

    // Only byte-identical, explicitly supplied current observations may be
    // referenced by a revision instead of duplicated into every subsequent event.
    // Recovery replays historical transitions in RAM; it never calls this a fresh
    // provider observation or exposes the reconstructed live keys to the caller.
    fn current_reference(&self, attempt: u64, supplied: &CommitteeInput) -> Result<u64, JournalError> {
        let current = self.machine.broker.current_inputs(attempt)?.ok_or(Error::Incomplete)?;
        if current != supplied { return Err(Error::Stale.into()); }
        Ok(self.machine.broker.input_revision(attempt)?)
    }
    fn check_action(&self, attempt: u64, action: &FrozenAction) -> Result<(), JournalError> {
        if self.machine.actions.get(&attempt) != Some(action) { return Err(Error::Binding.into()); }
        Ok(())
    }
    pub fn authorize(&mut self, revision: u64, attempt: u64, current: &CommitteeInput, snapshot: Snapshot) -> Result<FilePermit, JournalError> {
        let input_revision = self.current_reference(attempt, current)?;
        self.transact(revision, Event::Authorize(attempt, input_revision, snapshot))?;
        Ok(FilePermit { issuer: Rc::clone(&self.issuer), attempt })
    }
    pub fn request_human_approval(&mut self, revision: u64, request: u64, attempt: u64,
        current: &CommitteeInput, expires_at: ElapsedTick) -> Result<FileHumanRequest, JournalError>
    {
        let input_revision = self.current_reference(attempt, current)?;
        match self.transact(revision, Event::RequestHuman(request, attempt, input_revision, expires_at))? {
            Transition::HumanRequested(evidence) => Ok(FileHumanRequest { issuer: Rc::clone(&self.issuer), evidence }),
            _ => unreachable!("human request transition"),
        }
    }
    /// Recover only the immutable review request, never a lost approval key.
    pub fn human_request(&self, request: u64) -> Result<FileHumanRequest, JournalError> {
        Ok(FileHumanRequest { issuer: Rc::clone(&self.issuer), evidence: self.machine.broker.human_request(request)? })
    }
    pub fn dispatch(&mut self, revision: u64, automatic: &FilePermit, human: &FileHumanPermit,
        action: &FrozenAction, current: &CommitteeInput, snapshot: Snapshot) -> Result<(), JournalError>
    {
        if !Rc::ptr_eq(&self.issuer, &automatic.issuer) || !Rc::ptr_eq(&self.issuer, &human.issuer)
            || automatic.attempt != human.attempt { return Err(Error::Binding.into()); }
        self.check_action(automatic.attempt, action)?;
        let input_revision = self.current_reference(automatic.attempt, current)?;
        self.transact(revision, Event::Dispatch(automatic.attempt, human.request, input_revision, snapshot))?;
        Ok(())
    }
    pub fn publish(&mut self, revision: u64, attempt: u64) -> Result<EndpointOutcome, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::Publish(attempt)))? {
            Transition::Published(result) => Ok(result), _ => unreachable!("publication transition"),
        }
    }
    pub fn reconcile(&mut self, revision: u64, attempt: u64) -> Result<Reconciliation, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::Reconcile(attempt)))? {
            Transition::Reconciled(result) => Ok(result), _ => unreachable!("reconciliation transition"),
        }
    }
    pub fn seal_unexecuted(&mut self, revision: u64, attempt: u64) -> Result<Reconciliation, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::Seal(attempt)))? {
            Transition::Reconciled(result) => Ok(result), _ => unreachable!("sealing transition"),
        }
    }
    pub fn reconcile_pending(&mut self, revision: u64) -> Result<BTreeMap<u64, Result<Reconciliation, Error>>, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::Sweep))? {
            Transition::Swept(results) => Ok(results), _ => unreachable!("pending sweep transition"),
        }
    }
    pub fn cancel(&mut self, revision: u64, attempt: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::Core(BaseEvent::Cancel(attempt)))?; Ok(())
    }
    pub fn fence(&mut self, revision: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::Core(BaseEvent::Fence))?; Ok(())
    }
    pub fn request_stop(&mut self, revision: u64, request: StopRequest) -> Result<StopReceipt, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::Stop(request)))? {
            Transition::Stopped(receipt) => Ok(receipt), _ => unreachable!("terminal stop transition"),
        }
    }
    pub fn stop_progress(&self) -> Result<StopProgress, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.stop_progress()?)
    }
    pub fn progress_stop(&mut self, revision: u64, tick: ElapsedTick) -> Result<FileStopSweep, JournalError> {
        match self.transact(revision, Event::Core(BaseEvent::StopProgress(tick)))? {
            Transition::StopProgressed(result) => Ok(result), _ => unreachable!("stop drain transition"),
        }
    }

    fn transact(&mut self, revision: u64, event: Event) -> Result<Transition, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        if let Err(error) = self.store.replace(&bytes) {
            self.fault = Some(match &error {
                JournalError::Io(failure) => failure.clone(),
                _ => JournalFailure { operation: JournalIo::Stage, kind: io::ErrorKind::Other, replacement_may_be_visible: true },
            });
            // No candidate automatic/human key, decision, receipt or refund is
            // returned. Inspection remains the last fully acknowledged cut.
            return Err(error);
        }
        self.events.push(event);
        self.machine = candidate;
        Ok(result)
    }
}