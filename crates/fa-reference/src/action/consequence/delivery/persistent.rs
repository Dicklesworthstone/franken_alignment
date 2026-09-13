//! Whole-process recovery for one operator-controlled publication file.
//!
//! The original DeliveryBroker and PublicationEndpoint are replayed ONLY in RAM.
//! The one externally visible mutation is replacement of this host's journal.
//! No imported event can send a network request or operate another filesystem
//! endpoint. Storage and ballot authenticity remain operator assumptions.

mod codec;
mod storage;

use super::{DeliveryBroker, DispatchEnvelope, EndpointOutcome, EndpointStatus, PublicationEndpoint};
use super::super::congress::CongressPolicy;
use super::super::gate::{ControlInspection, TargetCeiling};
use super::super::gate::containment::ActorState;
use super::super::gate::containment::session::policy::{Policy, controller::{ControllerConfig, PolicyReceipt}};
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, ResolvedTarget, Scope};
use crate::round::Verdict;
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;
use std::rc::Rc;

pub const MAX_JOURNAL_EVENTS: usize = 4096;
pub const MAX_JOURNAL_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SNAPSHOT_ENTRIES: usize = 256;
pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalLimits { pub events: usize, pub bytes: usize }
impl Default for JournalLimits {
    fn default() -> Self { Self { events: MAX_JOURNAL_EVENTS, bytes: MAX_JOURNAL_BYTES } }
}
impl JournalLimits {
    fn check(self) -> Result<(), Error> {
        if self.events == 0 || self.events > MAX_JOURNAL_EVENTS || self.bytes == 0 || self.bytes > MAX_JOURNAL_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Independently supplied bootstrap DATA. Opening an existing store must supply
/// the same complete profile; total rights and policy are never read from a file
/// and accepted as new bootstrap authority. No actor/model restart runs here.
#[derive(Clone)]
pub struct FileDeliveryProfile {
    pub scope: Scope,
    pub total: u64,
    pub max_attempts: usize,
    pub actor: ActorState,
    pub suspend_at_incident: u64,
    pub policy: Policy,
    pub congress: CongressPolicy,
    pub narrowed_targets: Vec<ResolvedTarget>,
    pub target: ResolvedTarget,
    pub initial_payload: Vec<u8>,
    pub retention_ticks: u64,
    pub max_deliveries: usize,
    /// Names the operator's process-independent elapsed-clock domain. This host
    /// does not turn a saved tick or a new process's Instant into current time.
    pub clock_domain: u64,
    pub limits: JournalLimits,
}
impl fmt::Debug for FileDeliveryProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDeliveryProfile").field("scope", &self.scope)
            .field("target", &self.target).field("limits", &self.limits).finish_non_exhaustive()
    }
}

/// Trusted reference observations, not actor instructions. The original round
/// reconstructs commitments/reveals and evaluates the fixed congress roster.
/// An omitted member stays missing; this API does not authenticate a helper.
#[derive(Clone)]
pub struct ReferenceBallot { pub verdict: Verdict, pub salt: Vec<u8> }
#[derive(Clone)]
pub struct ReferenceReview {
    pub attempt: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub snapshot: Snapshot,
    pub ballots: BTreeMap<String, ReferenceBallot>,
}
impl fmt::Debug for ReferenceReview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReferenceReview").field("attempt", &self.attempt)
            .field("round", &self.round).field("ballots", &self.ballots.len()).finish_non_exhaustive()
    }
}

/// Process-local one-use handle. Its original broker permit stays private and
/// cannot be recovered by importing this handle or by guessing an attempt ID.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// fn duplicate(key: FilePermit) { let _ = key.clone(); }
/// ```
#[derive(Debug)]
pub struct FilePermit { issuer: Rc<()>, attempt: u64 }
impl FilePermit { pub fn attempt(&self) -> u64 { self.attempt } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalIo { Directory, Lock, Read, Stage, Write, FileSync, Rename, DirectorySync, Cleanup }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalFailure {
    pub operation: JournalIo,
    pub kind: io::ErrorKind,
    /// True once rename was attempted, including an ambiguous rename error.
    /// False is NOT an endpoint nonexecution receipt or permission to refund.
    pub replacement_may_be_visible: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalError { Contract(Error), Io(JournalFailure), Busy, Unavailable, InvalidFile }
impl From<Error> for JournalError { fn from(error: Error) -> Self { Self::Contract(error) } }
impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for JournalError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reconciliation { Resolved(EndpointOutcome), AwaitingResolution, RetentionExpired }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDeliverySnapshot {
    pub revision: u64,
    pub control: ControlInspection,
    pub dispatcher_epoch: u64,
    pub target: ResolvedTarget,
    pub payload: Vec<u8>,
    pub executions: u64,
}

/// Exclusive owner of the sole effect sink. No broker, endpoint, original permit
/// or sendable envelope accessor exists. Speculative replay projections are RAM
/// only; only the locked owner may publish their canonical journal replacement.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::FileDelivery;
/// fn escape(host: &mut FileDelivery) { host.broker_mut(); }
/// ```
pub struct FileDelivery {
    profile: FileDeliveryProfile,
    store: storage::Store,
    events: Vec<Event>,
    machine: Machine,
    issuer: Rc<()>,
    fault: Option<JournalFailure>,
}
impl fmt::Debug for FileDelivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDelivery").field("revision", &self.revision())
            .field("clock_ready", &self.clock_ready()).field("fault", &self.fault).finish_non_exhaustive()
    }
}

#[derive(Clone)]
enum Event {
    Time(ElapsedTick),
    Propose(u64, ActionSpec, Snapshot),
    Review(ReferenceReview),
    Authorize(u64, Snapshot),
    Dispatch(u64, Snapshot),
    Publish(u64),
    Reconcile(u64),
    Seal(u64),
    Cancel(u64),
    Fence,
}
enum Transition {
    Unit,
    Proposed(FrozenAction),
    Reviewed(PolicyReceipt),
    Published(EndpointOutcome),
    Reconciled(Reconciliation),
}

struct Machine {
    broker: DeliveryBroker,
    endpoint: PublicationEndpoint,
    actions: BTreeMap<u64, FrozenAction>,
    permits: BTreeMap<u64, Permit>,
    envelopes: BTreeMap<u64, DispatchEnvelope>,
    clock_ready: bool,
}
impl Machine {
    fn new(profile: &FileDeliveryProfile) -> Result<Self, Error> {
        codec::validate_profile(profile)?;
        let mut endpoint = PublicationEndpoint::new(profile.target, profile.initial_payload.clone(),
            profile.retention_ticks, profile.max_deliveries)?;
        let mut broker = DeliveryBroker::new(ControllerConfig {
            scope: profile.scope, total: profile.total, max_attempts: profile.max_attempts,
            actor: profile.actor.clone(), suspend_at_incident: profile.suspend_at_incident,
            policy: profile.policy.clone(), congress: profile.congress.clone(),
            narrowed_targets: TargetCeiling::new(&profile.narrowed_targets)?,
        }, &mut endpoint)?;
        broker.confirm_fence(endpoint.install_fence(broker.fence_request())?)?;
        Ok(Self { broker, endpoint, actions: BTreeMap::new(), permits: BTreeMap::new(),
            envelopes: BTreeMap::new(), clock_ready: false })
    }
    fn replay(profile: &FileDeliveryProfile, events: &[Event]) -> Result<Self, Error> {
        let mut machine = Self::new(profile)?;
        for event in events { machine.apply(event)?; }
        Ok(machine)
    }
    fn apply(&mut self, event: &Event) -> Result<Transition, Error> {
        if matches!(event, Event::Propose(..) | Event::Review(..) | Event::Authorize(..)
            | Event::Dispatch(..) | Event::Publish(..) | Event::Reconcile(..) | Event::Seal(..))
            && !self.clock_ready { return Err(Error::Incomplete); }
        match event {
            Event::Time(tick) => {
                self.broker.observe_time(*tick)?;
                self.endpoint.observe_time(*tick)?;
                self.clock_ready = true;
            }
            Event::Propose(id, spec, snapshot) => {
                let proposal = self.broker.propose(*id, spec.clone(), snapshot)?;
                self.actions.insert(*id, proposal.action.clone());
                return Ok(Transition::Proposed(proposal.action));
            }
            Event::Review(input) => {
                let mut session = self.broker.begin_review(input.attempt, input.round, input.evidence_root, &input.snapshot)?;
                for (member, ballot) in &input.ballots {
                    let commitment = session.commitment(member, ballot.verdict, &ballot.salt)?;
                    session.commit(member, commitment)?;
                }
                session.open_reveals()?;
                for (member, ballot) in &input.ballots { session.reveal(member, ballot.verdict, &ballot.salt)?; }
                let receipt = self.broker.apply_review(session.finish()?, &input.snapshot)?;
                return Ok(Transition::Reviewed(receipt));
            }
            Event::Authorize(id, snapshot) => {
                let permit = self.broker.authorize(*id, snapshot)?;
                self.permits.insert(*id, permit);
            }
            Event::Dispatch(id, snapshot) => {
                let action = self.actions.get(id).ok_or(Error::Missing)?;
                let permit = self.permits.get(id).ok_or(Error::Missing)?;
                let message = self.broker.dispatch(permit, action, snapshot)?;
                self.envelopes.insert(*id, message);
                self.permits.remove(id);
            }
            Event::Publish(id) => {
                let message = self.envelopes.get(id).ok_or(Error::Missing)?;
                let receipt = self.endpoint.deliver(message)?;
                // Publication and retained endpoint outcome are one persistent
                // replacement; acknowledgment/authority reconciliation is separate.
                return Ok(Transition::Published(receipt.outcome()));
            }
            Event::Reconcile(id) => {
                let query = self.broker.status_query(*id)?;
                let status = self.endpoint.status(&query)?;
                let status = self.broker.reconcile_status(&query, status)?;
                return Ok(Transition::Reconciled(match status {
                    EndpointStatus::Resolved(receipt) => Reconciliation::Resolved(receipt.outcome()),
                    EndpointStatus::AwaitingResolution => Reconciliation::AwaitingResolution,
                    EndpointStatus::RetentionExpired => Reconciliation::RetentionExpired,
                }));
            }
            Event::Seal(id) => {
                let query = self.broker.status_query(*id)?;
                let receipt = self.endpoint.seal_unexecuted(&query)?;
                let outcome = receipt.outcome();
                self.broker.accept_receipt(receipt)?;
                return Ok(Transition::Reconciled(Reconciliation::Resolved(outcome)));
            }
            Event::Cancel(id) => { self.broker.cancel(*id)?; self.permits.remove(id); }
            Event::Fence => {
                // The original authority keeps spent rights and its suspension.
                // Only undispatched attempts are eligible for cancellation/refund.
                self.broker.revoke_epoch()?;
                for (id, stage) in self.broker.inspect().ledger.stages {
                    if matches!(stage, ActionState::Proposed | ActionState::Prepared
                        | ActionState::Reviewing | ActionState::Authorized) { self.broker.cancel(id)?; }
                }
                let fence = self.broker.restart_dispatcher()?;
                self.broker.confirm_fence(self.endpoint.install_fence(fence)?)?;
                self.permits.clear();
                self.envelopes.clear(); // Old dispatches may be queried, NEVER resent.
                self.clock_ready = false;
            }
        }
        Ok(Transition::Unit)
    }
    fn snapshot(&self, revision: usize) -> FileDeliverySnapshot {
        FileDeliverySnapshot { revision: revision as u64, control: self.broker.inspect(),
            dispatcher_epoch: self.broker.dispatcher_epoch(), target: self.endpoint.target(),
            payload: self.endpoint.payload().to_vec(), executions: self.endpoint.execution_count() }
    }
}

impl FileDelivery {
    /// Provision a NEW protected directory. Existing paths, even empty ones,
    /// cannot be mistaken for permission to initialize another copy of rights.
    pub fn create(directory: impl AsRef<Path>, profile: FileDeliveryProfile) -> Result<Self, JournalError> {
        let machine = Machine::new(&profile)?;
        let store = storage::Store::create(directory.as_ref())?;
        let bytes = codec::encode(&profile, store.identity(), &[])?;
        store.replace(&bytes)?;
        Ok(Self { profile, store, events: Vec::new(), machine, issuer: Rc::new(()), fault: None })
    }

    /// Rebuild both original reducers from the complete canonical history, then
    /// durably fence old permissions BEFORE exposing a writable owner. No pending
    /// effect is resent or refunded. A fresh process-independent time observation
    /// is mandatory even when the archive contains a recent-looking saved tick.
    pub fn open(directory: impl AsRef<Path>, profile: FileDeliveryProfile) -> Result<Self, JournalError> {
        profile.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.limits.bytes)?;
        let events = codec::decode(&profile, store.identity(), &bytes)?;
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let mut host = Self { profile, store, events, machine, issuer: Rc::new(()), fault: None };
        host.transact(host.revision(), Event::Fence)?;
        Ok(host)
    }

    /// Read one immutable canonical replacement. Pure replay cannot expose an
    /// authority or publish a side effect. The returned elapsed tick is HISTORY,
    /// not proof that the corresponding permission window is still current.
    pub fn read_publication(directory: impl AsRef<Path>, profile: &FileDeliveryProfile) -> Result<FileDeliverySnapshot, JournalError> {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.limits.bytes)?;
        let events = codec::decode(profile, &identity, &bytes)?;
        Ok(Machine::replay(profile, &events)?.snapshot(events.len()))
    }

    pub fn revision(&self) -> u64 { self.events.len() as u64 }
    pub fn inspect(&self) -> FileDeliverySnapshot { self.machine.snapshot(self.events.len()) }
    pub fn clock_ready(&self) -> bool { self.fault.is_none() && self.machine.clock_ready }
    pub fn storage_failure(&self) -> Option<&JournalFailure> { self.fault.as_ref() }

    pub fn observe_time(&mut self, revision: u64, tick: ElapsedTick) -> Result<(), JournalError> {
        self.transact(revision, Event::Time(tick))?; Ok(())
    }
    pub fn propose(&mut self, revision: u64, attempt: u64, spec: ActionSpec,
        snapshot: Snapshot) -> Result<FrozenAction, JournalError>
    {
        match self.transact(revision, Event::Propose(attempt, spec, snapshot))? {
            Transition::Proposed(action) => Ok(action), _ => unreachable!("proposal transition"),
        }
    }
    pub fn review(&mut self, revision: u64, input: ReferenceReview) -> Result<PolicyReceipt, JournalError> {
        match self.transact(revision, Event::Review(input))? {
            Transition::Reviewed(receipt) => Ok(receipt), _ => unreachable!("review transition"),
        }
    }
    pub fn authorize(&mut self, revision: u64, attempt: u64, snapshot: Snapshot) -> Result<FilePermit, JournalError> {
        self.transact(revision, Event::Authorize(attempt, snapshot))?;
        Ok(FilePermit { issuer: Rc::clone(&self.issuer), attempt })
    }
    pub fn dispatch(&mut self, revision: u64, key: &FilePermit,
        exact_action: &FrozenAction, snapshot: Snapshot) -> Result<(), JournalError>
    {
        if !Rc::ptr_eq(&self.issuer, &key.issuer)
            || self.machine.actions.get(&key.attempt) != Some(exact_action) { return Err(Error::Binding.into()); }
        self.transact(revision, Event::Dispatch(key.attempt, snapshot))?; Ok(())
    }
    pub fn publish(&mut self, revision: u64, attempt: u64) -> Result<EndpointOutcome, JournalError> {
        match self.transact(revision, Event::Publish(attempt))? {
            Transition::Published(outcome) => Ok(outcome), _ => unreachable!("publication transition"),
        }
    }
    pub fn reconcile(&mut self, revision: u64, attempt: u64) -> Result<Reconciliation, JournalError> {
        match self.transact(revision, Event::Reconcile(attempt))? {
            Transition::Reconciled(outcome) => Ok(outcome), _ => unreachable!("reconciliation transition"),
        }
    }
    pub fn seal_unexecuted(&mut self, revision: u64, attempt: u64) -> Result<Reconciliation, JournalError> {
        match self.transact(revision, Event::Seal(attempt))? {
            Transition::Reconciled(outcome) => Ok(outcome), _ => unreachable!("sealing transition"),
        }
    }
    pub fn cancel(&mut self, revision: u64, attempt: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::Cancel(attempt))?; Ok(())
    }
    pub fn fence(&mut self, revision: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::Fence)?; Ok(())
    }

    fn transact(&mut self, revision: u64, event: Event) -> Result<Transition, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if self.events.len() >= self.profile.limits.events { return Err(Error::Limit.into()); }
        // Bound/encode the new inputs before replay or cloning a history. The
        // candidate is a private, non-effectful projection, not a second sink.
        let bytes = codec::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let result = candidate.apply(&event)?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        if let Err(error) = self.store.replace(&bytes) {
            if let JournalError::Io(failure) = &error { self.fault = Some(failure.clone()); }
            else { self.fault = Some(JournalFailure { operation: JournalIo::Stage,
                kind: io::ErrorKind::Other, replacement_may_be_visible: true }); }
            // Do not return a permit, outcome or refund after ambiguous I/O.
            // inspect() remains the last fully acknowledged state; disk may be
            // newer. Both are unavailable for new effects until exclusive reopen.
            return Err(error);
        }
        self.events.push(event);
        self.machine = candidate;
        Ok(result)
    }
}
